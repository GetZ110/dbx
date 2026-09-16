//! OHOS **native child process** transport for agent drivers (route D).
//!
//! HarmonyOS 6 refuses to `execve` an ELF that lives in the app sandbox (BinSec /
//! `code_protect`): the sandboxed agent binary the driver manager downloads comes
//! back as `Permission denied (os error 13)`. The supported way to run native code
//! under the app identity is `OH_Ability_StartNativeChildProcess`, where appspawn
//! `dlopen`s a library out of the HAP's `libs/` directory and calls its exported
//! `Main(NativeChildProcess_Args)`. No execute bit and no Huawei signing material
//! are involved.
//!
//! The child cannot inherit the parent's pipes, so the parent creates a
//! `socketpair`, hands one end over in the `fdList`, and the bundled agent shim
//! (`agents/drivers/*/ohos_ncp_shim.c`) `dup2`s it onto fd 0/1. The existing
//! stdin/stdout JSON-RPC protocol then rides on the socket unchanged.
//!
//! See `docs/ohos-agent-exec-denied.md` §13/§14 for the full evidence chain.
//!
//! Everything here is `target_env = "ohos"` only; other targets get a stub so
//! `cargo check` keeps working on a desktop host.

#![cfg_attr(not(target_env = "ohos"), allow(dead_code, unused_variables))]

use std::io;
use std::process::ExitStatus;

#[cfg(target_env = "ohos")]
mod imp {
    use super::*;
    use std::ffi::CString;
    use std::net::Shutdown;
    use std::os::fd::AsRawFd;
    use std::os::raw::{c_char, c_int};
    use std::os::unix::net::UnixStream;
    use std::os::unix::process::ExitStatusExt;
    use std::ptr;

    #[repr(C)]
    struct NativeChildProcessFd {
        fd_name: *mut c_char,
        fd: i32,
        next: *mut NativeChildProcessFd,
    }

    #[repr(C)]
    struct NativeChildProcessFdList {
        head: *mut NativeChildProcessFd,
    }

    #[repr(C)]
    struct NativeChildProcessArgs {
        entry_params: *mut c_char,
        fd_list: NativeChildProcessFdList,
    }

    #[repr(C)]
    struct NativeChildProcessOptions {
        isolation_mode: c_int,
        reserved: i64,
    }

    #[link(name = "child_process")]
    extern "C" {
        fn OH_Ability_StartNativeChildProcess(
            entry: *const c_char,
            args: NativeChildProcessArgs,
            options: NativeChildProcessOptions,
            pid: *mut i32,
        ) -> c_int;
        fn OH_Ability_KillChildProcess(pid: i32) -> c_int;
    }

    /// Handle to one appspawn-managed native child process.
    ///
    /// The child is a child of `appspawn`, not of this process, so `waitpid(2)`
    /// does not apply. The supported ways to observe it are the exit callback
    /// (`OH_Ability_RegisterNativeChildProcessExitCallback`) and
    /// `OH_Ability_KillChildProcess`; closing the socket makes a well-behaved
    /// agent return from `Main` and exit on its own.
    pub struct NcpChild {
        pid: i32,
        /// A clone of the parent socket end, kept only to shut the channel down.
        ctl: UnixStream,
    }

    /// Starts `entry` (for example `libdbx_agent_oracle.so:Main`) as a native
    /// child process whose fd 0/1 are the child end of a fresh socketpair.
    ///
    /// Returns the child handle and the parent end of the socketpair.
    pub fn spawn(entry: &str) -> io::Result<(NcpChild, UnixStream)> {
        let (parent_stream, child_stream) = UnixStream::pair()?;
        let fd_name = CString::new("agent")
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "fd name contained NUL"))?;
        let mut fd_node = NativeChildProcessFd {
            fd_name: fd_name.as_ptr() as *mut c_char,
            fd: child_stream.as_raw_fd(),
            next: ptr::null_mut(),
        };
        let entry_c = CString::new(entry)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "entry point contained NUL"))?;

        let args = NativeChildProcessArgs {
            entry_params: ptr::null_mut(),
            fd_list: NativeChildProcessFdList { head: &mut fd_node },
        };
        let options = NativeChildProcessOptions { isolation_mode: 0, reserved: 0 };

        let mut pid: i32 = 0;
        let err = unsafe { OH_Ability_StartNativeChildProcess(entry_c.as_ptr(), args, options, &mut pid) };
        if err != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("OH_Ability_StartNativeChildProcess({entry}) failed, err={err}"),
            ));
        }

        // The child owns its dup now; dropping our end makes an agent crash show
        // up as EOF instead of a half-open channel.
        drop(child_stream);
        let ctl = parent_stream.try_clone()?;
        Ok((NcpChild { pid, ctl }, parent_stream))
    }

    impl NcpChild {
        pub fn id(&self) -> u32 {
            self.pid as u32
        }

        pub fn kill(&mut self) -> io::Result<()> {
            let _ = self.ctl.shutdown(Shutdown::Both);
            let rc = unsafe { OH_Ability_KillChildProcess(self.pid) };
            if rc == 0 {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("OH_Ability_KillChildProcess({}) failed, err={rc}", self.pid),
                ))
            }
        }

        /// No `waitpid` for appspawn children: closing the socket is the teardown
        /// signal, so report completion without blocking the reaper thread.
        pub fn wait(&mut self) -> io::Result<ExitStatus> {
            let _ = self.ctl.shutdown(Shutdown::Both);
            Ok(ExitStatus::from_raw(0))
        }

        pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
            Ok(None)
        }
    }
}

#[cfg(not(target_env = "ohos"))]
mod imp {
    use super::*;
    use std::os::unix::net::UnixStream;

    pub struct NcpChild;

    pub fn spawn(entry: &str) -> io::Result<(NcpChild, UnixStream)> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("native child processes are only supported on OHOS (entry {entry})"),
        ))
    }

    impl NcpChild {
        pub fn id(&self) -> u32 {
            0
        }

        pub fn kill(&mut self) -> io::Result<()> {
            Ok(())
        }

        pub fn wait(&mut self) -> io::Result<ExitStatus> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "native child processes are only supported on OHOS"))
        }

        pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
            Ok(None)
        }
    }
}

pub use imp::{spawn, NcpChild};
