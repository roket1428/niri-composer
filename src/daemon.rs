use std::ffi::OsString;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Seek, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use niri_ipc::Event;
use niri_ipc::state::{EventStreamState, EventStreamStatePart};

use notify::RecursiveMode;
use notify_debouncer_full::{DebouncedEvent, new_debouncer};

use thingbuf::mpsc::errors::TryRecvError;
use thingbuf::mpsc::{Receiver, Sender, channel};
use tokio::sync::Notify;
use tokio::task::JoinSet;
use tracing::{debug, info};

use crate::config::Config;
use crate::errors::{Context, DaemonError, LockError, ServerError, TaskError};
use crate::socket::{BidiSocket, EventSocket};

async fn server(dir: PathBuf, config: Config) -> Result<(), ServerError> {
    let main_bell = Arc::new(Notify::new());
    let event_bell = main_bell.clone();
    let config_bell = main_bell.clone();

    let (event_tx, event_rx) = channel::<Option<Event>>(64);
    let (config_tx, config_rx) = channel::<Config>(4);
    let (dispatch_tx, dispatch_rx) = channel::<String>(4);

    let event_socket = EventSocket::connect().await?;
    let bidi_socket = BidiSocket::connect().await?;

    let mut set = JoinSet::new();

    #[rustfmt::skip]
    set.spawn(async move { main_task(config, bidi_socket, event_rx, config_rx, dispatch_rx, main_bell).await });
    set.spawn(async move { event_task(event_socket, event_tx, event_bell).await });
    set.spawn(async move { config_task(dir, config_tx, config_bell).await });
    // set.spawn(async move { dispatch_task(dispatch_tx).await });

    match set.join_next().await {
        Some(result) => match result {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(ServerError::from(e)),
            Err(e) => Err(ServerError::from(e)),
        },
        None => Ok(()),
    }
}

async fn event_task(
    mut socket: EventSocket,
    event_tx: Sender<Option<Event>>,
    bell: Arc<Notify>,
) -> Result<(), TaskError> {
    info!("Event task started");
    loop {
        let event = socket.read_event().await?;
        let mut slot = event_tx
            .send_ref()
            .await
            .map_err(|_| TaskError::ChannelClosed { channel: "event" })?;

        *slot = Some(event);
        bell.notify_one();
    }
}

async fn main_task(
    mut config: Config,
    socket: BidiSocket,
    event_rx: Receiver<Option<Event>>,
    config_rx: Receiver<Config>,
    dispatch_rx: Receiver<String>,
    bell: Arc<Notify>,
) -> Result<(), TaskError> {
    info!("Main task started");
    let mut state = EventStreamState::default();
    loop {
        loop {
            match event_rx.try_recv_ref() {
                Ok(mut slot) => {
                    if let Some(event) = std::mem::take(&mut *slot) {
                        debug!("State update: {:?}", &event);
                        state.apply(event);
                    }
                }
                Err(TryRecvError::Closed) => {
                    return Err(TaskError::ChannelClosed { channel: "event" });
                }
                Err(_) => break,
            }
        }

        match config_rx.try_recv_ref() {
            Ok(mut slot) => {
                config = std::mem::take(&mut *slot);
                debug!("Config updated in main");
            }
            Err(TryRecvError::Closed) => {
                return Err(TaskError::ChannelClosed { channel: "config" });
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
                    info!("dispatch eval will run");
                }
            }
            Err(TryRecvError::Closed) => {
                return Err(TaskError::ChannelClosed {
                    channel: "dispatch",
                });
            }
            _ => {}
        }

        bell.notified().await;
    }
}

async fn config_task(
    dir: PathBuf,
    config_tx: Sender<Config>,
    bell: Arc<Notify>,
) -> Result<(), TaskError> {
    info!("Config task started");
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
    })?;

    watcher.watch(&dir, RecursiveMode::NonRecursive)?;
    info!("Watching for changes: {:?}", &dir);

    let path = {
        let mut path = dir;
        path.push(Config::FILE);
        path
    };

    loop {
        match rx.recv().await {
            Some(_) => {
                debug!("Received config change signal");
                let mut slot = config_tx
                    .send_ref()
                    .await
                    .map_err(|_| TaskError::ChannelClosed { channel: "config" })?;

                if let Ok(config) = Config::validate(&path) {
                    *slot = config;
                    bell.notify_one();
                    debug!("Sending parsed config to main");
                }
            }
            None => {
                return Err(TaskError::ChannelClosed { channel: "notify" });
            }
        }
    }
}

// async fn dispatch_task(dispatch_tx: Sender<String>, bell: Arc<Notify>) -> Result<()> {
//     // TODO
//     // handle listening to our unix socket and accept
//     // niri-composer dispatch <name> calls from the client,
//     // will pass dispatch names to main task

//     Ok(())
// }

pub fn run(maybe_config: Option<PathBuf>) -> Result<(), DaemonError> {
    let _lock = acquire_lock()?;
    let (path, dir) = Config::prepare(maybe_config)?;
    let config = Config::validate(&path)?;
    info!("Loading config at: {:?}", &path);

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(server(dir, config))?;

    Ok(())
}

pub fn acquire_lock() -> Result<File, LockError> {
    let mut lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open("/tmp/niri-composer.lock")
        .context("Couldn't open the lock file")?;

    match lock_file.try_lock() {
        Ok(()) => {
            lock_file.set_len(0)?;
            lock_file.rewind()?;
            writeln!(lock_file, "{}", std::process::id())?;
            info!("Lock acquired, PID = {}", std::process::id());
            Ok(lock_file)
        }
        Err(TryLockError::WouldBlock) => Err(LockError::AlreadyRunning),
        Err(e) => Err(LockError::from(e)),
    }
}
