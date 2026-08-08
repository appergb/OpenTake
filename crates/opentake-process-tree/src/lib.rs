//! Cross-platform containment for untrusted helper processes.

use std::io;
use std::process::Command;

/// Configure a command before spawn so descendants can be terminated as a
/// unit. Call [`ProcessTree::attach`] immediately after a successful spawn.
///
/// Windows children start with their primary thread suspended. `attach` first
/// places that inert process in a kill-on-close Job Object and only then resumes
/// it, closing the spawn-to-assignment escape window.
pub fn configure_command(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, CREATE_SUSPENDED};
        command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_SUSPENDED);
    }
}

#[cfg(windows)]
mod windows_containment {
    use super::*;
    use std::mem::size_of;
    use std::ptr;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, OpenThread, ResumeThread, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        THREAD_SUSPEND_RESUME,
    };

    struct OwnedHandle(HANDLE);

    impl OwnedHandle {
        fn from_nullable(handle: HANDLE) -> io::Result<Self> {
            if handle.is_null() {
                Err(io::Error::last_os_error())
            } else {
                Ok(Self(handle))
            }
        }

        fn from_snapshot(handle: HANDLE) -> io::Result<Self> {
            if handle == INVALID_HANDLE_VALUE {
                Err(io::Error::last_os_error())
            } else {
                Ok(Self(handle))
            }
        }

        fn raw(&self) -> HANDLE {
            self.0
        }

        fn into_raw(mut self) -> HANDLE {
            let handle = self.0;
            self.0 = ptr::null_mut();
            handle
        }
    }

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
                // SAFETY: this value owns the handle and closes it exactly once.
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
    }

    fn open_primary_thread(process_id: u32) -> io::Result<OwnedHandle> {
        // The target was created suspended, so it can only have its original
        // primary thread while this snapshot is inspected.
        // SAFETY: this call takes no borrowed pointers; the documented thread
        // snapshot flag ignores the process-id argument. `OwnedHandle` checks
        // the sentinel and assumes sole ownership of a successful result.
        let snapshot =
            OwnedHandle::from_snapshot(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })?;
        let mut entry = THREADENTRY32 {
            dwSize: size_of::<THREADENTRY32>() as u32,
            ..THREADENTRY32::default()
        };
        // SAFETY: `entry` has the required size and remains valid throughout
        // the enumeration; `snapshot` is a live ToolHelp snapshot.
        let mut has_entry = unsafe { Thread32First(snapshot.raw(), &mut entry) } != 0;
        while has_entry {
            if entry.th32OwnerProcessID == process_id {
                // SAFETY: the thread id comes from the live snapshot and only
                // suspend/resume access is requested.
                return OwnedHandle::from_nullable(unsafe {
                    OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID)
                });
            }
            // SAFETY: same snapshot and initialized output structure as above.
            has_entry = unsafe { Thread32Next(snapshot.raw(), &mut entry) } != 0;
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "suspended child primary thread was not found",
        ))
    }

    pub(super) fn attach(process_id: u32) -> io::Result<HANDLE> {
        // SAFETY: null pointers request an unnamed job with default security.
        let job =
            OwnedHandle::from_nullable(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) })?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: structure and information class are paired correctly.
        if unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }

        // The process has not executed any user code, so even an OpenProcess or
        // assignment failure leaves no descendant to escape. The caller kills
        // and reaps the still-suspended immediate child on every attach error.
        // SAFETY: `process_id` identifies that live suspended child; the access
        // mask is sufficient for job assignment/termination, and a successful
        // handle is transferred into the single-owner `OwnedHandle` wrapper.
        let process = OwnedHandle::from_nullable(unsafe {
            OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, process_id)
        })?;
        // SAFETY: both handles are valid and have the documented rights.
        if unsafe { AssignProcessToJobObject(job.raw(), process.raw()) } == 0 {
            return Err(io::Error::last_os_error());
        }

        let primary_thread = open_primary_thread(process_id)?;
        // SAFETY: this is the primary thread created by CREATE_SUSPENDED. It is
        // resumed only after successful Job Object assignment.
        let previous_suspend_count = unsafe { ResumeThread(primary_thread.raw()) };
        if previous_suspend_count != 1 {
            return Err(if previous_suspend_count == u32::MAX {
                io::Error::last_os_error()
            } else {
                io::Error::other(format!(
                    "unexpected suspended child thread count: {previous_suspend_count}"
                ))
            });
        }

        Ok(job.into_raw())
    }

    pub(super) fn terminate(job: HANDLE) -> io::Result<()> {
        // SAFETY: the caller owns a live Job Object handle.
        if unsafe { TerminateJobObject(job, 1) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

/// Owns the operating-system containment object for one spawned process tree.
/// Dropping an armed value is fail-closed and terminates the tree.
pub struct ProcessTree {
    #[cfg(unix)]
    process_group: i32,
    #[cfg(windows)]
    job: windows_sys::Win32::Foundation::HANDLE,
    armed: bool,
}

// SAFETY: on Windows the owned Job Object HANDLE is an opaque kernel value with
// exclusive ownership here — it is moved, never shared, and closed exactly once
// (Drop/disarm). Moving the handle between threads is therefore sound; it is
// only ever used through this value's own methods.
unsafe impl Send for ProcessTree {}

impl ProcessTree {
    /// Attach to a child spawned from a command prepared by
    /// [`configure_command`].
    ///
    /// Call this immediately after spawn and before doing any other work with
    /// the child. On Windows the configured child remains suspended until this
    /// method assigns it to the Job Object; on Unix the configuration creates
    /// the isolated process group stored here.
    pub fn attach(process_id: u32) -> io::Result<Self> {
        // On Unix, negating these reserved identifiers would change the
        // target from one isolated child process group to the caller's group
        // (`kill(0, ...)`) or every permitted process (`kill(-1, ...)`). Keep
        // the public cross-platform API fail-closed even if it is called with
        // an identifier that did not come from `Child::id()`.
        if process_id <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "process id must identify an isolated child process",
            ));
        }
        #[cfg(unix)]
        {
            let process_group = i32::try_from(process_id)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid child pid"))?;
            Ok(Self {
                process_group,
                armed: true,
            })
        }
        #[cfg(windows)]
        {
            let job = windows_containment::attach(process_id)?;
            Ok(Self { job, armed: true })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = process_id;
            Ok(Self { armed: true })
        }
    }

    pub fn terminate(&self) -> io::Result<()> {
        if !self.armed {
            return Ok(());
        }
        #[cfg(unix)]
        {
            // A negative pid targets the entire child process group.
            // SAFETY: `attach` rejects process groups 0 and 1, and the public
            // launch contract requires this value to come from a child whose
            // command was prepared by `configure_command`, which makes its PID
            // the isolated process-group id. No pointers cross the FFI call.
            let result = unsafe { libc::kill(-self.process_group, libc::SIGKILL) };
            if result != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
            }
        }
        #[cfg(windows)]
        {
            windows_containment::terminate(self.job)?;
        }
        Ok(())
    }

    /// Mark normal completion and release containment without termination.
    pub fn disarm(&mut self) {
        self.armed = false;
        #[cfg(windows)]
        if !self.job.is_null() {
            // SAFETY: this value owns and closes the job handle exactly once.
            unsafe {
                let _ = windows_sys::Win32::Foundation::CloseHandle(self.job);
            }
            self.job = std::ptr::null_mut();
        }
    }
}

impl Drop for ProcessTree {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.terminate();
        }
        #[cfg(windows)]
        if !self.job.is_null() {
            // SAFETY: this value exclusively owns the non-null Job Object
            // handle and closes it exactly once here; disarm nulls it after an
            // earlier close. KILL_ON_JOB_CLOSE is a second fail-closed boundary.
            unsafe {
                let _ = windows_sys::Win32::Foundation::CloseHandle(self.job);
            }
            self.job = std::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    #[test]
    fn attach_rejects_process_ids_with_group_or_broadcast_semantics() {
        for process_id in [0, 1] {
            match ProcessTree::attach(process_id) {
                Err(error) => assert_eq!(error.kind(), io::ErrorKind::InvalidInput),
                Ok(tree) => {
                    // Leak an invalid regression value so its Drop cannot
                    // exercise kill(0) or kill(-1) in the test runner.
                    std::mem::forget(tree);
                    panic!("reserved process id must be rejected before containment is armed");
                }
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Stdio;
    use std::thread;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    const TEST_NAME: &str = "tests::windows_suspended_job_contains_fast_exit_descendant";
    const MODE_ENV: &str = "OPENTAKE_PROCESS_TREE_TEST_MODE";
    const DIR_ENV: &str = "OPENTAKE_PROCESS_TREE_TEST_DIR";

    struct TestProcessHandle(HANDLE);

    impl Drop for TestProcessHandle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: the test owns this process handle.
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
    }

    fn wait_for_file(path: &std::path::Path, deadline: Instant) {
        while !path.is_file() {
            assert!(Instant::now() < deadline, "timed out waiting for {path:?}");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn helper_mode() {
        let Some(mode) = std::env::var_os(MODE_ENV) else {
            return;
        };
        let directory = std::path::PathBuf::from(
            std::env::var_os(DIR_ENV).expect("helper test directory must be supplied"),
        );
        if mode == "fast-parent" {
            let child = Command::new(std::env::current_exe().expect("current test executable"))
                .args(["--exact", TEST_NAME, "--nocapture", "--test-threads=1"])
                .env(MODE_ENV, "grandchild")
                .env(DIR_ENV, &directory)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn grandchild helper");
            fs::write(directory.join("grandchild.pid"), child.id().to_string())
                .expect("publish grandchild pid");
            // Intentionally exit without waiting. The grandchild must remain in
            // the parent's already-assigned Job Object despite this fast exit.
            std::process::exit(0);
        }
        if mode == "grandchild" {
            fs::write(directory.join("grandchild.ready"), b"ready")
                .expect("publish grandchild readiness");
            thread::sleep(Duration::from_secs(60));
            std::process::exit(0);
        }
        panic!("unknown process-tree helper mode: {mode:?}");
    }

    #[test]
    fn windows_suspended_job_contains_fast_exit_descendant() {
        helper_mode();

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "opentake-process-tree-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create process-tree test directory");
        let mut command = Command::new(std::env::current_exe().expect("current test executable"));
        command
            .args(["--exact", TEST_NAME, "--nocapture", "--test-threads=1"])
            .env(MODE_ENV, "fast-parent")
            .env(DIR_ENV, &directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_command(&mut command);
        let mut parent = command.spawn().expect("spawn suspended parent helper");
        let mut tree = ProcessTree::attach(parent.id()).expect("attach and resume parent helper");

        let deadline = Instant::now() + Duration::from_secs(10);
        let pid_path = directory.join("grandchild.pid");
        let ready_path = directory.join("grandchild.ready");
        wait_for_file(&pid_path, deadline);
        wait_for_file(&ready_path, deadline);
        let grandchild_pid = fs::read_to_string(pid_path)
            .expect("read grandchild pid")
            .parse::<u32>()
            .expect("parse grandchild pid");

        let parent_status = loop {
            match parent.try_wait().expect("poll fast parent") {
                Some(status) => break status,
                None => {
                    assert!(Instant::now() < deadline, "fast parent did not exit");
                    thread::sleep(Duration::from_millis(10));
                }
            }
        };
        assert!(parent_status.success());

        // SAFETY: `grandchild_pid` was read from the just-spawned helper. The
        // requested access is query-only, and `TestProcessHandle` exclusively
        // owns and closes a successful handle exactly once.
        let grandchild = TestProcessHandle(unsafe {
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, grandchild_pid)
        });
        assert!(
            !grandchild.0.is_null(),
            "grandchild exited before containment check"
        );

        tree.terminate().expect("terminate contained job");
        tree.disarm();
        let exit_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let mut exit_code = 0_u32;
            // SAFETY: the held process handle stays valid for the loop.
            assert_ne!(
                unsafe { GetExitCodeProcess(grandchild.0, &mut exit_code) },
                0
            );
            if exit_code != STILL_ACTIVE as u32 {
                break;
            }
            assert!(
                Instant::now() < exit_deadline,
                "grandchild survived Job Object termination"
            );
            thread::sleep(Duration::from_millis(10));
        }
        fs::remove_dir_all(&directory).expect("remove process-tree test directory");
    }
}
