//! The real backend: Windows System Media Transport Controls (what the volume flyout and the lock
//! screen show). Everything here is a thin translation to `MediaBackend`; decisions live in
//! backend.rs and state.rs, where they are unit tested.
//!
//! Events, not polling: while subscribed, the system calls our handlers when the current session
//! changes or when a session's media properties, playback info or timeline change. Each handler
//! only sends a wake-up; the worker then re-reads the state once.
//!
//! Which session: `GetCurrentSession()` is the one Windows itself considers current (the one the
//! system media flyout controls). We follow whichever that is and re-subscribe when it changes.
use super::{
    art,
    backend::{MediaBackend, Wake},
    state::{Action, Controls, Playback, RawSession, RawTimeline, Snapshot},
};
use windows::{
    Foundation::TypedEventHandler,
    Media::Control::{
        CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
        MediaPropertiesChangedEventArgs, PlaybackInfoChangedEventArgs,
        TimelinePropertiesChangedEventArgs,
    },
    Storage::Streams::DataReader,
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize},
};

const TICKS_PER_MS: i64 = 10_000;
/// Windows DateTime counts 100 ns ticks from 1601-01-01; this is that instant in Unix ms.
const WINDOWS_TO_UNIX_MS: i64 = 11_644_473_600_000;

struct SessionSub {
    session: Session,
    tokens: [i64; 3],
}

pub struct SmtcBackend {
    manager: Manager,
    wake: Option<Wake>,
    manager_token: Option<i64>,
    sub: Option<SessionSub>,
}

impl SmtcBackend {
    /// Must be called on the thread that will use it.
    pub fn new() -> Result<Self, String> {
        // MTA. "Already initialised" (S_FALSE / a different mode) is fine for our purposes.
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let manager = Manager::RequestAsync()
            .and_then(|op| op.join())
            .map_err(|e| format!("media controls are unavailable: {e}"))?;
        Ok(Self {
            manager,
            wake: None,
            manager_token: None,
            sub: None,
        })
    }

    fn current(&self) -> Option<Session> {
        self.manager.GetCurrentSession().ok()
    }

    fn drop_session_sub(&mut self) {
        if let Some(sub) = self.sub.take() {
            let _ = sub.session.RemoveMediaPropertiesChanged(sub.tokens[0]);
            let _ = sub.session.RemovePlaybackInfoChanged(sub.tokens[1]);
            let _ = sub.session.RemoveTimelinePropertiesChanged(sub.tokens[2]);
        }
    }

    /// Follow the current session: (re)subscribe to its three change events when it is a different
    /// session than before. No-op when not subscribed.
    fn follow(&mut self, current: Option<&Session>) {
        let Some(wake) = self.wake.clone() else {
            return;
        };
        match (&self.sub, current) {
            (Some(s), Some(c)) if &s.session == c => return,
            (None, None) => return,
            _ => {}
        }
        self.drop_session_sub();
        let Some(session) = current else { return };
        let (w1, w2, w3) = (wake.clone(), wake.clone(), wake);
        let t1 = session.MediaPropertiesChanged(&TypedEventHandler::<
            Session,
            MediaPropertiesChangedEventArgs,
        >::new(move |_, _| {
            w1();
            Ok(())
        }));
        let t2 = session.PlaybackInfoChanged(&TypedEventHandler::<
            Session,
            PlaybackInfoChangedEventArgs,
        >::new(move |_, _| {
            w2();
            Ok(())
        }));
        let t3 = session.TimelinePropertiesChanged(&TypedEventHandler::<
            Session,
            TimelinePropertiesChangedEventArgs,
        >::new(move |_, _| {
            w3();
            Ok(())
        }));
        if let (Ok(a), Ok(b), Ok(c)) = (t1, t2, t3) {
            self.sub = Some(SessionSub {
                session: session.clone(),
                tokens: [a, b, c],
            });
        }
    }
}

fn ticks_to_ms(ticks: i64) -> i64 {
    ticks / TICKS_PER_MS
}

fn map_status(s: Status) -> Playback {
    match s {
        Status::Playing => Playback::Playing,
        Status::Paused => Playback::Paused,
        Status::Stopped => Playback::Stopped,
        Status::Opened | Status::Changing => Playback::Busy,
        _ => Playback::Closed,
    }
}

fn read_session(s: &Session) -> windows::core::Result<RawSession> {
    let app_id = s
        .SourceAppUserModelId()
        .map(|h| h.to_string())
        .unwrap_or_default();
    // Some sessions refuse to give properties for a moment; show what we can rather than failing.
    let props = s.TryGetMediaPropertiesAsync().and_then(|op| op.join()).ok();
    let text = |f: &dyn Fn(
        &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
    ) -> windows::core::Result<windows::core::HSTRING>| {
        props
            .as_ref()
            .and_then(|p| f(p).ok())
            .map(|h| h.to_string())
            .unwrap_or_default()
    };
    let title = text(&|p| p.Title());
    let artist = text(&|p| p.Artist());
    let album = text(&|p| p.AlbumTitle());
    let has_thumbnail = props.as_ref().is_some_and(|p| p.Thumbnail().is_ok());

    let info = s.GetPlaybackInfo()?;
    let c = info.Controls()?;
    let flag = |r: windows::core::Result<bool>| r.unwrap_or(false);
    let controls = Controls {
        play: flag(c.IsPlayEnabled()),
        pause: flag(c.IsPauseEnabled()),
        toggle: flag(c.IsPlayPauseToggleEnabled()),
        next: flag(c.IsNextEnabled()),
        previous: flag(c.IsPreviousEnabled()),
        seek: flag(c.IsPlaybackPositionEnabled()),
    };
    let rate = info
        .PlaybackRate()
        .ok()
        .and_then(|r| r.Value().ok())
        .unwrap_or(1.0);
    let timeline = s.GetTimelineProperties().ok().and_then(|t| {
        let updated = t
            .LastUpdatedTime()
            .ok()
            .map(|d| d.UniversalTime)
            .filter(|&u| u > 0);
        Some(RawTimeline {
            position_ms: ticks_to_ms(t.Position().ok()?.Duration),
            start_ms: ticks_to_ms(t.StartTime().ok()?.Duration),
            end_ms: ticks_to_ms(t.EndTime().ok()?.Duration),
            updated_unix_ms: updated.map(|u| ticks_to_ms(u) - WINDOWS_TO_UNIX_MS),
        })
    });
    Ok(RawSession {
        app_id,
        title,
        artist,
        album,
        has_thumbnail,
        status: map_status(info.PlaybackStatus()?),
        controls,
        rate,
        timeline,
    })
}

impl MediaBackend for SmtcBackend {
    fn snapshot(&mut self) -> Result<Snapshot, String> {
        let sessions_count = self
            .manager
            .GetSessions()
            .and_then(|v| v.Size())
            .map(|n| n as usize)
            .unwrap_or(0);
        let current = self.current();
        self.follow(current.as_ref());
        let session = match &current {
            Some(s) => Some(
                read_session(s).map_err(|e| format!("could not read the media session: {e}"))?,
            ),
            None => None,
        };
        Ok(Snapshot {
            session,
            sessions_count,
        })
    }

    fn thumbnail(&mut self) -> Result<Option<Vec<u8>>, String> {
        let err = |e: windows::core::Error| format!("thumbnail: {e}");
        let Some(session) = self.current() else {
            return Ok(None);
        };
        let props = session
            .TryGetMediaPropertiesAsync()
            .and_then(|op| op.join())
            .map_err(err)?;
        let Ok(reference) = props.Thumbnail() else {
            return Ok(None);
        };
        let stream = reference
            .OpenReadAsync()
            .and_then(|op| op.join())
            .map_err(err)?;
        let size = stream.Size().map_err(err)?;
        if size == 0 || size as usize > art::MAX_BYTES {
            return Ok(None);
        }
        let input = stream.GetInputStreamAt(0).map_err(err)?;
        let reader = DataReader::CreateDataReader(&input).map_err(err)?;
        reader
            .LoadAsync(size as u32)
            .and_then(|op| op.join())
            .map_err(err)?;
        let mut buf = vec![0u8; size as usize];
        reader.ReadBytes(&mut buf).map_err(err)?;
        Ok(Some(buf))
    }

    fn control(&mut self, action: Action, seek_to_ms: Option<i64>) -> Result<bool, String> {
        let err = |e: windows::core::Error| format!("media control: {e}");
        let Some(session) = self.current() else {
            return Err("nothing is playing".into());
        };
        let op = match action {
            Action::Next => session.TrySkipNextAsync(),
            Action::Previous => session.TrySkipPreviousAsync(),
            Action::Seek => {
                let start = session
                    .GetTimelineProperties()
                    .ok()
                    .and_then(|t| t.StartTime().ok())
                    .map_or(0, |t| t.Duration);
                let to = seek_to_ms.ok_or("a position is required")?;
                session.TryChangePlaybackPositionAsync(start + to * TICKS_PER_MS)
            }
            Action::TogglePlayPause => {
                let info = session.GetPlaybackInfo().map_err(err)?;
                let c = info.Controls().map_err(err)?;
                if c.IsPlayPauseToggleEnabled().unwrap_or(false) {
                    session.TryTogglePlayPauseAsync()
                } else if info
                    .PlaybackStatus()
                    .map(|s| s == Status::Playing)
                    .unwrap_or(false)
                {
                    session.TryPauseAsync()
                } else {
                    session.TryPlayAsync()
                }
            }
        };
        op.and_then(|o| o.join()).map_err(err)
    }

    fn subscribe(&mut self, wake: Wake) -> Result<(), String> {
        self.unsubscribe();
        let w = wake.clone();
        let token = self
            .manager
            .CurrentSessionChanged(
                &TypedEventHandler::<Manager, CurrentSessionChangedEventArgs>::new(move |_, _| {
                    w();
                    Ok(())
                }),
            )
            .map_err(|e| format!("could not subscribe to media changes: {e}"))?;
        self.manager_token = Some(token);
        self.wake = Some(wake);
        let current = self.current();
        self.follow(current.as_ref());
        Ok(())
    }

    fn unsubscribe(&mut self) {
        self.drop_session_sub();
        if let Some(t) = self.manager_token.take() {
            let _ = self.manager.RemoveCurrentSessionChanged(t);
        }
        self.wake = None;
    }
}

impl Drop for SmtcBackend {
    fn drop(&mut self) {
        self.unsubscribe();
    }
}
