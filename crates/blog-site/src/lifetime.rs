//! Keep long-running helpers (Zola serve, Typst watch) from outliving blogctl
//! when it is killed instead of stopped with Ctrl+C.

use std::process::Command;

/// On Windows, put this process in a job object that kills every member when
/// its last handle closes, so children started afterwards die with us.
#[cfg(windows)]
pub fn kill_children_on_exit() {
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    // SAFETY: plain Win32 calls on handles we own; the job handle is
    // deliberately leaked so it stays open for the life of the process.
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return;
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            size,
        ) != 0
        {
            AssignProcessToJobObject(job, GetCurrentProcess());
        }
    }
}

#[cfg(not(windows))]
pub fn kill_children_on_exit() {}

/// On Linux, ask the kernel to stop the child when blogctl exits.
pub fn tie_to_parent(command: &mut Command) -> &mut Command {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: prctl is async-signal-safe and only affects the child.
        unsafe {
            command.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
    }
    command
}
