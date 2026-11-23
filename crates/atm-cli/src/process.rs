//! Other processes: whether one runs, when it started, and stopping it (the dashboard server's, a claim's watcher's
//! shell).

/// When process `pid` started, in seconds since 1970, or none when there is no such process (or it has ended): with
/// its pid, it tells a process from a later one that got the same pid.
#[must_use]
pub fn process_started(pid: u32) -> Option<u64> {
    let pid = sysinfo::Pid::from_u32(pid);
    let mut system = sysinfo::System::new().ok()?;
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    system
        .process(pid)
        .filter(|p| p.status() != sysinfo::ProcessStatus::Zombie)
        .map(sysinfo::Process::start_time)
}

/// Whether there is a process `pid` at all, one that ended but isn't reaped yet included.
#[must_use]
pub fn pid_exists(pid: u32) -> bool {
    let pid = sysinfo::Pid::from_u32(pid);
    let Ok(mut system) = sysinfo::System::new() else {
        return false;
    };
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).is_some()
}

/// The shell that ran atm, 0 when it can't be found (ATM-482: without it, a claim's watcher on Windows followed pid 0,
/// and outlived its shell). On Unix atm's parent, the kernel's answer. On Windows from sysinfo's process list, and one
/// level further up when the parent's parent is the same program: Git Bash (MSYS, as Claude Code's shell there) runs a
/// program that isn't the line's last command from a forked copy of itself, which stays only until the program ends,
/// so the shell that goes on running the line is that copy's parent.
#[must_use]
pub fn shell_pid() -> u32 {
    #[cfg(unix)]
    {
        std::os::unix::process::parent_id()
    }
    #[cfg(not(unix))]
    {
        let Ok(mut system) = sysinfo::System::new() else {
            return 0;
        };
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let parent_of = |pid: u32| {
            system
                .process(sysinfo::Pid::from_u32(pid))
                .and_then(sysinfo::Process::parent)
                .map(sysinfo::Pid::as_u32)
        };
        let name_of = |pid: u32| {
            system
                .process(sysinfo::Pid::from_u32(pid))
                .map(|p| p.name().to_string_lossy().to_lowercase())
        };
        let Some(parent) = parent_of(std::process::id()) else {
            return 0;
        };
        match parent_of(parent) {
            Some(grandparent)
                if name_of(parent).is_some() && name_of(parent) == name_of(grandparent) =>
            {
                grandparent
            }
            _ => parent,
        }
    }
}

/// A process as it was found: its pid and when it started (ATM-673, audit C14). Only that process is ever signalled:
/// once it ended, a later one given the same pid has another start time, and is left alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Running {
    pub pid: u32,
    started: u64,
}

impl Running {
    /// Process `pid` as it runs now, or none
    #[must_use]
    pub fn find(pid: u32) -> Option<Self> {
        process_started(pid).map(|started| Self { pid, started })
    }

    /// When it started, in seconds since 1970
    #[must_use]
    pub fn started(&self) -> u64 {
        self.started
    }

    /// Whether it still runs: its pid, with its start time
    #[must_use]
    pub fn alive(&self) -> bool {
        process_started(self.pid) == Some(self.started)
    }

    /// [`signal_process`], only while it is still this process. Whether the signal was sent
    #[must_use]
    pub fn signal(&self, kill: bool) -> bool {
        self.alive() && signal_process(self.pid, kill)
    }
}

/// Whether process `pid` is an atm dashboard server, `atm _serve …` (ATM-673, audit C6): what `atm service stop`
/// may stop. A whoami answer names a pid, but anyone on the machine can answer on a local port, so the pid is never
/// taken on its word.
#[must_use]
pub fn is_atm_server(pid: u32) -> bool {
    let pid = sysinfo::Pid::from_u32(pid);
    let Ok(mut system) = sysinfo::System::new() else {
        return false;
    };
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[pid]),
        true,
        sysinfo::ProcessRefreshKind::nothing()
            .with_exe(sysinfo::UpdateKind::Always)
            .with_cmd(sysinfo::UpdateKind::Always),
    );
    system.process(pid).is_some_and(|process| {
        let program = process
            .exe()
            .and_then(std::path::Path::file_stem)
            .is_some_and(|name| name == "atm");
        let serving = process.cmd().get(1).is_some_and(|arg| arg == "_serve");
        program && serving
    })
}

/// Ask process `pid` to stop (SIGTERM), or with `kill`, end it (SIGKILL). Whether the signal was sent.
#[must_use]
pub fn signal_process(pid: u32, kill: bool) -> bool {
    let pid = sysinfo::Pid::from_u32(pid);
    let Ok(mut system) = sysinfo::System::new() else {
        return false;
    };
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    let signal = if kill {
        sysinfo::Signal::Kill
    } else {
        sysinfo::Signal::Term
    };
    system
        .process(pid)
        .and_then(|p| p.kill_with(signal))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_runs_and_started_before_now() {
        let me = std::process::id();
        assert!(Running::find(me).is_some_and(|r| r.alive()) && pid_exists(me));
        assert!(process_started(me).is_some_and(|t| t > 0));
    }

    #[test]
    fn the_shell_is_a_running_process() {
        let parent = shell_pid();
        assert_ne!(parent, 0);
        assert!(Running::find(parent).is_some());
    }

    /// ATM-673, audit C14: a process is known by its start time too; another start time is another process, never
    /// signalled
    #[test]
    fn a_process_is_its_pid_and_its_start_time() {
        let me = Running::find(std::process::id()).unwrap();
        assert!(me.alive());
        let later = Running {
            started: me.started + 1,
            ..me
        };
        assert!(!later.alive());
        assert!(
            !later.signal(false),
            "a process with another start time is never signalled"
        );
    }

    /// ATM-673, audit C6: only `atm _serve` is a dashboard server; this test binary isn't one
    #[test]
    fn only_atm_serve_is_a_dashboard_server() {
        assert!(!is_atm_server(std::process::id()));
        assert!(!is_atm_server(u32::MAX));
    }
}
