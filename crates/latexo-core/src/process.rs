//! The process list: one [`ProcessInfo`] per process, and the sorting, filtering and searching of
//! the Processes screen ([`ProcessQuery`]).

use std::cmp::Ordering;

/// One running process.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessInfo {
    /// Process id.
    pub pid: u32,
    /// Parent process id.
    pub parent: Option<u32>,
    /// The executable name: `firefox`, `chrome.exe`.
    pub name: String,
    /// The owner's user name, `None` when it cannot be read.
    pub user: Option<String>,
    /// CPU in percent of one logical processor, so a busy process with four threads can show
    /// 400 % (as `top` and the design's "% of one thread").
    pub cpu_percent: f32,
    /// Resident memory in bytes.
    pub memory: u64,
    /// Number of threads, when the platform reports it.
    pub threads: Option<u32>,
    /// Bytes read per second over the last interval.
    pub disk_read_per_sec: f64,
    /// Bytes written per second over the last interval.
    pub disk_write_per_sec: f64,
    /// Owned by the user running Latexo, so an app rather than a system process.
    pub own: bool,
}

impl ProcessInfo {
    /// Disk throughput, read and write together.
    pub fn disk_per_sec(&self) -> f64 {
        self.disk_read_per_sec + self.disk_write_per_sec
    }
}

/// The columns of the process table, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessColumn {
    /// The name.
    Name,
    /// The process id.
    Pid,
    /// The owner.
    User,
    /// CPU usage, the default.
    #[default]
    Cpu,
    /// Resident memory.
    Memory,
    /// Threads.
    Threads,
    /// Disk throughput.
    Disk,
}

impl ProcessColumn {
    /// Every column, in the table's order.
    pub const ALL: [ProcessColumn; 7] = [
        ProcessColumn::Name,
        ProcessColumn::Pid,
        ProcessColumn::User,
        ProcessColumn::Cpu,
        ProcessColumn::Memory,
        ProcessColumn::Threads,
        ProcessColumn::Disk,
    ];

    /// Whether a first click sorts ascending: text columns read A to Z, numbers biggest first.
    pub fn ascending_first(self) -> bool {
        matches!(
            self,
            ProcessColumn::Name | ProcessColumn::User | ProcessColumn::Pid
        )
    }
}

/// Which processes the table shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessFilter {
    /// Every process.
    #[default]
    All,
    /// The user's own processes.
    Apps,
    /// Everyone else's: the system and its services.
    System,
}

/// How the table is sorted, filtered and searched.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProcessQuery {
    /// The sorted column.
    pub column: ProcessColumn,
    /// Ascending order.
    pub ascending: bool,
    /// The filter.
    pub filter: ProcessFilter,
    /// The search text: part of the name, or the start of the PID.
    pub search: String,
}

impl ProcessQuery {
    /// The query after a click on a column header: the same column flips the order, another one
    /// starts in its natural order.
    pub fn clicked(&mut self, column: ProcessColumn) {
        if self.column == column {
            self.ascending = !self.ascending;
        } else {
            self.column = column;
            self.ascending = column.ascending_first();
        }
    }

    /// Whether `process` passes the filter and the search.
    pub fn matches(&self, process: &ProcessInfo) -> bool {
        let filtered = match self.filter {
            ProcessFilter::All => true,
            ProcessFilter::Apps => process.own,
            ProcessFilter::System => !process.own,
        };
        if !filtered {
            return false;
        }
        let search = self.search.trim();
        if search.is_empty() {
            return true;
        }
        if search.chars().all(|c| c.is_ascii_digit()) && process.pid.to_string().starts_with(search)
        {
            return true;
        }
        process.name.to_lowercase().contains(&search.to_lowercase())
    }

    /// The processes that match, sorted. Ties keep a stable order by PID.
    pub fn apply<'a>(&self, processes: &'a [ProcessInfo]) -> Vec<&'a ProcessInfo> {
        let mut rows: Vec<&ProcessInfo> = processes.iter().filter(|p| self.matches(p)).collect();
        rows.sort_by(|a, b| {
            let order = compare(self.column, a, b);
            let order = if self.ascending {
                order
            } else {
                order.reverse()
            };
            order.then(a.pid.cmp(&b.pid))
        });
        rows
    }
}

fn compare(column: ProcessColumn, a: &ProcessInfo, b: &ProcessInfo) -> Ordering {
    match column {
        ProcessColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        ProcessColumn::Pid => a.pid.cmp(&b.pid),
        ProcessColumn::User => a.user.cmp(&b.user),
        ProcessColumn::Cpu => a.cpu_percent.total_cmp(&b.cpu_percent),
        ProcessColumn::Memory => a.memory.cmp(&b.memory),
        ProcessColumn::Threads => a.threads.cmp(&b.threads),
        ProcessColumn::Disk => a.disk_per_sec().total_cmp(&b.disk_per_sec()),
    }
}

/// The `count` processes using the most CPU, busiest first.
pub fn busiest(processes: &[ProcessInfo], count: usize) -> Vec<&ProcessInfo> {
    top_by(processes, count, |p| p.cpu_percent as f64)
}

/// The `count` processes with the most disk throughput, ignoring idle ones.
pub fn top_disk(processes: &[ProcessInfo], count: usize) -> Vec<&ProcessInfo> {
    top_by(processes, count, ProcessInfo::disk_per_sec)
        .into_iter()
        .filter(|p| p.disk_per_sec() > 0.0)
        .collect()
}

/// The `count` processes using the most memory.
pub fn top_memory(processes: &[ProcessInfo], count: usize) -> Vec<&ProcessInfo> {
    top_by(processes, count, |p| p.memory as f64)
}

fn top_by(
    processes: &[ProcessInfo],
    count: usize,
    key: impl Fn(&ProcessInfo) -> f64,
) -> Vec<&ProcessInfo> {
    let mut rows: Vec<&ProcessInfo> = processes.iter().collect();
    rows.sort_by(|a, b| key(b).total_cmp(&key(a)).then(a.pid.cmp(&b.pid)));
    rows.truncate(count);
    rows
}

/// The PIDs of `root` and every descendant, `root` first, for "end the process tree".
pub fn tree(processes: &[ProcessInfo], root: u32) -> Vec<u32> {
    let mut found = vec![root];
    let mut index = 0;
    while index < found.len() {
        let parent = found[index];
        for process in processes {
            if process.parent == Some(parent) && !found.contains(&process.pid) {
                found.push(process.pid);
            }
        }
        index += 1;
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn process(pid: u32, name: &str, cpu: f32, own: bool) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.to_string(),
            cpu_percent: cpu,
            memory: u64::from(pid) * 10,
            own,
            ..ProcessInfo::default()
        }
    }

    fn pids(rows: &[&ProcessInfo]) -> Vec<u32> {
        rows.iter().map(|p| p.pid).collect()
    }

    #[test]
    fn sorts_by_cpu_descending_by_default() {
        let list = [
            process(1, "a", 5.0, true),
            process(2, "b", 50.0, false),
            process(3, "c", 20.0, true),
        ];
        let query = ProcessQuery::default();
        assert_eq!(pids(&query.apply(&list)), vec![2, 3, 1]);
    }

    #[test]
    fn clicking_a_header_flips_or_starts_in_its_natural_order() {
        let mut query = ProcessQuery::default();
        query.clicked(ProcessColumn::Name);
        assert_eq!(query.column, ProcessColumn::Name);
        assert!(query.ascending);
        query.clicked(ProcessColumn::Name);
        assert!(!query.ascending);
        query.clicked(ProcessColumn::Memory);
        assert!(!query.ascending);
    }

    #[test]
    fn filters_and_searches() {
        let list = [
            process(10, "Firefox", 1.0, true),
            process(102, "systemd", 1.0, false),
            process(7, "firewalld", 1.0, false),
        ];
        let mut query = ProcessQuery {
            filter: ProcessFilter::Apps,
            ..ProcessQuery::default()
        };
        assert_eq!(pids(&query.apply(&list)), vec![10]);
        query.filter = ProcessFilter::System;
        assert_eq!(pids(&query.apply(&list)), vec![7, 102]);
        query.filter = ProcessFilter::All;
        query.search = "FIRE".to_string();
        assert_eq!(pids(&query.apply(&list)), vec![7, 10]);
        query.search = "10".to_string();
        assert_eq!(pids(&query.apply(&list)), vec![10, 102]);
    }

    #[test]
    fn busiest_and_top_disk() {
        let mut list = vec![process(1, "a", 5.0, true), process(2, "b", 50.0, false)];
        list[0].disk_write_per_sec = 100.0;
        assert_eq!(pids(&busiest(&list, 1)), vec![2]);
        assert_eq!(pids(&top_disk(&list, 5)), vec![1]);
        assert_eq!(pids(&top_memory(&list, 5)), vec![2, 1]);
    }

    #[test]
    fn the_tree_holds_every_descendant() {
        let mut list = vec![
            process(1, "root", 0.0, true),
            process(2, "child", 0.0, true),
            process(3, "grandchild", 0.0, true),
            process(4, "other", 0.0, true),
        ];
        list[1].parent = Some(1);
        list[2].parent = Some(2);
        assert_eq!(tree(&list, 1), vec![1, 2, 3]);
        assert_eq!(tree(&list, 4), vec![4]);
    }
}
