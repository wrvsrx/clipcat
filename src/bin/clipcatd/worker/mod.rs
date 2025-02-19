use std::sync::Arc;

use snafu::ResultExt;
use tokio::sync::{mpsc, Mutex};

use clipcat::{ClipboardManager, ClipboardMonitor};

use crate::{
    config::Config,
    error::{self, Error},
    history::{HistoryDriver, HistoryManager},
};

mod clipboard;
mod grpc;
mod signal;

pub enum CtlMessage {
    Shutdown,
}

pub type CtlMessageSender = mpsc::UnboundedSender<CtlMessage>;

#[allow(clippy::never_loop)]
pub async fn start(config: Config) -> Result<(), Error> {
    let grpc_addr = format!("{}:{}", config.grpc.host, config.grpc.port)
        .parse()
        .context(error::ParseSockAddrSnafu)?;

    let (clipboard_manager, history_manager) = {
        let file_path = config.history_file_path;

        tracing::info!("History file path: {:?}", file_path);
        let history_manager =
            HistoryManager::new(&file_path).context(error::CreateHistoryManagerSnafu)?;

        tracing::info!("Load history from {:?}", history_manager.path());
        let history_clips = history_manager
            .load()
            .context(error::LoadHistoryManagerSnafu)?;
        let clip_count = history_clips.len();
        tracing::info!("{} clip(s) loaded", clip_count);

        tracing::info!(
            "Initialize ClipboardManager with capacity {}",
            config.max_history
        );
        let mut clipboard_manager = ClipboardManager::with_capacity(config.max_history)
            .context(error::CreateClipboardManagerSnafu)?;

        tracing::info!("Import {} clip(s) into ClipboardManager", clip_count);
        clipboard_manager.import(&history_clips);

        (
            Arc::new(Mutex::new(clipboard_manager)),
            Arc::new(Mutex::new(history_manager)),
        )
    };

    let (ctl_tx, mut ctl_rx) = mpsc::unbounded_channel::<CtlMessage>();

    let _signal_join = signal::start(ctl_tx.clone());

    let monitor_opts = config.monitor.into();
    let clipboard_monitor = {
        let monitor =
            ClipboardMonitor::new(monitor_opts).context(error::CreateClipboardMonitorSnafu)?;
        Arc::new(Mutex::new(monitor))
    };

    let (clip_tx, clipboard_join) = clipboard::start(
        ctl_tx.clone(),
        clipboard_monitor.clone(),
        clipboard_manager.clone(),
        history_manager,
    );
    let (grpc_tx, grpc_join) = grpc::start(grpc_addr, clipboard_monitor, clipboard_manager);

    while let Some(msg) = ctl_rx.recv().await {
        match msg {
            CtlMessage::Shutdown => {
                let _ = clip_tx.send(clipboard::Message::Shutdown);
                let _ = grpc_tx.send(grpc::Message::Shutdown);
                break;
            }
        }
    }

    let _ = grpc_join.await;
    tracing::info!("gRPC service is down");

    let _ = clipboard_join.await;
    tracing::info!("ClipboardWorker is down");

    Ok(())
}
