//! Starting a process that outlives atm (the claim's watcher, the dashboard server), detached from whatever ran atm,
//! so that its caller sees atm's output end when atm exits (T-480). On Unix: a process group of its own. On Windows:
//! no console and a process group of its own, and none of atm's standard handles inherited. Windows passes a child
//! every inheritable handle of its parent, the caller's pipes among them, and a caller reading atm's output would
//! wait for the background process to end too.

use std::io;
use std::process::{Child, Command};

/// Start `command` detached. Its standard input and output are the caller's to set (the null device, a log file).
///
/// # Errors
/// As `Command::spawn`.
pub fn spawn_detached(command: &mut Command) -> io::Result<Child> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
        command.spawn()
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP: no console of the caller's, and the caller's Ctrl+C doesn't
        // reach it
        command.creation_flags(0x0000_0008 | 0x0000_0200);
        let inherited = windows::stop_inheriting();
        let child = command.spawn();
        windows::restore(&inherited);
        child
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows {
    use windows_sys::Win32::Foundation::{
        GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE,
        SetHandleInformation,
    };
    use windows_sys::Win32::System::Console::{
        GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };

    /// Mark atm's standard handles not inheritable; returns those that were, to restore.
    pub fn stop_inheriting() -> Vec<HANDLE> {
        let mut were = Vec::new();
        for which in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            // SAFETY: GetStdHandle takes one of the three constants above and returns this process's handle for it,
            // or null or INVALID_HANDLE_VALUE, which are skipped; GetHandleInformation and SetHandleInformation only
            // read and change that handle's flags, and `flags` outlives the call that writes it
            unsafe {
                let handle = GetStdHandle(which);
                if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                    continue;
                }
                let mut flags = 0u32;
                if GetHandleInformation(handle, &raw mut flags) != 0
                    && flags & HANDLE_FLAG_INHERIT != 0
                    && SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) != 0
                {
                    were.push(handle);
                }
            }
        }
        were
    }

    /// Make the handles inheritable again, for atm's own children afterwards (git, with atm's stderr).
    pub fn restore(handles: &[HANDLE]) {
        for &handle in handles {
            // SAFETY: a handle stop_inheriting just took from GetStdHandle; only its inherit flag changes
            unsafe {
                SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT);
            }
        }
    }
}
