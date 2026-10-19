use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use miette::{IntoDiagnostic, Result, bail};

use niri_ipc::Event;
use niri_ipc::state::{EventStreamState, EventStreamStatePart};

use notify::RecursiveMode;
use notify_debouncer_full::{DebouncedEvent, new_debouncer};

use thingbuf::mpsc::errors::TryRecvError;
use thingbuf::mpsc::{Receiver, Sender, channel};
use tokio::task::{JoinSet, yield_now};

use crate::config::Config;
use crate::socket::{BidiSocket, EventSocket};

pub async fn daemon(dir: PathBuf, config: Config) -> Result<()> {
    let (event_tx, event_rx) = channel::<Option<Event>>(64);
    let (config_tx, config_rx) = channel::<Config>(4);
    let (dispatch_tx, dispatch_rx) = channel::<String>(4);

    let event_socket = EventSocket::connect().await?;
    let bidi_socket = BidiSocket::connect().await?;

    let mut set = JoinSet::new();
    set.spawn(async move { event_task(event_socket, event_tx).await });
    set.spawn(
        async move { main_task(config, bidi_socket, event_rx, config_rx, dispatch_rx).await },
    );
    set.spawn(async move { config_task(dir, config_tx).await });
    set.spawn(async move { dispatch_task(dispatch_tx).await });

    match set.join_next().await {
        // ugly
        Some(result) => match result.into_diagnostic() {
            Ok(Err(e)) | Err(e) => Err(e),
            Ok(Ok(_)) => Ok(()),
        },
        None => Ok(()),
    }
}

async fn event_task(mut socket: EventSocket, event_tx: Sender<Option<Event>>) -> Result<()> {
    loop {
        let event = socket.read_event().await?;
        let mut slot = match event_tx.send_ref().await {
            Ok(slot) => slot,
            Err(_) => {
                bail!("event channel is closed")
            }
        };
        // we could directly parse incoming events to channel slot
        // to do one less copy but that breaks the current abstraction
        // layers, so I need to design something suitable for that later
        *slot = Some(event);
    }
}

async fn main_task(
    mut config: Config,
    socket: BidiSocket,
    event_rx: Receiver<Option<Event>>,
    config_rx: Receiver<Config>,
    dispatch_rx: Receiver<String>,
) -> Result<()> {
    let mut state = EventStreamState::default();
    loop {
        loop {
            match event_rx.try_recv_ref() {
                Ok(mut slot) => {
                    if let Some(event) = std::mem::take(&mut *slot) {
                        state.apply(event);
                    }
                }
                Err(TryRecvError::Closed) => {
                    bail!("event channel is closed")
                }
                Err(_) => break,
            }
        }

        match config_rx.try_recv_ref() {
            Ok(mut slot) => {
                config = std::mem::take(&mut *slot);
            }
            Err(TryRecvError::Closed) => {
                bail!("config channel is closed")
            }
            _ => {}
        }

        match dispatch_rx.try_recv_ref() {
            Ok(slot) => {
                if let Some(dispatch) = config
                    .dispatches
                    .iter()
                    .find(|dispatch| dispatch.name == *slot)
                {
                    // TODO
                    // eval dispatch here
                }
            }
            Err(TryRecvError::Closed) => {
                bail!("dispatch channel is closed")
            }
            _ => {}
        }

        yield_now().await;
    }
}

async fn config_task(dir: PathBuf, config_tx: Sender<Config>) -> Result<()> {
    let (tx, rx) = channel::<()>(1);
    let mut watcher = new_debouncer(Duration::from_secs(1), None, move |notif| {
        let events: Vec<DebouncedEvent> = match notif {
            Ok(events) => events,
            Err(_) => return,
        };

        let file = OsString::from(Config::FILE);
        let should_reload = events.iter().any(|event| {
            (event.kind.is_modify() || event.kind.is_create())
                && event
                    .paths
                    .iter()
                    .any(|p| p.file_name() == Some(file.as_os_str()))
        });

        if should_reload && tx.is_empty() {
            let _ = tx.try_send(());
        }
    })
    .into_diagnostic()?;

    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .into_diagnostic()?;

    let path = {
        let mut path = dir;
        path.push(Config::FILE);
        path
    };

    loop {
        match rx.recv().await {
            Some(_) => {
                let mut slot = match config_tx.send_ref().await {
                    Ok(slot) => slot,
                    Err(_) => {
                        bail!("config channel is closed")
                    }
                };
                if let Ok(config) = Config::validate(&path) {
                    *slot = config;
                }
            }
            None => {
                bail!("notify channel is closed")
            }
        }
    }
}

async fn dispatch_task(dispatch_tx: Sender<String>) -> Result<()> {
    // TODO
    // handle listening to our unix socket and accept
    // niri-composer dispatch <name> calls from the client,
    // will pass dispatch names to main task

    Ok(())
}
