//! Media: what is playing (title, artist, album, cover), play/pause/previous/next, progress and seek,
//! from the Windows System Media Transport Controls. Works for any app that publishes a session:
//! Spotify, a browser tab, VLC, the Windows media player...
//!
//! - Event driven. A worker thread blocks on a channel; the OS calls our handlers when something
//!   changes (see smtc.rs). There is no timer and no polling here. The UI extrapolates the
//!   progress bar from the last reported position.
//! - Only while the Media tab is on screen. The UI reports that (`visible`); when it is not, we
//!   are unsubscribed from the OS entirely and read nothing.
//! - Privacy: track titles, artists and covers stay in memory, go only to the UI, are never logged
//!   and are never stored in SQLite.
//!
//! Ported from Elissa (the product owner's island app, M15 / D-030) without her audio visualiser
//! and fake player.
mod art;
mod backend;
mod smtc;
mod state;

use super::registry::Module;
use backend::{Controller, MediaBackend, unix_ms};
use serde::Serialize;
use serde_json::{Value, json};
use state::{Action, MediaState, Snapshot};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        mpsc::{Sender, channel},
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

const EVENT: &str = "media://state";
const REPLY_TIMEOUT: Duration = Duration::from_secs(10);
/// Bursts of OS events (properties + playback + timeline) for one change are folded into one read.
const COALESCE: Duration = Duration::from_millis(40);

/// What the UI receives: the state, plus why media is unavailable if it is.
#[derive(Clone, Serialize)]
struct Update {
    state: MediaState,
    error: Option<String>,
}

type Reply<T> = Sender<Result<T, String>>;

enum Msg {
    /// The Media tab is on screen.
    Visible(bool),
    Wake,
    State(Reply<Update>),
    Cover(String, Reply<Option<String>>),
    Act(Action, Option<i64>, Reply<bool>),
}

struct Media {
    tx: Mutex<Option<Sender<Msg>>>,
}

pub fn module() -> Box<dyn Module> {
    Box::new(Media {
        tx: Mutex::new(None),
    })
}

/// Stands in when the OS media API cannot be opened: an empty state plus the reason.
struct Unavailable(String);
impl MediaBackend for Unavailable {
    fn snapshot(&mut self) -> Result<Snapshot, String> {
        Err(self.0.clone())
    }
    fn thumbnail(&mut self) -> Result<Option<Vec<u8>>, String> {
        Ok(None)
    }
    fn control(&mut self, _: Action, _: Option<i64>) -> Result<bool, String> {
        Err(self.0.clone())
    }
    fn subscribe(&mut self, _: backend::Wake) -> Result<(), String> {
        Ok(())
    }
    fn unsubscribe(&mut self) {}
}

fn make_backend() -> Box<dyn MediaBackend> {
    match smtc::SmtcBackend::new() {
        Ok(b) => Box::new(b),
        Err(e) => Box::new(Unavailable(e)),
    }
}

fn read(c: &mut Controller) -> Update {
    match c.refresh(unix_ms()) {
        Ok(state) => Update { state, error: None },
        Err(e) => Update {
            state: MediaState::empty(0),
            error: Some(e),
        },
    }
}

/// Read the state and send it to the UI.
fn publish(app: &AppHandle, c: &mut Controller) {
    let _ = app.emit(EVENT, read(c));
}

fn run(app: AppHandle, tx: Sender<Msg>, rx: std::sync::mpsc::Receiver<Msg>) {
    // Created here, on the thread that will use it, and never moved.
    let mut controller = Controller::new(make_backend());
    let started = Instant::now();
    let mut visible = false;
    let mut pending: VecDeque<Msg> = VecDeque::new();
    let wake: backend::Wake = {
        let tx = Mutex::new(tx);
        Arc::new(move || {
            let _ = tx.lock().map(|t| t.send(Msg::Wake));
        })
    };

    loop {
        let msg = match pending.pop_front() {
            Some(m) => m,
            None => match rx.recv() {
                Ok(m) => m,
                Err(_) => return,
            },
        };
        match msg {
            Msg::Visible(v) if v != visible => {
                visible = v;
                if v {
                    if let Err(e) = controller.subscribe(wake.clone()) {
                        eprintln!("media: {e}");
                    }
                    publish(&app, &mut controller);
                } else {
                    controller.unsubscribe();
                }
            }
            Msg::Visible(_) => {}
            Msg::Wake => {
                if !visible {
                    continue;
                }
                thread::sleep(COALESCE);
                while let Ok(m) = rx.try_recv() {
                    if !matches!(m, Msg::Wake) {
                        pending.push_back(m);
                    }
                }
                publish(&app, &mut controller);
            }
            Msg::State(reply) => {
                let _ = reply.send(Ok(read(&mut controller)));
            }
            Msg::Cover(key, reply) => {
                let now_ms = started.elapsed().as_millis() as u64;
                let _ = reply.send(controller.cover(&key, now_ms, unix_ms()));
            }
            Msg::Act(action, seek_to, reply) => {
                let r = controller.act(action, seek_to, unix_ms());
                let _ = reply.send(r);
                if visible {
                    publish(&app, &mut controller);
                }
            }
        }
    }
}

impl Media {
    fn ask<T>(&self, make: impl FnOnce(Reply<T>) -> Msg) -> Result<T, String> {
        let (rtx, rrx) = channel();
        self.tx
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .ok_or("media module not started")?
            .send(make(rtx))
            .map_err(|_| "media worker stopped".to_string())?;
        rrx.recv_timeout(REPLY_TIMEOUT)
            .map_err(|_| "media operation timed out".to_string())?
    }

    fn send(&self, m: Msg) -> Result<(), String> {
        self.tx
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .ok_or("media module not started")?
            .send(m)
            .map_err(|_| "media worker stopped".to_string())
    }
}

impl Module for Media {
    fn id(&self) -> &'static str {
        "media"
    }

    fn setup(&self, app: &AppHandle) -> Result<(), String> {
        let (tx, rx) = channel();
        *self.tx.lock().map_err(|e| e.to_string())? = Some(tx.clone());
        let app = app.clone();
        thread::Builder::new()
            .name("media-worker".into())
            .spawn(move || run(app, tx, rx))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn invoke(&self, _app: &AppHandle, command: &str, args: Value) -> Result<Value, String> {
        let pos = || args.get("position_ms").and_then(Value::as_i64);
        match command {
            "visible" => {
                let v = args
                    .get("visible")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                self.send(Msg::Visible(v))?;
                Ok(Value::Null)
            }
            "state" => Ok(json!(self.ask(Msg::State)?)),
            "cover" => {
                let key = args
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or("key is required")?
                    .to_string();
                Ok(json!(self.ask(|r| Msg::Cover(key, r))?))
            }
            "toggle" => Ok(json!(self.ask(|r| Msg::Act(
                Action::TogglePlayPause,
                None,
                r
            ))?)),
            "next" => Ok(json!(self.ask(|r| Msg::Act(Action::Next, None, r))?)),
            "previous" => Ok(json!(self.ask(|r| Msg::Act(Action::Previous, None, r))?)),
            "seek" => {
                let p = pos().ok_or("position_ms is required")?;
                Ok(json!(self.ask(|r| Msg::Act(Action::Seek, Some(p), r))?))
            }
            other => Err(format!("media: unknown command '{other}'")),
        }
    }
}
