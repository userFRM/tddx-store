//! Change notifications pushed to the webview.
//!
//! The UI refreshes when the backend says something changed, instead of
//! asking on a timer. Three sources:
//!
//! - commands that change the queue or the schedules announce it;
//! - the worker pool already streams `tdds:progress` per task;
//! - a file watcher on the output directory reports the library
//!   changing, including from outside the app (Finder, a script).
//!
//! Payloads are empty: each event means "re-read this", and the reader
//! already knows how.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use tauri::Emitter;

use crate::state::AppState;

pub const QUEUE_CHANGED: &str = "tdds:queue-changed";
pub const LIBRARY_CHANGED: &str = "tdds:library-changed";
pub const SCHEDULES_CHANGED: &str = "tdds:schedules-changed";

/// A download writes many files in a burst; one notice per quiet half
/// second is enough for a view to follow along.
const LIBRARY_DEBOUNCE: Duration = Duration::from_millis(500);

impl AppState {
    /// Tell the webview `event` happened. A no-op before the app handle
    /// is set, and when no window is listening.
    pub fn notify(&self, event: &str) {
        if let Some(app) = self.app.get() {
            if let Err(e) = app.emit(event, ()) {
                tracing::debug!(%event, error = %e, "emit failed");
            }
        }
    }

    /// Pass a command's result through, announcing `event` if it
    /// succeeded.
    pub fn notify_ok<T>(&self, event: &str, result: Result<T, String>) -> Result<T, String> {
        if result.is_ok() {
            self.notify(event);
        }
        result
    }
}

/// Watch `dir` recursively, replacing any previous watch. Failure is
/// logged, not fatal: the Library still refreshes on the app's own
/// changes and on the fallback check, just not on outside edits.
pub fn watch_output_dir(state: &Arc<AppState>, dir: &str) {
    let path = PathBuf::from(dir);
    if dir.is_empty() {
        return;
    }
    if let Err(e) = std::fs::create_dir_all(&path) {
        tracing::warn!(dir = %path.display(), error = %e, "library watch: cannot create output dir");
        return;
    }
    let weak = Arc::downgrade(state);
    let debouncer = new_debouncer(LIBRARY_DEBOUNCE, move |res: DebounceEventResult| {
        let Some(state) = weak.upgrade() else { return };
        match res {
            Ok(events) => {
                tracing::debug!(paths = events.len(), "library changed");
                // The footprint cache is only as fresh as the files; a
                // change on disk makes it wrong now, not in 15 s.
                *state.disk_usage.blocking_lock() = None;
                state.notify(LIBRARY_CHANGED);
            }
            Err(e) => tracing::warn!(error = %e, "library watch error"),
        }
    });
    let mut debouncer = match debouncer {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(error = %e, "library watch: cannot start watcher");
            return;
        }
    };
    if let Err(e) = debouncer.watcher().watch(&path, RecursiveMode::Recursive) {
        tracing::warn!(dir = %path.display(), error = %e, "library watch: cannot watch");
        return;
    }
    // Dropping the previous debouncer stops its watch.
    *state.watcher.lock().unwrap_or_else(|p| p.into_inner()) = Some(debouncer);
    tracing::info!(dir = %path.display(), "watching library");
}
