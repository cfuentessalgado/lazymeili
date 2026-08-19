mod action;
mod app;
mod config;
mod editor;
mod event;
mod meili;
mod secrets;
mod terminal;
mod ui;

use std::{io::Write, sync::Arc, time::Duration};

use base64::Engine;

use clap::Parser;
use crossterm::event::{Event as CrosstermEvent, KeyEventKind};
use tracing_subscriber::EnvFilter;

#[cfg(target_os = "linux")]
use crate::secrets::NativeStore;
use crate::{
    action::map_key,
    app::{App, Command, CommandOrEditor, Message},
    event::Event,
    meili::{HttpService, MeiliService},
    secrets::{AgeVault, Secrets},
};

#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Delete the encrypted fallback vault after an explicit typed confirmation.
    #[arg(long)]
    reset_vault: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .try_init()
        .ok();
    let paths = config::Paths::discover()?;
    if cli.reset_vault {
        let confirmation =
            rpassword::prompt_password("Type RESET to delete all fallback-vault secrets: ")?;
        anyhow::ensure!(confirmation == "RESET", "vault reset canceled");
        AgeVault::reset(&paths.vault)?;
        println!("Fallback vault removed.");
        return Ok(());
    }
    let store = config::ConfigStore::new(paths.config.clone());
    let config = store.load()?;
    let fallback = prepare_vault(&paths.vault)?;
    let secrets = Secrets::new(fallback);

    terminal::install_panic_hook();
    let mut terminal = terminal::init()?;
    let result = run(&mut terminal, App::new(config, store, secrets)).await;
    terminal::restore()?;
    result
}

fn prepare_vault(path: &std::path::Path) -> anyhow::Result<Option<AgeVault>> {
    if AgeVault::exists(path) {
        let passphrase = rpassword::prompt_password("Unlock mtui secret vault: ")?;
        return AgeVault::open(path.to_path_buf(), passphrase).map(Some);
    }
    #[cfg(target_os = "linux")]
    if !NativeStore::available() {
        eprintln!("Linux Secret Service is unavailable. Create the encrypted mtui fallback vault.");
        let first = rpassword::prompt_password("New vault passphrase: ")?;
        let second = rpassword::prompt_password("Confirm vault passphrase: ")?;
        anyhow::ensure!(first == second, "vault passphrases did not match");
        return AgeVault::open(path.to_path_buf(), first).map(Some);
    }
    Ok(None)
}

async fn run(terminal: &mut terminal::Tui, mut app: App) -> anyhow::Result<()> {
    let mut events = event::Events::new(Duration::from_millis(250));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    if let Some(command) = app.startup_command() {
        dispatch(command, &app, tx.clone());
    }
    let mut ticks = 0_u16;
    while app.running {
        terminal.draw(|frame| ui::draw(frame, &app))?;
        tokio::select! {
            Some(message) = rx.recv() => app.apply(message),
            event = events.next() => match event? {
                Event::Input(CrosstermEvent::Key(key)) if key.kind == KeyEventKind::Press => {
                    let action = map_key(key, app.is_editing());
                    let effects = app.update(action);
                    process_effects(effects, &mut app, terminal, &tx)?;
                }
                Event::Tick => {
                    ticks = ticks.wrapping_add(1);
                    if ticks.is_multiple_of(28) {
                        let effects = app.timed_refresh();
                        process_effects(effects, &mut app, terminal, &tx)?;
                    }
                }
                Event::Input(_) => {}
            }
        }
    }
    Ok(())
}

fn process_effects(
    effects: Vec<CommandOrEditor>,
    app: &mut App,
    terminal: &mut terminal::Tui,
    tx: &tokio::sync::mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    for effect in effects {
        match effect {
            CommandOrEditor::Command(command) => {
                app.loading = true;
                dispatch(command, app, tx.clone());
            }
            CommandOrEditor::Clipboard(value) => {
                let encoded = base64::engine::general_purpose::STANDARD.encode(value);
                write!(std::io::stdout(), "\x1b]52;c;{encoded}\x07")?;
                std::io::stdout().flush()?;
                app.notice =
                    Some("API key copied with OSC 52; clear your clipboard after use".into());
            }
            CommandOrEditor::FetchSettings => {
                if let (Some(service), Some(index)) =
                    (app.service.clone(), app.selected_index.clone())
                {
                    app.loading = true;
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let result = service
                            .settings(&index)
                            .await
                            .map(Message::Settings)
                            .unwrap_or_else(|error| Message::Error(error.to_string()));
                        let _ = tx.send(result);
                    });
                }
            }
            CommandOrEditor::Editor(target, initial) => match editor::edit_json(terminal, &initial)
            {
                Ok(Some(value)) => {
                    let followups = app.editor_result(target, value);
                    process_effects(followups, app, terminal, tx)?;
                }
                Ok(None) => app.notice = Some("No changes".into()),
                Err(error) => app.apply(Message::Error(error.to_string())),
            },
        }
    }
    Ok(())
}

fn dispatch(command: Command, app: &App, tx: tokio::sync::mpsc::UnboundedSender<Message>) {
    let service = app.service.clone();
    let index = app.selected_index.clone();
    tokio::spawn(async move {
        execute(command, service, index, &tx).await;
    });
}

async fn execute(
    command: Command,
    service: Option<Arc<HttpService>>,
    index: Option<String>,
    tx: &tokio::sync::mpsc::UnboundedSender<Message>,
) {
    if let Command::Connect { url, key } = command {
        let result = async {
            let service = Arc::new(HttpService::new(url, key)?);
            service.health().await?;
            let version = service.version().await?;
            let stats = service.stats().await?;
            let indexes = service.indexes(0, 1000).await?.results;
            Ok::<_, meili::Error>(Message::Connected {
                service,
                version,
                stats,
                indexes,
            })
        }
        .await;
        let _ = tx.send(result.unwrap_or_else(|error| Message::Error(error.to_string())));
        return;
    }
    let Some(service) = service else {
        let _ = tx.send(Message::Error("No connected application".into()));
        return;
    };
    let result: meili::Result<Message> = match command {
        Command::RefreshDashboard => {
            async {
                Ok(Message::Dashboard {
                    stats: service.stats().await?,
                    indexes: service.indexes(0, 1000).await?.results,
                })
            }
            .await
        }
        Command::Search(query) => {
            require_index(index.as_deref())
                .and_then_async(|uid| async {
                    service.search(uid, &query).await.map(Message::Search)
                })
                .await
        }
        Command::CreateIndex { uid, primary_key } => service
            .create_index(&uid, primary_key.as_deref())
            .await
            .map(Message::TaskQueued),
        Command::UpdatePrimaryKey { uid, primary_key } => service
            .update_primary_key(&uid, &primary_key)
            .await
            .map(Message::TaskQueued),
        Command::DeleteIndex(uid) => service.delete_index(&uid).await.map(Message::TaskQueued),
        Command::AddDocuments(value) => {
            require_index(index.as_deref())
                .and_then_async(|uid| async {
                    service
                        .add_documents(uid, &value)
                        .await
                        .map(Message::TaskQueued)
                })
                .await
        }
        Command::UpdateDocuments(value) => {
            require_index(index.as_deref())
                .and_then_async(|uid| async {
                    service
                        .update_documents(uid, &value)
                        .await
                        .map(Message::TaskQueued)
                })
                .await
        }
        Command::DeleteDocument { id } => {
            require_index(index.as_deref())
                .and_then_async(|uid| async {
                    service
                        .delete_document(uid, &id)
                        .await
                        .map(Message::TaskQueued)
                })
                .await
        }
        Command::UpdateSettings(value) => {
            require_index(index.as_deref())
                .and_then_async(|uid| async {
                    service
                        .update_settings(uid, &value)
                        .await
                        .map(Message::TaskQueued)
                })
                .await
        }
        Command::FetchTasks(filter) => service.tasks(&filter).await.map(Message::Tasks),
        Command::CancelTask(uid) => service.cancel_task(uid).await.map(Message::TaskQueued),
        Command::FetchKeys(offset) => service.keys(offset, 20).await.map(Message::Keys),
        Command::CreateKey(key) => service.create_key(&key).await.map(Message::KeyCreated),
        Command::UpdateKey {
            uid,
            name,
            description,
        } => service
            .update_key(&uid, &name, &description)
            .await
            .map(Message::KeyUpdated),
        Command::DeleteKey(uid) => service.delete_key(&uid).await.map(|()| Message::KeyDeleted),
        Command::CreateDump => service.create_dump().await.map(Message::TaskQueued),
        Command::Connect { .. } => unreachable!(),
    };
    match result {
        Ok(message @ Message::TaskQueued(_)) => {
            let uid = if let Message::TaskQueued(task) = &message {
                task.task_uid
            } else {
                0
            };
            let _ = tx.send(message);
            poll_task(service, uid, tx).await;
        }
        Ok(message) => {
            let _ = tx.send(message);
        }
        Err(error) => {
            let _ = tx.send(Message::Error(error.to_string()));
        }
    }
}

async fn poll_task(
    service: Arc<HttpService>,
    uid: u64,
    tx: &tokio::sync::mpsc::UnboundedSender<Message>,
) {
    for _ in 0..150 {
        tokio::time::sleep(Duration::from_millis(400)).await;
        match service.task(uid).await {
            Ok(task) if matches!(task.status.as_str(), "succeeded" | "failed" | "canceled") => {
                let _ = tx.send(Message::TaskFinished(task));
                break;
            }
            Ok(_) => {}
            Err(error) => {
                let _ = tx.send(Message::Error(error.to_string()));
                break;
            }
        }
    }
}

fn require_index(index: Option<&str>) -> meili::Result<&str> {
    index.ok_or_else(|| meili::Error::InvalidRequest("select an index first".into()))
}

trait AndThenAsync<T> {
    async fn and_then_async<U, F, Fut>(self, f: F) -> meili::Result<U>
    where
        F: FnOnce(T) -> Fut,
        Fut: std::future::Future<Output = meili::Result<U>>;
}
impl<T> AndThenAsync<T> for meili::Result<T> {
    async fn and_then_async<U, F, Fut>(self, f: F) -> meili::Result<U>
    where
        F: FnOnce(T) -> Fut,
        Fut: std::future::Future<Output = meili::Result<U>>,
    {
        match self {
            Ok(value) => f(value).await,
            Err(error) => Err(error),
        }
    }
}
