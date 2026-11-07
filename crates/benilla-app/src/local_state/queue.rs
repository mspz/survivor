//! State files written on a background thread: flushing to disk takes about 100 ms on some
//! drives, too long for a save made during play. Only the newest contents of a file are kept
//! while it waits, and readers see them at once through [`Queue::get`].
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    sync::{Condvar, Mutex, MutexGuard},
    time::{Duration, Instant},
};

type Write = fn(&Path, &[u8]) -> io::Result<()>;

struct Files {
    waiting: BTreeMap<PathBuf, Vec<u8>>,
    writing: Option<(PathBuf, Vec<u8>)>,
    worker: bool,
    finished: Option<Instant>,
    /// Files wait in memory until [`Queue::hold`] lifts, unless a [`Queue::flush`] needs them.
    held: bool,
    flushing: bool,
}

pub(super) struct Queue {
    files: Mutex<Files>,
    changed: Condvar,
    write: Write,
}

impl Queue {
    pub(super) const fn new(write: Write) -> Self {
        Self {
            files: Mutex::new(Files {
                waiting: BTreeMap::new(),
                writing: None,
                worker: false,
                finished: None,
                held: false,
                flushing: false,
            }),
            changed: Condvar::new(),
            write,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Files> {
        self.files.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn wait<'a>(&self, files: MutexGuard<'a, Files>) -> MutexGuard<'a, Files> {
        self.changed.wait(files).unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn push(&'static self, path: &Path, contents: &[u8]) -> io::Result<()> {
        let mut files = self.lock();
        if !files.worker {
            std::thread::Builder::new()
                .name("save-writer".into())
                .spawn(move || self.run())?;
            files.worker = true;
        }
        files.waiting.insert(path.to_path_buf(), contents.to_vec());
        self.changed.notify_all();
        Ok(())
    }

    fn run(&self) {
        let mut files = self.lock();
        loop {
            let ready = !files.held || files.flushing;
            let Some((path, contents)) = ready.then(|| files.waiting.pop_first()).flatten() else {
                files = self.wait(files);
                continue;
            };
            files.writing = Some((path.clone(), contents.clone()));
            drop(files);
            if let Err(error) = (self.write)(&path, &contents) {
                bevy::log::error!("Could not save {}: {error}", path.display());
            }
            files = self.lock();
            files.writing = None;
            files.finished = Some(Instant::now());
            self.changed.notify_all();
        }
    }

    pub(super) fn hold(&self, held: bool) {
        self.lock().held = held;
        self.changed.notify_all();
    }

    /// The contents still on their way to `path`.
    pub(super) fn get(&self, path: &Path) -> Option<Vec<u8>> {
        let files = self.lock();
        files.waiting.get(path).cloned().or_else(|| {
            let (writing, contents) = files.writing.as_ref()?;
            (writing == path).then(|| contents.clone())
        })
    }

    /// Whether a file is being written, or was within `window`.
    pub(super) fn wrote_within(&self, window: Duration) -> bool {
        let files = self.lock();
        files.writing.is_some() || files.finished.is_some_and(|at| at.elapsed() <= window)
    }

    /// Wait until everything queued is on disk.
    pub(super) fn flush(&self) {
        let mut files = self.lock();
        files.flushing = true;
        self.changed.notify_all();
        while !files.waiting.is_empty() || files.writing.is_some() {
            files = self.wait(files);
        }
        files.flushing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    static HOLD: AtomicBool = AtomicBool::new(true);
    static WRITES: Mutex<Vec<(PathBuf, Vec<u8>)>> = Mutex::new(Vec::new());
    static STARTED: AtomicUsize = AtomicUsize::new(0);
    static QUEUE: Queue = Queue::new(|path, contents| {
        STARTED.fetch_add(1, Ordering::SeqCst);
        while HOLD.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        WRITES
            .lock()
            .unwrap()
            .push((path.to_path_buf(), contents.to_vec()));
        Ok(())
    });

    static HELD_WRITES: AtomicUsize = AtomicUsize::new(0);
    static HELD: Queue = Queue::new(|_, _| {
        HELD_WRITES.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });

    #[test]
    fn held_files_stay_readable_in_memory_until_released_or_flushed() {
        let path = Path::new("held");
        HELD.hold(true);
        HELD.push(path, b"one").unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(HELD_WRITES.load(Ordering::SeqCst), 0);
        assert_eq!(HELD.get(path).unwrap(), b"one");
        HELD.flush();
        assert_eq!(HELD_WRITES.load(Ordering::SeqCst), 1);
        HELD.push(path, b"two").unwrap();
        HELD.hold(false);
        while HELD_WRITES.load(Ordering::SeqCst) < 2 {
            std::thread::sleep(Duration::from_millis(1));
        }
        HELD.flush();
        assert!(HELD.get(path).is_none());
    }

    #[test]
    fn readers_see_queued_contents_and_only_the_newest_version_is_written() {
        let (first, second) = (Path::new("first"), Path::new("second"));
        QUEUE.push(first, b"busy").unwrap();
        while STARTED.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(QUEUE.get(first).unwrap(), b"busy");
        QUEUE.push(second, b"old").unwrap();
        QUEUE.push(second, b"new").unwrap();
        assert_eq!(QUEUE.get(second).unwrap(), b"new");
        HOLD.store(false, Ordering::SeqCst);
        QUEUE.flush();
        assert!(QUEUE.get(first).is_none() && QUEUE.get(second).is_none());
        assert_eq!(
            *WRITES.lock().unwrap(),
            [
                (first.to_path_buf(), b"busy".to_vec()),
                (second.to_path_buf(), b"new".to_vec())
            ]
        );
    }
}
