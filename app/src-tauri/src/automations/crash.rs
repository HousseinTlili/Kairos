//! Program crashes and hangs from the Windows Application event log (M17 §3.1, Step 4):
//! *Application Error* 1000 and *Application Hang* 1002, readable without admin rights. Read every
//! 10 s for records newer than the last one seen; the crashes from before Kairos started are not
//! replayed.

use std::collections::HashMap;

use super::CrashInfo;

/// A burst of events from one program (a crash, then its hang, then the restart crashing) counts once.
pub const BURST_S: i64 = 60;

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn element<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{name}>"))?;
    Some(&xml[start..start + end])
}

/// `<Data Name='AppName'>…</Data>` (Windows writes single quotes; double are accepted too).
fn data(xml: &str, name: &str) -> Option<String> {
    for quote in ['\'', '"'] {
        let open = format!("<Data Name={quote}{name}{quote}>");
        if let Some(at) = xml.find(&open) {
            let start = at + open.len();
            let end = xml[start..].find("</Data>")?;
            let value = unescape(&xml[start..start + end]);
            return (!value.trim().is_empty()).then_some(value);
        }
    }
    None
}

/// One event's XML (as `EvtRender` gives it) as a crash, or `None` for anything else.
pub fn parse_event(xml: &str) -> Option<CrashInfo> {
    let hang = if xml.contains("Name='Application Hang'")
        || xml.contains("Name=\"Application Hang\"")
    {
        true
    } else if xml.contains("Name='Application Error'") || xml.contains("Name=\"Application Error\"")
    {
        false
    } else {
        return None;
    };
    let id = element(xml, "EventID")?.trim();
    if id != if hang { "1002" } else { "1000" } {
        return None;
    }
    Some(CrashInfo {
        record: element(xml, "EventRecordID")?.trim().parse().ok()?,
        program: data(xml, "AppName")?,
        version: data(xml, "AppVersion").unwrap_or_default(),
        // Windows writes "unknown" when it can't tell (a fail-fast exit, seen 2026-10-30).
        module: if hang {
            None
        } else {
            data(xml, "ModuleName").filter(|m| !m.eq_ignore_ascii_case("unknown"))
        },
        code: if hang {
            None
        } else {
            data(xml, "ExceptionCode")
        },
        hang,
    })
}

/// Drops ignored programs and repeats within a burst. `last` remembers each program's last event.
pub fn worth_telling(
    crash: &CrashInfo,
    at: i64,
    ignore: &[String],
    last: &mut HashMap<String, i64>,
) -> bool {
    let name = crash.program.to_lowercase();
    let ignored = name == "kairos.exe"
        || ignore.iter().any(|i| {
            let i = i.trim().to_lowercase();
            i == name || format!("{i}.exe") == name
        });
    if ignored {
        return false;
    }
    let recent = last.get(&name).is_some_and(|t| at - t < BURST_S);
    last.insert(name, at);
    !recent
}

#[cfg(windows)]
pub use windows::watch;

#[cfg(windows)]
mod windows {
    use std::time::Duration;

    use windows_sys::Win32::System::EventLog::{
        EVT_HANDLE, EvtClose, EvtNext, EvtQuery, EvtQueryChannelPath, EvtQueryReverseDirection,
        EvtRender, EvtRenderEventXml,
    };

    use super::{CrashInfo, parse_event};

    const POLL: Duration = Duration::from_secs(10);
    const FILTER: &str = "(Provider[@Name='Application Error'] and EventID=1000) or \
                          (Provider[@Name='Application Hang'] and EventID=1002)";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Closes the handle when dropped.
    struct Handle(EVT_HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            if self.0 != 0 {
                // SAFETY: a handle from EvtQuery or EvtNext, closed once.
                unsafe { EvtClose(self.0) };
            }
        }
    }

    fn render(event: EVT_HANDLE) -> Option<String> {
        let mut used = 0u32;
        let mut count = 0u32;
        // SAFETY: a size query with no buffer; it fails with "insufficient buffer" and sets `used`.
        unsafe {
            EvtRender(
                0,
                event,
                EvtRenderEventXml,
                0,
                std::ptr::null_mut(),
                &mut used,
                &mut count,
            )
        };
        if used == 0 {
            return None;
        }
        let mut buffer = vec![0u16; (used as usize).div_ceil(2)];
        // SAFETY: the buffer holds `used` bytes, as asked for.
        let ok = unsafe {
            EvtRender(
                0,
                event,
                EvtRenderEventXml,
                (buffer.len() * 2) as u32,
                buffer.as_mut_ptr().cast(),
                &mut used,
                &mut count,
            )
        };
        if ok == 0 {
            return None;
        }
        let text = String::from_utf16_lossy(&buffer);
        Some(text.trim_end_matches('\0').to_string())
    }

    /// The matching events after record `after`, oldest first (or, with `newest_only`, just the
    /// newest one's record number, to start from).
    fn query(after: u64, newest_only: bool) -> Result<Vec<CrashInfo>, String> {
        let path = wide("Application");
        let text = format!("*[System[({FILTER}) and EventRecordID > {after}]]");
        let query = wide(&text);
        let mut flags = EvtQueryChannelPath;
        if newest_only {
            flags |= EvtQueryReverseDirection;
        }
        // SAFETY: two NUL-terminated UTF-16 strings that outlive the call.
        let results = Handle(unsafe { EvtQuery(0, path.as_ptr(), query.as_ptr(), flags) });
        if results.0 == 0 {
            return Err("couldn't read the Application event log".into());
        }
        let mut found = Vec::new();
        loop {
            let mut events = [0isize; 16];
            let mut returned = 0u32;
            // SAFETY: room for 16 handles; each returned one is closed by `Handle`.
            let ok = unsafe {
                EvtNext(
                    results.0,
                    events.len() as u32,
                    events.as_mut_ptr(),
                    0,
                    0,
                    &mut returned,
                )
            };
            if ok == 0 || returned == 0 {
                break;
            }
            for raw in &events[..returned as usize] {
                let event = Handle(*raw);
                if let Some(crash) = render(event.0).as_deref().and_then(parse_event) {
                    found.push(crash);
                }
            }
            if newest_only && !found.is_empty() {
                found.truncate(1);
                break;
            }
        }
        Ok(found)
    }

    /// Calls `on_crash` for every new crash or hang, from a thread of its own, for as long as
    /// Kairos runs.
    pub fn watch(mut on_crash: impl FnMut(CrashInfo) + Send + 'static) -> std::io::Result<()> {
        std::thread::Builder::new()
            .name("kairos-crashes".into())
            .spawn(move || {
                let mut last = query(0, true)
                    .ok()
                    .and_then(|v| v.first().map(|c| c.record))
                    .unwrap_or(0);
                loop {
                    std::thread::sleep(POLL);
                    match query(last, false) {
                        Ok(crashes) => {
                            for crash in crashes {
                                last = last.max(crash.record);
                                on_crash(crash);
                            }
                        }
                        Err(err) => eprintln!("kairos: crashes: {err}"),
                    }
                }
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CRASH: &str = include_str!("../../../../fixtures/eventlog/crash-1000.xml");
    const HANG: &str = include_str!("../../../../fixtures/eventlog/hang-1002.xml");

    #[test]
    fn a_recorded_crash_gives_the_program_version_module_and_code() {
        let crash = parse_event(CRASH).expect("a crash");
        assert_eq!(
            crash,
            CrashInfo {
                record: 14608,
                program: "RuntimeBroker.exe".into(),
                version: "10.0.26100.9278".into(),
                module: Some("ntdll.dll".into()),
                code: Some("c0000409".into()),
                hang: false,
            }
        );
    }

    #[test]
    fn a_recorded_hang_is_a_hang_without_a_module() {
        let hang = parse_event(HANG).expect("a hang");
        assert!(hang.hang);
        assert_eq!(hang.program, "claude.exe");
        assert_eq!(hang.record, 19553);
        assert_eq!(hang.module, None);
    }

    #[test]
    fn an_unknown_module_is_left_out_but_the_exception_code_kept() {
        let crash = parse_event(&CRASH.replace(
            ">ntdll.dll</Data><Data Name='ModuleVersion'",
            ">unknown</Data><Data Name='ModuleVersion'",
        ))
        .expect("a crash");
        assert_eq!(crash.module, None);
        assert_eq!(
            crate::automations::Happening::Crash(crash).describe(),
            "RuntimeBroker.exe 10.0.26100.9278 crashed (exception 0xc0000409)"
        );
    }

    #[test]
    fn other_events_are_not_crashes() {
        assert_eq!(
            parse_event(&CRASH.replace("1000</EventID>", "1001</EventID>")),
            None
        );
        assert_eq!(
            parse_event(&CRASH.replace("Application Error", "Windows Error Reporting")),
            None
        );
    }

    #[test]
    fn ignored_programs_and_a_burst_from_one_program_are_told_once_at_most() {
        let mut crash = parse_event(HANG).expect("hang");
        let mut last = HashMap::new();
        let ignore = vec!["RuntimeBroker".to_string()];
        assert!(worth_telling(&crash, 100, &ignore, &mut last));
        assert!(
            !worth_telling(&crash, 130, &ignore, &mut last),
            "same burst"
        );
        assert!(
            worth_telling(&crash, 200, &ignore, &mut last),
            "a minute later"
        );
        crash.program = "runtimebroker.exe".into();
        assert!(
            !worth_telling(&crash, 300, &ignore, &mut last),
            "ignored without .exe"
        );
        crash.program = "Kairos.exe".into();
        assert!(
            !worth_telling(&crash, 400, &[], &mut last),
            "never Kairos itself"
        );
    }
}
