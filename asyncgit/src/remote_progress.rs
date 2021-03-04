//!

use crate::{
    error::Result, sync::remotes::push::ProgressNotification,
    AsyncNotification,
};
use crossbeam_channel::{Receiver, Sender};
use git2::PackBuilderStage;
use std::{
    cmp,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

///
#[derive(Clone, Debug)]
pub enum RemoteProgressState {
    ///
    PackingAddingObject,
    ///
    PackingDeltafiction,
    ///
    Pushing,
    /// fetch progress
    Transfer,
    /// remote progress done
    Done,
}

///
#[derive(Clone, Debug)]
pub struct RemoteProgress {
    ///
    pub state: RemoteProgressState,
    /// percent 0..100
    pub progress: u8,
}

impl RemoteProgress {
    ///
    pub fn new(
        state: RemoteProgressState,
        current: usize,
        total: usize,
    ) -> Self {
        let total = cmp::max(current, total) as f32;
        let progress = current as f32 / total * 100.0;
        let progress = progress as u8;
        Self { state, progress }
    }

    pub(crate) fn set_progress(
        progress: Arc<Mutex<Option<ProgressNotification>>>,
        state: Option<ProgressNotification>,
    ) -> Result<()> {
        let mut progress = progress.lock()?;

        *progress = state;

        Ok(())
    }

    /// spawn thread to listen to progress notifcations coming in from blocking remote git method (fetch/push)
    pub(crate) fn spawn_receiver_thread(
        notification_type: AsyncNotification,
        sender: Sender<AsyncNotification>,
        receiver: Receiver<ProgressNotification>,
        progress: Arc<Mutex<Option<ProgressNotification>>>,
    ) -> JoinHandle<()> {
        thread::spawn(move || loop {
            let incoming = receiver.recv();
            match incoming {
                Ok(update) => {
                    Self::set_progress(
                        progress.clone(),
                        Some(update.clone()),
                    )
                    .expect("set prgoress failed");
                    sender
                        .send(notification_type)
                        .expect("Notification error");

                    //NOTE: for better debugging
                    thread::sleep(Duration::from_millis(1));

                    if let ProgressNotification::Done = update {
                        break;
                    }
                }
                Err(e) => {
                    log::error!(
                        "push progress receiver error: {}",
                        e
                    );
                    break;
                }
            }
        })
    }
}

impl From<ProgressNotification> for RemoteProgress {
    fn from(progress: ProgressNotification) -> Self {
        match progress {
            ProgressNotification::Packing {
                stage,
                current,
                total,
            } => match stage {
                PackBuilderStage::AddingObjects => {
                    RemoteProgress::new(
                        RemoteProgressState::PackingAddingObject,
                        current,
                        total,
                    )
                }
                PackBuilderStage::Deltafication => {
                    RemoteProgress::new(
                        RemoteProgressState::PackingDeltafiction,
                        current,
                        total,
                    )
                }
            },
            ProgressNotification::PushTransfer {
                current,
                total,
                ..
            } => RemoteProgress::new(
                RemoteProgressState::Pushing,
                current,
                total,
            ),
            ProgressNotification::Transfer {
                objects,
                total_objects,
                ..
            } => RemoteProgress::new(
                RemoteProgressState::Transfer,
                objects,
                total_objects,
            ),
            _ => RemoteProgress::new(RemoteProgressState::Done, 1, 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_progress::RemoteProgressState;

    #[test]
    fn test_progress_zero_total() {
        let prog =
            RemoteProgress::new(RemoteProgressState::Pushing, 1, 0);

        assert_eq!(prog.progress, 100);
    }

    #[test]
    fn test_progress_rounding() {
        let prog =
            RemoteProgress::new(RemoteProgressState::Pushing, 2, 10);

        assert_eq!(prog.progress, 20);
    }
}