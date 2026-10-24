//! Mounting the game's content on a background thread, so the window opens
//! at once: reading the archives' file lists takes a moment on a slow
//! disk.

use halberd_assets::{GameContent, GmodInstall, material_names};
use std::sync::mpsc;

/// Content being mounted in the background.
pub(crate) struct ContentLoader {
    /// `None` if the background thread could not be started.
    done: Option<mpsc::Receiver<Mounted>>,
}

/// Mounted content, with the names of its materials (listing them takes
/// a moment too).
pub(crate) struct Mounted {
    pub(crate) content: GameContent,
    pub(crate) materials: Vec<String>,
}

/// How mounting is going.
pub(crate) enum Mounting {
    /// Still reading.
    Busy,
    /// Done.
    Ready(Mounted),
    /// The background work stopped without a result (it could not start,
    /// or it failed).
    Failed,
}

impl ContentLoader {
    /// Starts mounting `install`'s content; `wake` is called when it is
    /// done (to redraw the window).
    pub(crate) fn start(install: GmodInstall, wake: impl Fn() + Send + 'static) -> Self {
        let (send, done) = mpsc::channel();
        let started = std::thread::Builder::new()
            .name("halberd-mount".into())
            .spawn(move || {
                let content = GameContent::mount(&install);
                let materials = material_names(&content);
                // The window may already be gone; then nobody needs it.
                send.send(Mounted { content, materials }).ok();
                // Let go of the channel before waking the window, so a poll
                // after the result never sees the work as still going.
                drop(send);
                wake();
            });
        Self {
            done: started.is_ok().then_some(done),
        }
    }

    /// How mounting is going. Gives the content once; after that, and if
    /// the work stopped without a result, says it failed.
    pub(crate) fn poll(&self) -> Mounting {
        let Some(done) = &self.done else {
            return Mounting::Failed;
        };
        match done.try_recv() {
            Ok(content) => Mounting::Ready(content),
            Err(mpsc::TryRecvError::Empty) => Mounting::Busy,
            Err(mpsc::TryRecvError::Disconnected) => Mounting::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halberd_assets::{FoundBy, inspect_gmod_dir};
    use std::time::{Duration, Instant};

    #[test]
    fn content_is_mounted_in_the_background_and_handed_over_once() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("garrysmod");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(
            game.join("gameinfo.txt"),
            "\"GameInfo\" { \"FileSystem\" { \"SearchPaths\" { \"game\" \"|gameinfo_path|.\" } } }",
        )
        .unwrap();
        let install = inspect_gmod_dir(dir.path(), FoundBy::UserChoice).unwrap();
        let (woke, waker) = mpsc::channel();
        let loader = ContentLoader::start(install, move || {
            woke.send(()).ok();
        });
        waker.recv_timeout(Duration::from_secs(10)).unwrap();
        let start = Instant::now();
        let content = loop {
            if let Mounting::Ready(mounted) = loader.poll() {
                assert!(mounted.materials.is_empty());
                break mounted.content;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "never arrived");
        };
        assert_eq!(content.folder_count(), 1);
        assert!(
            matches!(loader.poll(), Mounting::Failed),
            "handed over once"
        );
    }

    #[test]
    fn work_that_stops_without_a_result_is_reported_as_failed() {
        // As when the mounting thread panics: its end of the channel goes.
        let (send, done) = mpsc::channel::<Mounted>();
        drop(send);
        let loader = ContentLoader { done: Some(done) };
        assert!(matches!(loader.poll(), Mounting::Failed));
        let never_started = ContentLoader { done: None };
        assert!(matches!(never_started.poll(), Mounting::Failed));
    }
}
