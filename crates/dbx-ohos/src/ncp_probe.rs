//! Native-child-process (NCP) agent transport probe.
//!
//! Proves the parent half of route D (see docs/ohos-agent-exec-denied.md §13/§14):
//! the parent process creates a `socketpair`, hands one end to an appspawn child
//! process through `OH_Ability_StartNativeChildProcess`, and then speaks the
//! existing agent JSON-RPC protocol over that fd. No file execution bit and no
//! Huawei signing material are involved - appspawn `dlopen`s the agent library
//! out of the HAP's `libs/` directory and runs it under the app identity.
//!
//! Only built for OHOS (`target_env = "ohos"`); every other target gets a stub so
//! `cargo check` stays usable on a desktop host.

#![cfg_attr(not(target_env = "ohos"), allow(dead_code, unused_variables))]

use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

/// Maximum time to wait for a single protocol line from the agent.
/* 原生探测会阻塞 ArkTS 主线程，别把超时设得太大（系统 6s 看门狗）。 */
const READ_TIMEOUT: Duration = Duration::from_secs(3);
/// The agent prints this before serving requests (see agents/drivers/*/main.go).
const AGENT_READY_MARKER: &str = "\"ready\"";
#[cfg(target_env = "ohos")]
mod imp {
  use super::*;
  use std::ffi::CString;
  use std::os::fd::AsRawFd;
  use std::os::raw::{c_char, c_int};
  use std::os::unix::net::UnixStream;
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
  }

  /// Starts `entry` (for example `libdbx_agent_oracle.so:Main`) as a native child
  /// process whose stdin/stdout are wired to one end of a socketpair, waits for the
  /// agent's ready line, sends `request` and returns `(ready_line, response_line, pid)`.
  pub fn start_and_exchange(
    entry: &str,
    request: &str,
    entry_params: Option<&str>,
  ) -> Result<(String, String, i32), String> {
    let (parent_stream, child_stream) = UnixStream::pair().map_err(|e| format!("socketpair failed: {e}"))?;

    let fd_name = CString::new("agent").map_err(|e| format!("fd name: {e}"))?;
    let mut fd_node = NativeChildProcessFd {
      // The child receives a dup of this fd and our shim looks it up by name.
      fd_name: fd_name.as_ptr() as *mut c_char,
      fd: child_stream.as_raw_fd(),
      next: ptr::null_mut(),
    };
    let mut entry_params_c = entry_params
      .map(|value| CString::new(value).map_err(|e| format!("entry params: {e}")))
      .transpose()?;
    let entry_c = CString::new(entry).map_err(|e| format!("entry point: {e}"))?;

    let args = NativeChildProcessArgs {
      entry_params: entry_params_c
        .as_mut()
        .map(|value| value.as_ptr() as *mut c_char)
        .unwrap_or(ptr::null_mut()),
      fd_list: NativeChildProcessFdList { head: &mut fd_node },
    };
    let options = NativeChildProcessOptions { isolation_mode: 0, reserved: 0 };

    let mut pid: i32 = 0;
    let err = unsafe {
      OH_Ability_StartNativeChildProcess(entry_c.as_ptr(), args, options, &mut pid as *mut i32)
    };
    if err != 0 {
      return Err(format!("OH_Ability_StartNativeChildProcess({entry}) failed, err={err}"));
    }

    // The child owns its dup now; dropping our end means an agent crash shows up as EOF.
    drop(child_stream);

    let mut stream = parent_stream;
    stream
      .set_read_timeout(Some(READ_TIMEOUT))
      .map_err(|e| format!("set_read_timeout: {e}"))?;
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| format!("clone stream: {e}"))?);

    let ready = read_line(&mut reader).map_err(|e| format!("waiting for agent ready (pid={pid}): {e}"))?;
    if !ready.contains(AGENT_READY_MARKER) {
      return Err(format!("unexpected agent startup line (pid={pid}): {ready}"));
    }

    let mut payload = request.to_string();
    if !payload.ends_with('\n') {
      payload.push('\n');
    }
    stream.write_all(payload.as_bytes()).map_err(|e| format!("write request: {e}"))?;
    stream.flush().map_err(|e| format!("flush request: {e}"))?;

    let response = read_line(&mut reader).map_err(|e| format!("waiting for agent response (pid={pid}): {e}"))?;
    Ok((ready, response, pid))
  }

  fn read_line(reader: &mut BufReader<UnixStream>) -> Result<String, String> {
    let mut line = String::new();
    loop {
      line.clear();
      let read = reader.read_line(&mut line).map_err(|e| e.to_string())?;
      if read == 0 {
        return Err("agent closed the channel".to_string());
      }
      let trimmed = line.trim();
      if !trimmed.is_empty() {
        return Ok(trimmed.to_string());
      }
    }
  }
}

#[cfg(not(target_env = "ohos"))]
mod imp {
  use super::*;

  pub fn start_and_exchange(
    entry: &str,
    request: &str,
    entry_params: Option<&str>,
  ) -> Result<(String, String, i32), String> {
    let _ = (entry, request, entry_params);
    Err("native child processes are only supported on OHOS".to_string())
  }
}

/// Runs one JSON-RPC round trip against a bundled agent library and returns a
/// human readable summary (logged by ArkTS via hilog).
///
/// `entry_point` is the appspawn entry, e.g. `libdbx_agent_oracle.so:Main`.
#[cfg_attr(not(target_env = "ohos"), allow(unused))]
pub fn probe(entry_point: &str, request: &str) -> Result<String, String> {
  let (ready, response, pid) = imp::start_and_exchange(entry_point, request, None)?;
  Ok(format!("pid={pid} ready={ready} response={response}"))
}
