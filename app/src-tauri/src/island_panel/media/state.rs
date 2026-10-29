//! Pure media state: the raw data an OS backend reports, and what the UI is told. No OS calls, no
//! clock reads (the caller passes "now"), so everything here is unit tested.
//!
//! Privacy: titles, artists, albums and covers are only ever held in memory and sent to the UI.
//! Nothing in this module logs them, and nothing stores them.
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Playback {
    Playing,
    Paused,
    Stopped,
    /// Opening or changing track: there is a session but it is not steady yet.
    Busy,
    Closed,
}

/// Which transport controls the session says it supports right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Controls {
    pub play: bool,
    pub pause: bool,
    pub toggle: bool,
    pub next: bool,
    pub previous: bool,
    pub seek: bool,
}

impl Controls {
    /// Can the play/pause button do anything?
    pub fn can_play_pause(&self) -> bool {
        self.toggle || self.play || self.pause
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    TogglePlayPause,
    Next,
    Previous,
    Seek,
}

/// Timeline as the OS reports it (positions relative to the OS's own origin).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawTimeline {
    pub position_ms: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    /// When `position_ms` was last updated, as Unix milliseconds. `None` if the app never says.
    pub updated_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RawSession {
    pub app_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub has_thumbnail: bool,
    pub status: Playback,
    pub controls: Controls,
    pub rate: f64,
    pub timeline: Option<RawTimeline>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// The session Windows considers current (what the system media flyout shows).
    pub session: Option<RawSession>,
    pub sessions_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Timeline {
    /// Position at the moment this state was built, already corrected for the time elapsed since the
    /// OS last updated it. The UI extrapolates forward from here using `rate`.
    pub position_ms: i64,
    pub duration_ms: i64,
    pub rate: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MediaState {
    pub has_session: bool,
    pub app: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub status: Playback,
    pub controls: Controls,
    /// Identifies the track (app + title + artist + album); the UI asks for the cover by this key.
    pub track_key: String,
    pub has_thumbnail: bool,
    /// `None` when the app reports no usable timeline (live streams, apps that never report one).
    pub timeline: Option<Timeline>,
    pub sessions_count: usize,
}

impl MediaState {
    pub fn empty(sessions_count: usize) -> Self {
        Self {
            has_session: false,
            app: None,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            status: Playback::Closed,
            controls: Controls::default(),
            track_key: String::new(),
            has_thumbnail: false,
            timeline: None,
            sessions_count,
        }
    }
}

/// "Spotify.exe" -> "Spotify"; "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify" -> "Spotify";
/// "Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic" -> "Microsoft.ZuneMusic".
pub fn friendly_app(app_id: &str) -> String {
    let id = app_id.trim();
    let tail = id.rsplit('!').next().unwrap_or(id);
    let tail = tail.rsplit(['\\', '/']).next().unwrap_or(tail);
    let name = tail
        .strip_suffix(".exe")
        .or_else(|| tail.strip_suffix(".EXE"))
        .unwrap_or(tail);
    if name.is_empty() {
        id.to_string()
    } else {
        name.to_string()
    }
}

/// A stable key for "this track from this app" (FNV-1a over the metadata, hex). Never logged.
pub fn track_key(app_id: &str, title: &str, artist: &str, album: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in [app_id, title, artist, album] {
        for b in part.bytes().chain(std::iter::once(0x1f)) {
            h ^= b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

/// Position now, from what the OS last said. While playing, time has passed since the OS updated
/// the position, so move it forward by `rate`; clamp to the track.
pub fn corrected_position(
    raw_position_ms: i64,
    updated_unix_ms: Option<i64>,
    now_unix_ms: i64,
    playing: bool,
    rate: f64,
    duration_ms: i64,
) -> i64 {
    let mut pos = raw_position_ms;
    if playing && let Some(updated) = updated_unix_ms {
        // A clock that is behind, or a nonsense timestamp, must not move the position
        // backwards or by days.
        let age = (now_unix_ms - updated).clamp(0, 24 * 3600 * 1000);
        pos += (age as f64 * rate) as i64;
    }
    pos.clamp(0, duration_ms.max(0))
}

pub fn map_snapshot(snap: &Snapshot, now_unix_ms: i64) -> MediaState {
    let Some(s) = &snap.session else {
        return MediaState::empty(snap.sessions_count);
    };
    let rate = if s.rate.is_finite() && s.rate > 0.0 {
        s.rate
    } else {
        1.0
    };
    let timeline = s.timeline.and_then(|t| {
        let duration = t.end_ms - t.start_ms;
        // Apps that do not know the length report 0 (or end before start): no timeline to show.
        (duration > 0).then(|| Timeline {
            position_ms: corrected_position(
                t.position_ms - t.start_ms,
                t.updated_unix_ms,
                now_unix_ms,
                s.status == Playback::Playing,
                rate,
                duration,
            ),
            duration_ms: duration,
            rate,
        })
    });
    MediaState {
        has_session: true,
        app: Some(friendly_app(&s.app_id)),
        title: s.title.clone(),
        artist: s.artist.clone(),
        album: s.album.clone(),
        status: s.status,
        controls: s.controls,
        track_key: track_key(&s.app_id, &s.title, &s.artist, &s.album),
        has_thumbnail: s.has_thumbnail,
        timeline,
        sessions_count: snap.sessions_count,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn session() -> RawSession {
        RawSession {
            app_id: "Spotify.exe".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            has_thumbnail: true,
            status: Playback::Playing,
            controls: Controls {
                play: true,
                pause: true,
                toggle: true,
                next: true,
                previous: true,
                seek: true,
            },
            rate: 1.0,
            timeline: Some(RawTimeline {
                position_ms: 10_000,
                start_ms: 0,
                end_ms: 200_000,
                updated_unix_ms: Some(1_000_000),
            }),
        }
    }

    fn snap(s: RawSession) -> Snapshot {
        Snapshot {
            session: Some(s),
            sessions_count: 1,
        }
    }

    #[test]
    fn no_session_is_an_empty_state_not_an_error() {
        let st = map_snapshot(&Snapshot::default(), 0);
        assert!(!st.has_session);
        assert_eq!(st.status, Playback::Closed);
        assert_eq!(st.timeline, None);
        assert_eq!(st.controls, Controls::default());
        assert!(st.title.is_empty() && st.app.is_none());
        assert_eq!(
            map_snapshot(
                &Snapshot {
                    session: None,
                    sessions_count: 3
                },
                0
            )
            .sessions_count,
            3
        );
    }

    #[test]
    fn playing_position_is_moved_forward_by_the_time_since_the_os_update() {
        // updated at t=1_000_000 ms, read 4 s later: 10 s + 4 s
        let st = map_snapshot(&snap(session()), 1_004_000);
        assert_eq!(st.timeline.unwrap().position_ms, 14_000);
        assert_eq!(st.timeline.unwrap().duration_ms, 200_000);
    }

    #[test]
    fn paused_position_does_not_move() {
        let mut s = session();
        s.status = Playback::Paused;
        let st = map_snapshot(&snap(s), 1_060_000);
        assert_eq!(st.timeline.unwrap().position_ms, 10_000);
    }

    #[test]
    fn playback_rate_scales_the_correction() {
        let mut s = session();
        s.rate = 2.0;
        assert_eq!(
            map_snapshot(&snap(s), 1_004_000)
                .timeline
                .unwrap()
                .position_ms,
            18_000
        );
    }

    #[test]
    fn position_is_clamped_to_the_track_and_survives_bad_clocks() {
        assert_eq!(
            corrected_position(199_000, Some(0), 60_000, true, 1.0, 200_000),
            200_000
        );
        // clock behind the update time: no backwards jump
        assert_eq!(
            corrected_position(5_000, Some(2_000_000), 1_000_000, true, 1.0, 200_000),
            5_000
        );
        // a days-old timestamp is capped, not applied in full
        assert_eq!(
            corrected_position(0, Some(0), i64::MAX / 4, true, 1.0, i64::MAX / 8),
            86_400_000
        );
        // unknown update time: take the position as given
        assert_eq!(
            corrected_position(7_000, None, 9_999_999, true, 1.0, 200_000),
            7_000
        );
        assert_eq!(corrected_position(-5, None, 0, false, 1.0, 100), 0);
    }

    #[test]
    fn timeline_is_dropped_when_the_length_is_unknown() {
        let mut s = session();
        s.timeline = Some(RawTimeline {
            position_ms: 0,
            start_ms: 0,
            end_ms: 0,
            updated_unix_ms: None,
        });
        assert_eq!(map_snapshot(&snap(s.clone()), 0).timeline, None);
        s.timeline = Some(RawTimeline {
            position_ms: 0,
            start_ms: 50,
            end_ms: 10,
            updated_unix_ms: None,
        });
        assert_eq!(map_snapshot(&snap(s.clone()), 0).timeline, None);
        s.timeline = None; // apps that never report a timeline at all
        let st = map_snapshot(&snap(s), 0);
        assert!(st.has_session && st.timeline.is_none());
    }

    #[test]
    fn position_is_relative_to_the_timeline_start() {
        let mut s = session();
        s.timeline = Some(RawTimeline {
            position_ms: 65_000,
            start_ms: 60_000,
            end_ms: 260_000,
            updated_unix_ms: None,
        });
        let t = map_snapshot(&snap(s), 0).timeline.unwrap();
        assert_eq!((t.position_ms, t.duration_ms), (5_000, 200_000));
    }

    #[test]
    fn controls_pass_through_exactly_and_seek_is_only_as_reported() {
        let mut s = session();
        s.controls = Controls {
            play: true,
            pause: false,
            toggle: false,
            next: false,
            previous: true,
            seek: false,
        };
        let c = map_snapshot(&snap(s), 0).controls;
        assert!(c.can_play_pause() && c.previous && !c.next && !c.seek);
        assert!(!Controls::default().can_play_pause());
    }

    #[test]
    fn nonsense_rates_fall_back_to_normal_speed() {
        for r in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let mut s = session();
            s.rate = r;
            let t = map_snapshot(&snap(s), 1_001_000).timeline.unwrap();
            assert_eq!(t.rate, if r.is_finite() && r > 0.0 { r } else { 1.0 });
            assert_eq!(t.position_ms, 11_000);
        }
    }

    #[test]
    fn friendly_names() {
        assert_eq!(friendly_app("Spotify.exe"), "Spotify");
        assert_eq!(friendly_app("chrome.exe"), "chrome");
        assert_eq!(
            friendly_app("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
            "Spotify"
        );
        assert_eq!(
            friendly_app("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"),
            "Microsoft.ZuneMusic"
        );
        assert_eq!(friendly_app("C:\\Apps\\vlc.exe"), "vlc");
        assert_eq!(friendly_app(""), "");
    }

    #[test]
    fn track_key_is_stable_and_changes_with_the_track() {
        let a = track_key("Spotify.exe", "Song", "Artist", "Album");
        assert_eq!(a, track_key("Spotify.exe", "Song", "Artist", "Album"));
        assert_ne!(a, track_key("Spotify.exe", "Other", "Artist", "Album"));
        assert_ne!(a, track_key("chrome.exe", "Song", "Artist", "Album"));
        // field boundaries matter: moving a character between fields must change the key
        assert_ne!(track_key("a", "bc", "", ""), track_key("ab", "c", "", ""));
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn app_name_and_session_count_are_reported() {
        let st = map_snapshot(
            &Snapshot {
                session: Some(session()),
                sessions_count: 3,
            },
            0,
        );
        assert_eq!(st.app.as_deref(), Some("Spotify"));
        assert_eq!(st.sessions_count, 3);
        assert!(st.has_thumbnail);
    }
}
