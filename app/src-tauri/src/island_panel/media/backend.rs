//! The seam between the media logic and the OS. `MediaBackend` is implemented by the real Windows
//! System Media Transport Controls adapter (smtc.rs). `Controller` holds everything that is decisions,
//! not OS calls: state mapping, which controls may be used, and the cover cache.
use super::{
    art,
    state::{Action, MediaState, Snapshot, map_snapshot},
};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

/// Wall-clock Unix milliseconds (what the OS stamps timeline updates with).
pub fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Called by the backend (from any thread) when something about the session changed.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// Created and used on the worker thread only, so it does not need to be `Send` (the real
/// WinRT objects may not be).
pub trait MediaBackend {
    /// Read the current state of the system's current media session.
    fn snapshot(&mut self) -> Result<Snapshot, String>;
    /// Raw bytes of the current session's thumbnail, or `None` if it has none (yet).
    fn thumbnail(&mut self) -> Result<Option<Vec<u8>>, String>;
    /// Send a control to the current session. `Ok(false)` means the app declined it.
    fn control(&mut self, action: Action, seek_to_ms: Option<i64>) -> Result<bool, String>;
    /// Start delivering change notifications (event driven; no polling).
    fn subscribe(&mut self, wake: Wake) -> Result<(), String>;
    /// Stop them and release the session.
    fn unsubscribe(&mut self);
}

/// A player often announces a track before its thumbnail is ready, so a missing cover is retried,
/// but not on every request.
pub const COVER_RETRY_MS: u64 = 3000;
pub const COVER_CACHE: usize = 8;

enum Cover {
    Have(String),
    /// Tried at this time (ms) and found none.
    Missed(u64),
}

pub struct Controller {
    backend: Box<dyn MediaBackend>,
    last: MediaState,
    covers: HashMap<String, Cover>,
    order: VecDeque<String>,
}

impl Controller {
    pub fn new(backend: Box<dyn MediaBackend>) -> Self {
        Self {
            backend,
            last: MediaState::empty(0),
            covers: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    #[cfg(test)]
    pub fn last(&self) -> &MediaState {
        &self.last
    }

    pub fn subscribe(&mut self, wake: Wake) -> Result<(), String> {
        self.backend.subscribe(wake)
    }

    pub fn unsubscribe(&mut self) {
        self.backend.unsubscribe();
    }

    /// Re-read the session. No session at all is a normal state, not an error.
    pub fn refresh(&mut self, now_unix_ms: i64) -> Result<MediaState, String> {
        let snap = self.backend.snapshot()?;
        self.last = map_snapshot(&snap, now_unix_ms);
        Ok(self.last.clone())
    }

    fn remember(&mut self, key: &str, cover: Cover) {
        if self.covers.insert(key.to_string(), cover).is_none() {
            self.order.push_back(key.to_string());
            while self.order.len() > COVER_CACHE {
                if let Some(old) = self.order.pop_front() {
                    self.covers.remove(&old);
                }
            }
        }
    }

    /// The cover for `key` as a data URL. Cached by track key, so the same cover is not read twice.
    /// Only the *current* track has a readable cover, so a stale key returns `None`.
    pub fn cover(
        &mut self,
        key: &str,
        now_ms: u64,
        now_unix_ms: i64,
    ) -> Result<Option<String>, String> {
        if !self.last.has_session || self.last.track_key != key || !self.last.has_thumbnail {
            return Ok(None);
        }
        match self.covers.get(key) {
            Some(Cover::Have(url)) => return Ok(Some(url.clone())),
            Some(Cover::Missed(at)) if now_ms.saturating_sub(*at) < COVER_RETRY_MS => {
                return Ok(None);
            }
            _ => {}
        }
        let bytes = self.backend.thumbnail()?;
        // The track may have changed while we were reading: never file one track's cover under
        // another track's key.
        let still_current = self.refresh(now_unix_ms)?.track_key == key;
        let url = bytes.as_deref().and_then(art::data_url);
        match (url, still_current) {
            (Some(url), true) => {
                self.remember(key, Cover::Have(url.clone()));
                Ok(Some(url))
            }
            (_, true) => {
                self.remember(key, Cover::Missed(now_ms));
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Run a transport control, but only if the session says it is enabled right now.
    pub fn act(
        &mut self,
        action: Action,
        seek_to_ms: Option<i64>,
        now_unix_ms: i64,
    ) -> Result<bool, String> {
        let st = self.refresh(now_unix_ms)?;
        if !st.has_session {
            return Err("nothing is playing".into());
        }
        let c = st.controls;
        let seek_to = match action {
            Action::TogglePlayPause if !c.can_play_pause() => {
                return Err("this app does not support play/pause".into());
            }
            Action::Next if !c.next => return Err("this app does not support next".into()),
            Action::Previous if !c.previous => {
                return Err("this app does not support previous".into());
            }
            Action::Seek => {
                let (true, Some(t)) = (c.seek, st.timeline) else {
                    return Err("this app does not support seeking".into());
                };
                let to = seek_to_ms.ok_or("a position is required")?;
                Some(to.clamp(0, t.duration_ms))
            }
            _ => None,
        };
        self.backend.control(action, seek_to)
    }
}
