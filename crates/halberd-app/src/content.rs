//! Mounting the game's content on a background thread, so the window opens
//! at once: reading the archives' file lists takes a moment on a slow
//! disk.

use halberd_assets::{GameContent, GmodInstall};
use std::sync::mpsc;

/// Content being mounted in the background.
pub(crate) struct ContentLoader {
    done: mpsc::Receiver<GameContent>,
}

impl ContentLoader {
    /// Starts mounting `install`'s content; `wake` is called when it is
    /// done (to redraw the window).
    pub(crate) fn start(install: GmodInstall, wake: impl Fn() + Send + 'static) -> Self {
        let (send, done) = mpsc::channel();
        std::thread::Builder::new()
            .name("halberd-mount".into())
            .spawn(move || {
                let content = GameContent::mount(&install);
                // The window may already be gone; then nobody needs it.
                send.send(content).ok();
                wake();
            })
            .ok();
        Self { done }
    }

    /// The mounted content, once it is ready (only once).
    pub(crate) fn poll(&self) -> Option<GameContent> {
        self.done.try_recv().ok()
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
            if let Some(content) = loader.poll() {
                break content;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "never arrived");
        };
        assert_eq!(content.folder_count(), 1);
        assert!(loader.poll().is_none(), "handed over once");
    }
}
