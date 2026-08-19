use std::time::Duration;

use crossterm::event::{Event as CrosstermEvent, EventStream};
use futures::StreamExt;

#[derive(Debug)]
pub enum Event {
    Input(CrosstermEvent),
    Tick,
}

pub struct Events {
    reader: EventStream,
    tick: tokio::time::Interval,
}

impl Events {
    #[must_use]
    pub fn new(rate: Duration) -> Self {
        Self {
            reader: EventStream::new(),
            tick: tokio::time::interval(rate),
        }
    }

    pub async fn next(&mut self) -> anyhow::Result<Event> {
        tokio::select! {
            _ = self.tick.tick() => Ok(Event::Tick),
            maybe = self.reader.next() => match maybe {
                Some(Ok(event)) => Ok(Event::Input(event)),
                Some(Err(error)) => Err(error.into()),
                None => anyhow::bail!("terminal input stream closed"),
            },
        }
    }
}
