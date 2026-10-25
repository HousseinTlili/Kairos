//! The system page's readings (M3 §5, D-012): read here, drawn by the app, never sent through
//! Hermes or a model.
//!
//! One background thread owns the readers and samples every 2 s while the System page is visible,
//! every 15 s otherwise (alerts still need readings while the window sits in the tray).

pub mod alerts;
pub mod parse;
pub mod processes;
pub mod rates;
pub mod speedtest;

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sysinfo::{
    CpuRefreshKind, DiskKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind,
    ProcessesToUpdate, RefreshKind, System,
};

use parse::{GpuInfo, WifiInfo};
use processes::{ProcessGroup, ProcessReading};
use rates::Counters;

const VISIBLE_INTERVAL: Duration = Duration::from_secs(2);
const HIDDEN_INTERVAL: Duration = Duration::from_secs(15);
const WIFI_INTERVAL: Duration = Duration::from_secs(10);
const DISK_LIST_INTERVAL: Duration = Duration::from_secs(60);
const TOOL_TIMEOUT: Duration = Duration::from_secs(3);
const TOP_PROCESSES: usize = 8;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CpuReading {
    pub name: String,
    pub frequency_mhz: u64,
    /// Percent of the whole CPU. `None` on the first sample, before there is anything to compare.
    pub total: Option<f32>,
    pub per_core: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MemoryReading {
    pub used: u64,
    pub total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DiskReading {
    /// `C:\` on Windows.
    pub mount: String,
    /// The volume label, often empty.
    pub name: String,
    /// `ssd`, `hdd` or `unknown`.
    pub kind: &'static str,
    pub removable: bool,
    pub total: u64,
    pub available: u64,
    pub read_per_s: Option<f64>,
    pub write_per_s: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NetworkReading {
    pub down_per_s: Option<f64>,
    pub up_per_s: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BatteryState {
    Charging,
    Discharging,
    Full,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BatteryReading {
    pub percent: f32,
    pub state: BatteryState,
    pub time_to_empty_s: Option<u64>,
    pub time_to_full_s: Option<u64>,
    /// Full-charge capacity as a share of the design capacity; wear is 100 minus this.
    pub health_percent: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SystemSnapshot {
    /// Unix milliseconds.
    pub taken_at: u64,
    pub cpu: CpuReading,
    pub memory: MemoryReading,
    pub disks: Vec<DiskReading>,
    pub network: NetworkReading,
    pub wifi: Option<WifiInfo>,
    pub gpus: Vec<GpuInfo>,
    pub battery: Option<BatteryReading>,
    pub uptime_s: u64,
    pub top_cpu: Vec<ProcessGroup>,
    pub top_memory: Vec<ProcessGroup>,
    /// Every process group, for the alerts; not sent to the page.
    #[serde(skip)]
    pub all_processes: Vec<ProcessGroup>,
    /// Readings that couldn't be taken this time, in words ("nvidia-smi didn't answer").
    pub notes: Vec<String>,
}

/// How a drive is named in a sentence: `C:\\` → `C:`; a root mount (`/`) keeps its name instead of
/// trimming to nothing.
pub fn drive_label(mount: &str) -> &str {
    let trimmed = mount.trim_end_matches(['\\', '/']);
    if trimmed.is_empty() { mount } else { trimmed }
}

/// Runs a console tool with no window and a time limit; its stdout, or why there is none.
fn run_tool(program: &str, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .spawn()
        .map_err(|err| format!("{program} isn't available ({err})"))?;
    // Read on another thread so a chatty tool can't fill the pipe and stall while we wait.
    let mut stdout = child.stdout.take().ok_or("no output pipe")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < TOOL_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(25))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program} didn't answer in time"));
            }
        }
    }
    let bytes = reader.join().map_err(|_| "output reader failed")?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Owns the readers. Lives on the monitor thread only.
pub struct Collector {
    system: System,
    disks: Disks,
    disks_listed_at: Instant,
    networks: Networks,
    battery: Option<starship_battery::Manager>,
    counters: Counters,
    wifi: Option<(Instant, Option<WifiInfo>)>,
    samples: u64,
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

impl Collector {
    pub fn new() -> Self {
        let system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        Self {
            system,
            disks: Disks::new_with_refreshed_list(),
            disks_listed_at: Instant::now(),
            networks: Networks::new_with_refreshed_list(),
            battery: starship_battery::Manager::new().ok(),
            counters: Counters::default(),
            wifi: None,
            samples: 0,
        }
    }

    pub fn sample(&mut self) -> SystemSnapshot {
        let now = Instant::now();
        let elapsed = self.counters.begin(now);
        let mut notes = Vec::new();
        self.samples += 1;

        self.system.refresh_cpu_all();
        self.system.refresh_memory();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );

        let cpus = self.system.cpus();
        let cores = cpus.len();
        let first = self.samples == 1;
        let cpu = CpuReading {
            name: cpus
                .first()
                .map(|c| c.brand().trim().to_string())
                .unwrap_or_default(),
            frequency_mhz: cpus.first().map_or(0, |c| c.frequency()),
            total: (!first).then(|| self.system.global_cpu_usage()),
            per_core: if first {
                Vec::new()
            } else {
                cpus.iter().map(|c| c.cpu_usage()).collect()
            },
        };
        let memory = MemoryReading {
            used: self.system.used_memory(),
            total: self.system.total_memory(),
            swap_used: self.system.used_swap(),
            swap_total: self.system.total_swap(),
        };

        if now.duration_since(self.disks_listed_at) >= DISK_LIST_INTERVAL {
            // Picks up drives plugged in since the last listing.
            self.disks = Disks::new_with_refreshed_list();
            self.disks_listed_at = now;
        } else {
            self.disks.refresh(true);
        }
        let mut disks = Vec::new();
        for disk in self.disks.list() {
            let mount = disk.mount_point().display().to_string();
            let usage = disk.usage();
            disks.push(DiskReading {
                name: disk.name().to_string_lossy().into_owned(),
                kind: match disk.kind() {
                    DiskKind::SSD => "ssd",
                    DiskKind::HDD => "hdd",
                    DiskKind::Unknown(_) => "unknown",
                },
                removable: disk.is_removable(),
                total: disk.total_space(),
                available: disk.available_space(),
                read_per_s: self.counters.rate(
                    &format!("disk-read:{mount}"),
                    usage.total_read_bytes,
                    elapsed,
                ),
                write_per_s: self.counters.rate(
                    &format!("disk-write:{mount}"),
                    usage.total_written_bytes,
                    elapsed,
                ),
                mount,
            });
        }
        disks.sort_by(|a, b| a.mount.cmp(&b.mount));

        self.networks.refresh(true);
        let (mut received, mut transmitted) = (0u64, 0u64);
        for (name, data) in self.networks.list() {
            let lower = name.to_lowercase();
            if lower == "lo" || lower.contains("loopback") {
                continue;
            }
            received += data.total_received();
            transmitted += data.total_transmitted();
        }
        let network = NetworkReading {
            down_per_s: self.counters.rate("net-down", received, elapsed),
            up_per_s: self.counters.rate("net-up", transmitted, elapsed),
        };

        let wifi = self.wifi(now, &mut notes);
        let gpus = gpus(&mut notes);
        let battery = self.battery_reading();

        let readings = self.system.processes().values().map(|p| ProcessReading {
            name: p.name().to_string_lossy().into_owned(),
            cpu_percent: p.cpu_usage(),
            memory: p.memory(),
        });
        let mut all_processes = processes::group(readings, cores);
        if first {
            // No CPU figures yet on the first sample; zero rather than garbage.
            for group in &mut all_processes {
                group.cpu_percent = 0.0;
            }
        }
        let (top_cpu, top_memory) = processes::top(&all_processes, TOP_PROCESSES);

        SystemSnapshot {
            taken_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64),
            cpu,
            memory,
            disks,
            network,
            wifi,
            gpus,
            battery,
            uptime_s: System::uptime(),
            top_cpu,
            top_memory,
            all_processes,
            notes,
        }
    }

    fn wifi(&mut self, now: Instant, notes: &mut Vec<String>) -> Option<WifiInfo> {
        if let Some((at, cached)) = &self.wifi
            && now.duration_since(*at) < WIFI_INTERVAL
        {
            return cached.clone();
        }
        let reading = if cfg!(windows) {
            match run_tool("netsh", &["wlan", "show", "interfaces"]) {
                Ok(text) => parse::parse_netsh_wlan(&text),
                Err(reason) => {
                    notes.push(format!("Wi-Fi: {reason}"));
                    None
                }
            }
        } else {
            None
        };
        self.wifi = Some((now, reading.clone()));
        reading
    }

    fn battery_reading(&mut self) -> Option<BatteryReading> {
        use starship_battery::units::ratio::percent;
        use starship_battery::units::time::second;
        let manager = self.battery.as_ref()?;
        let battery = manager.batteries().ok()?.flatten().next()?;
        let state = match battery.state() {
            starship_battery::State::Charging => BatteryState::Charging,
            starship_battery::State::Discharging | starship_battery::State::Empty => {
                BatteryState::Discharging
            }
            starship_battery::State::Full => BatteryState::Full,
            _ => BatteryState::Unknown,
        };
        let health = battery.state_of_health().get::<percent>();
        Some(BatteryReading {
            percent: battery.state_of_charge().get::<percent>().clamp(0.0, 100.0),
            state,
            time_to_empty_s: battery.time_to_empty().map(|t| t.get::<second>() as u64),
            time_to_full_s: battery.time_to_full().map(|t| t.get::<second>() as u64),
            health_percent: (health.is_finite() && health > 0.0).then_some(health.min(100.0)),
        })
    }
}

fn gpus(notes: &mut Vec<String>) -> Vec<GpuInfo> {
    match run_tool(
        "nvidia-smi",
        &[
            "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu",
            "--format=csv,noheader,nounits",
        ],
    ) {
        Ok(text) => parse::parse_nvidia_smi(&text),
        // No NVIDIA driver is normal, not worth a note; a hung one is.
        Err(reason) if reason.contains("in time") => {
            notes.push(format!("GPU: {reason}"));
            Vec::new()
        }
        Err(_) => Vec::new(),
    }
}

/// Settings ▸ System's "alerts" switch, shared between the monitor thread and the commands.
#[derive(Debug)]
pub struct AlertSwitch(pub AtomicBool);

impl AlertSwitch {
    pub const SETTING: &'static str = "alerts_enabled";

    pub fn enabled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// The monitor thread and the latest snapshot, shared with the commands.
#[derive(Default)]
pub struct SystemMonitor {
    latest: Mutex<Option<SystemSnapshot>>,
    visible: AtomicBool,
    wake: Mutex<Option<Sender<()>>>,
}

impl SystemMonitor {
    /// Starts sampling; `on_snapshot` runs on the monitor thread after each sample.
    pub fn start(
        self: &Arc<Self>,
        mut on_snapshot: impl FnMut(&SystemSnapshot) + Send + 'static,
    ) -> std::io::Result<()> {
        let (tx, rx) = mpsc::channel::<()>();
        if let Ok(mut wake) = self.wake.lock() {
            *wake = Some(tx);
        }
        let monitor = Arc::clone(self);
        std::thread::Builder::new()
            .name("kairos-system".into())
            .spawn(move || {
                let mut collector = Collector::new();
                loop {
                    let snapshot = collector.sample();
                    on_snapshot(&snapshot);
                    if let Ok(mut latest) = monitor.latest.lock() {
                        *latest = Some(snapshot);
                    }
                    let interval = if monitor.visible.load(Ordering::Relaxed) {
                        VISIBLE_INTERVAL
                    } else {
                        HIDDEN_INTERVAL
                    };
                    match rx.recv_timeout(interval) {
                        Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            })?;
        Ok(())
    }

    pub fn latest(&self) -> Option<SystemSnapshot> {
        self.latest.lock().ok().and_then(|latest| latest.clone())
    }

    /// The page tells the monitor when it is on screen; becoming visible samples at once.
    pub fn set_visible(&self, visible: bool) {
        let was = self.visible.swap(visible, Ordering::Relaxed);
        if visible
            && !was
            && let Ok(wake) = self.wake.lock()
            && let Some(tx) = wake.as_ref()
        {
            let _ = tx.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_samples_on_this_machine_give_cpu_memory_and_uptime() {
        let mut collector = Collector::new();
        let first = collector.sample();
        assert_eq!(
            first.cpu.total, None,
            "nothing to compare on the first sample"
        );
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let second = collector.sample();
        let total = second.cpu.total.expect("cpu total on the second sample");
        assert!((0.0..=100.0).contains(&total));
        assert!(!second.cpu.per_core.is_empty());
        assert!(second.memory.total > 0 && second.memory.used <= second.memory.total);
        assert!(second.uptime_s > 0);
        assert!(!second.all_processes.is_empty());
        assert!(second.top_cpu.len() <= TOP_PROCESSES);
    }

    #[test]
    fn drives_are_named_without_a_trailing_slash_but_never_empty() {
        assert_eq!(drive_label("C:\\"), "C:");
        assert_eq!(drive_label("/mnt/data/"), "/mnt/data");
        assert_eq!(drive_label("/"), "/");
    }

    #[test]
    fn a_missing_tool_is_an_explanation_not_a_crash() {
        let err = run_tool("kairos-no-such-tool", &[]).expect_err("missing");
        assert!(err.contains("isn't available"));
    }
}
