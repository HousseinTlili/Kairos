//! Top processes, grouped by executable the way Task Manager groups an app's processes
//! (M3 §5.1): twenty `chrome.exe` processes are one row with their CPU and memory summed.

use std::collections::HashMap;

use serde::Serialize;

/// One process as read from the OS. `cpu_percent` is per core, as sysinfo reports it: a process
/// keeping two cores busy reads 200.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessReading {
    pub name: String,
    pub cpu_percent: f32,
    pub memory: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcessGroup {
    pub name: String,
    /// How many processes the row stands for.
    pub count: u32,
    /// Share of the whole CPU: 100 means every core is busy, as in Task Manager.
    pub cpu_percent: f32,
    pub memory: u64,
}

/// Windows system processes: never the subject of a CPU alert, since they can't be closed. The
/// same names as the `system-processes` rule (M2 §6) and the plugin's protected list (M3 §6).
pub const PROTECTED: &[&str] = &[
    "explorer", "winlogon", "csrss", "lsass", "services", "svchost", "smss", "wininit", "dwm",
    "system", "idle", "registry", "kairos",
];

/// `Chrome.EXE` → `chrome`: names compare case-insensitively and without `.exe`.
pub fn base_name(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    lower
        .strip_suffix(".exe")
        .map(str::to_string)
        .unwrap_or(lower)
}

pub fn is_protected(name: &str) -> bool {
    let base = base_name(name);
    base.is_empty() || PROTECTED.contains(&base.as_str()) || base == "system idle process"
}

/// Groups readings by executable, normalises CPU to the whole machine, and returns the groups.
/// Idle-process pseudo entries are dropped.
pub fn group(
    readings: impl IntoIterator<Item = ProcessReading>,
    cores: usize,
) -> Vec<ProcessGroup> {
    let cores = cores.max(1) as f32;
    let mut groups: HashMap<String, ProcessGroup> = HashMap::new();
    for reading in readings {
        let key = base_name(&reading.name);
        if key.is_empty() || key == "idle" || key == "system idle process" {
            continue;
        }
        let entry = groups.entry(key).or_insert_with(|| ProcessGroup {
            name: reading.name.clone(),
            count: 0,
            cpu_percent: 0.0,
            memory: 0,
        });
        entry.count += 1;
        entry.cpu_percent += reading.cpu_percent / cores;
        entry.memory += reading.memory;
    }
    groups.into_values().collect()
}

/// The `n` groups using the most CPU, then the `n` using the most memory.
pub fn top(groups: &[ProcessGroup], n: usize) -> (Vec<ProcessGroup>, Vec<ProcessGroup>) {
    let mut by_cpu = groups.to_vec();
    by_cpu.sort_by(|a, b| {
        b.cpu_percent
            .total_cmp(&a.cpu_percent)
            .then_with(|| a.name.cmp(&b.name))
    });
    by_cpu.truncate(n);
    let mut by_memory = groups.to_vec();
    by_memory.sort_by(|a, b| b.memory.cmp(&a.memory).then_with(|| a.name.cmp(&b.name)));
    by_memory.truncate(n);
    (by_cpu, by_memory)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(name: &str, cpu: f32, memory: u64) -> ProcessReading {
        ProcessReading {
            name: name.into(),
            cpu_percent: cpu,
            memory,
        }
    }

    #[test]
    fn processes_of_one_app_are_one_row_with_summed_cpu_and_memory() {
        let groups = group(
            vec![
                reading("chrome.exe", 40.0, 100),
                reading("chrome.exe", 20.0, 300),
                reading("Code.exe", 10.0, 50),
            ],
            4,
        );
        let chrome = groups
            .iter()
            .find(|g| g.name == "chrome.exe")
            .expect("chrome");
        assert_eq!(chrome.count, 2);
        assert_eq!(chrome.memory, 400);
        assert_eq!(chrome.cpu_percent, 15.0);
    }

    #[test]
    fn cpu_is_a_share_of_the_whole_machine_like_task_manager() {
        // Two of eight cores fully busy is 25 % of the CPU, not 200 %.
        let groups = group(vec![reading("render.exe", 200.0, 0)], 8);
        assert_eq!(groups[0].cpu_percent, 25.0);
    }

    #[test]
    fn names_group_regardless_of_case_and_the_exe_suffix() {
        let groups = group(
            vec![reading("Teams.exe", 1.0, 1), reading("teams.EXE", 1.0, 1)],
            1,
        );
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].count, 2);
    }

    #[test]
    fn the_idle_process_is_not_a_top_process() {
        let groups = group(
            vec![
                reading("System Idle Process", 750.0, 0),
                reading("Idle", 750.0, 0),
                reading("app.exe", 1.0, 1),
            ],
            8,
        );
        assert_eq!(groups.len(), 1);
    }

    #[test]
    fn top_lists_are_ordered_and_cut_to_size() {
        let groups = group(
            vec![
                reading("a.exe", 5.0, 900),
                reading("b.exe", 50.0, 100),
                reading("c.exe", 20.0, 500),
            ],
            1,
        );
        let (cpu, memory) = top(&groups, 2);
        assert_eq!(
            cpu.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(),
            ["b.exe", "c.exe"]
        );
        assert_eq!(
            memory.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(),
            ["a.exe", "c.exe"]
        );
    }

    #[test]
    fn system_processes_and_kairos_are_protected_but_apps_are_not() {
        for name in [
            "explorer.exe",
            "LSASS.EXE",
            "svchost",
            "dwm.exe",
            "kairos.exe",
            "",
        ] {
            assert!(is_protected(name), "{name}");
        }
        for name in ["notepad.exe", "chrome.exe", "explorer-helper.exe"] {
            assert!(!is_protected(name), "{name}");
        }
    }
}
