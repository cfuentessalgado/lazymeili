# mtui

`mtui` is an English-language terminal UI for managing several Meilisearch applications. It follows the core workflows in [`meilisearch-ui`](https://github.com/eyeix/meilisearch-ui) without requiring a browser or CORS configuration.

## Status

The first release scope includes:

- saved applications and fast switching
- health, version, global stats, and index stats
- index create, primary-key update, and guarded deletion
- document search, JSON/table previews, partial update, upload, and deletion
- complete index settings as JSON
- task filtering, live refresh, details, and cancellation
- API key create, view, update, and guarded deletion
- server-side dump creation and task tracking

It does not manage snapshots, task-history deletion, batches, chat workspaces, search rules, webhooks, networks, logs, experimental features, export, or compaction.

## Install

Download the archive for your platform from GitHub Releases:

- `aarch64-apple-darwin` for Apple Silicon macOS
- `x86_64-unknown-linux-gnu` for x86_64 glibc Linux

Verify its adjacent SHA-256 file, extract `mtui`, and move it to a directory in `PATH`.

```sh
shasum -a 256 -c mtui-*.tar.gz.sha256
sudo install -m 0755 mtui /usr/local/bin/mtui
mtui
```

## Requirements

- A terminal with color and alternate-screen support.
- `$VISUAL` or `$EDITOR` for documents, advanced search forms, key forms, and settings changes.
- macOS Keychain or Linux Secret Service for preferred credential storage.

If Linux Secret Service is unavailable, `mtui` creates an `age` passphrase-encrypted fallback vault. The vault is unlocked before the TUI starts. There is no passphrase recovery. To discard it and all secrets in it, run `mtui --reset-vault` and type the requested confirmation.

## Configuration and secrets

Non-secret application metadata is a versioned TOML file in the platform config directory:

- macOS: `~/Library/Application Support/dev.mtui.mtui/config.toml`
- Linux: `${XDG_CONFIG_HOME:-~/.config}/mtui/config.toml` (the exact path follows the OS directory API)

API keys are never written to TOML. Native stores use service name `dev.mtui.mtui` and the stable application UUID as the account. Metadata and fallback-vault files use mode `0600`. Writes use an atomic temporary-file replacement.

Logs and errors do not include API keys. Settings previews hide fields with names such as `apiKey`, `password`, `secret`, and `token`.

## Navigation

| Key | Action |
| --- | --- |
| Arrow keys or `h/j/k/l` | Move selection or change screen |
| `Tab` / `Shift-Tab` | Next / previous screen |
| `Enter` | Open or confirm |
| `n` | New application, index, document upload, or API key |
| `e` | Edit primary key, document, settings, or API key |
| `d` | Delete selected item or cancel a running task |
| `/` | Advanced search JSON, index filter, or task filter |
| `y` | Yank a newly created API key to the clipboard with OSC 52 |
| `a` | Go to application selection |
| `r` | Refresh |
| `PageUp` / `PageDown` | Page results |
| `s`, `t`, `K` | Settings, tasks, API keys |
| `D` | Create a dump after confirmation |
| `?` | Help |
| `q` / `Ctrl-C` | Quit |

Text inputs disable Vim command keys. Destructive operations require typing the target UID or name. Index deletion removes its documents, settings, and task history.

### Search form

Press `/` on Documents. `mtui` opens a JSON form in `$VISUAL` or `$EDITOR`. It supports query, offset/limit, filter, sort expressions, ranking score, ranking score threshold, and hybrid embedder/semantic ratio. Fields unsupported by an older connected server are shown as unavailable in the dashboard and can return a clear server error.

### Task filters

Press `/` on Tasks and enter space-separated filters. Values can be comma-separated:

```text
index=movies,books status=enqueued,processing type=documentAdditionOrUpdate
```

Documents and tasks refresh every seven seconds while their screen is active.

### JSON editing

`mtui` writes a restrictive temporary `.json` file, suspends the alternate screen, starts `$VISUAL` and then `$EDITOR`, validates the saved JSON, deletes the temporary file, and restores the TUI. Settings changes show a secret-redacted unified diff and require an additional confirmation.

A document upload must be a JSON array. Editing one document sends a partial document update. A primary key is required to delete a document.

### API keys

Key creation uses an in-terminal form with `name`, `description`, `actions`, `indexes`, expiration, and a generated `uid`. The UID is only an identifier and cannot authenticate requests. Use Tab or the arrow keys to move between fields. Permission presets provide full access (`*`), read-only observability, read-only access to documents and settings, or minimal search-only access (`search`). The document and settings preset includes search, document reads, index reads, task reads, settings reads, statistics, metrics, and version information. It does not include create, update, delete, or cancel actions. Select Custom or change an action to set permissions manually. Press Enter on Actions to open a multi-select permission list, then use Space to toggle permissions and Enter to confirm the selection. Expiration has 30-day, 180-day, 365-day, and Never presets. Meilisearch can show the secret API key value only once. `mtui` shows that 64-character credential in a dedicated dialog after creation. Press `y` to yank it through the terminal's OSC 52 clipboard feature before closing the dialog. Clear the clipboard after use. Later key lists can show only metadata or a masked value, depending on the server version.

### Dumps

A dump is created on the Meilisearch server. `mtui` tracks the returned task. The standard Meilisearch API cannot list or download dump files. Retrieve the file from the server's configured dump directory.

## Compatibility

`mtui` targets the current stable Meilisearch API. It reads the server version and disables known unsupported actions on older 1.x servers. Compatibility with every historical 1.x release is not guaranteed. Permission errors usually mean the saved API key does not have the action required by the current screen. API key management requires a master key.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run
```

Set `RUST_LOG=mtui=debug` for local diagnostics. Do not put credentials in logs or command-line arguments.

Release tags run GitHub Actions builds for Apple Silicon macOS and x86_64 glibc Linux. Each archive has a SHA-256 checksum.

## License

MIT
