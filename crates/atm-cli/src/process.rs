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

/// Whether process `pid` runs (an ended one nobody reaped yet doesn't).
#[must_use]
pub fn pid_alive(pid: u32) -> bool {
    process_started(pid).is_some()
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
        assert!(pid_alive(me) && pid_exists(me));
        assert!(process_started(me).is_some_and(|t| t > 0));
    }
}
