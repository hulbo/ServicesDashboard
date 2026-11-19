//! 프로세스 트리 묶음.
//!
//! Windows: Job Object에 서비스 프로세스를 넣는다. 자식·손자 프로세스도 같은 Job에 들어가므로
//! `terminate()` 한 번으로 트리 전체가 종료된다. `KILL_ON_JOB_CLOSE` 덕분에 대시보드가
//! 비정상 종료돼 핸들이 닫혀도 서비스 프로세스가 고아로 남지 않는다.
//!
//! 그 외 OS: 새 프로세스 그룹으로 실행하고 그룹 전체에 SIGKILL을 보낸다.

use std::io;
use std::process::{Child, Command};

#[cfg(windows)]
mod imp {
    use super::*;
    use std::ffi::c_void;
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
        TerminateJobObject,
    };
    use windows_sys::Win32::System::Threading::{
        CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    pub struct Job {
        handle: HANDLE,
    }

    // HANDLE은 스레드 간에 공유해도 안전한 커널 객체 핸들이다.
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}

    impl Job {
        pub fn new() -> io::Result<Self> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    let err = io::Error::last_os_error();
                    CloseHandle(handle);
                    return Err(err);
                }
                Ok(Job { handle })
            }
        }

        pub fn assign(&self, child: &Child) -> io::Result<()> {
            let ok = unsafe { AssignProcessToJobObject(self.handle, child.as_raw_handle() as HANDLE) };
            if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
        }

        pub fn terminate(&self) {
            unsafe {
                TerminateJobObject(self.handle, 1);
            }
        }

        /// 이 Job에 속한 프로세스인지 (포트 점유 프로세스가 어느 서비스 것인지 알아낼 때 쓴다)
        pub fn contains(&self, pid: u32) -> bool {
            unsafe {
                let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                if h.is_null() {
                    return false;
                }
                let mut result = 0;
                let ok = IsProcessInJob(h, self.handle, &mut result);
                CloseHandle(h);
                ok != 0 && result != 0
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }

    /// 콘솔 창을 띄우지 않고, 나중에 Ctrl+Break를 보낼 수 있도록 새 프로세스 그룹으로 실행한다.
    pub fn configure(cmd: &mut Command) {
        cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use std::os::unix::process::CommandExt;
    use std::sync::atomic::{AtomicU32, Ordering};

    pub struct Job {
        pgid: AtomicU32,
    }

    impl Job {
        pub fn new() -> io::Result<Self> {
            Ok(Job { pgid: AtomicU32::new(0) })
        }

        pub fn assign(&self, child: &Child) -> io::Result<()> {
            self.pgid.store(child.id(), Ordering::SeqCst);
            Ok(())
        }

        pub fn contains(&self, _pid: u32) -> bool {
            false
        }

        pub fn terminate(&self) {
            let pgid = self.pgid.load(Ordering::SeqCst);
            if pgid != 0 {
                let _ = Command::new("kill").arg("-KILL").arg(format!("-{pgid}")).status();
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            self.terminate();
        }
    }

    pub fn configure(cmd: &mut Command) {
        cmd.process_group(0);
    }
}

pub use imp::{Job, configure};
