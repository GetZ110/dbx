use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use napi_derive_ohos::napi;
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

static SERVER_PORT: AtomicU16 = AtomicU16::new(0);
static SERVER_STATE: Mutex<Option<ServerHandle>> = Mutex::new(None);

struct ServerHandle {
  shutdown: CancellationToken,
  thread: Option<thread::JoinHandle<()>>,
}

#[napi(object)]
pub struct ServerOptions {
  pub port: u16,
  pub static_dir: String,
  pub data_dir: String,
  pub disable_password: bool,
}

/// Starts the DBX web backend (dbx-web) inside the current process.
/// Returns the bound port on success.
#[napi]
pub fn start_server(options: ServerOptions) -> napi_ohos::Result<u16> {
  if SERVER_PORT.load(Ordering::SeqCst) != 0 {
    return Ok(SERVER_PORT.load(Ordering::SeqCst));
  }

  let port = options.port;
  let static_dir = options.static_dir;
  let data_dir = options.data_dir;
  let disable_password = options.disable_password;

  std::env::set_var("DBX_PORT", port.to_string());
  std::env::set_var("DBX_STATIC_DIR", static_dir);
  std::env::set_var("DBX_DATA_DIR", data_dir);
  if disable_password {
    std::env::set_var("DBX_DISABLE_PASSWORD", "1");
  } else {
    std::env::remove_var("DBX_DISABLE_PASSWORD");
  }

  let runtime = Arc::new(Runtime::new().map_err(|e| napi_ohos::Error::from_reason(e.to_string()))?);
  let shutdown = CancellationToken::new();
  let server_shutdown = shutdown.clone();
  let rt = runtime.clone();
  let handle = thread::Builder::new()
    .name("dbx-web-server".to_string())
    .spawn(move || {
      rt.block_on(async {
        dbx_web::run_server_with_shutdown(server_shutdown).await;
      });
    })
    .map_err(|e| napi_ohos::Error::from_reason(e.to_string()))?;

  let mut state = SERVER_STATE.lock().map_err(|e| napi_ohos::Error::from_reason(e.to_string()))?;
  *state = Some(ServerHandle {
    shutdown,
    thread: Some(handle),
  });
  drop(state);

  SERVER_PORT.store(port, Ordering::SeqCst);
  Ok(port)
}

/// Stops the DBX web backend gracefully.
#[napi]
pub fn stop_server() -> napi_ohos::Result<()> {
  let mut state = SERVER_STATE.lock().map_err(|e| napi_ohos::Error::from_reason(e.to_string()))?;
  if let Some(handle) = state.take() {
    handle.shutdown.cancel();
    if let Some(thread) = handle.thread {
      let _ = thread.join();
    }
  }
  SERVER_PORT.store(0, Ordering::SeqCst);
  Ok(())
}

/// Returns the port of the native MCP server.
///
/// The MCP server is served by dbx-web at `http://127.0.0.1:{port}/mcp`.
/// This function returns the web server port when it is running.
#[napi]
pub fn start_mcp_server(port: u16) -> napi_ohos::Result<u16> {
  let web_port = SERVER_PORT.load(Ordering::SeqCst);
  if web_port == 0 {
    return Err(napi_ohos::Error::from_reason(
      "dbx-web native server is not running; startServer must be called first.",
    ));
  }
  let _ = port;
  Ok(web_port)
}

/// Stops the native MCP server (same lifecycle as the web server).
#[napi]
pub fn stop_mcp_server() -> napi_ohos::Result<()> {
  stop_server()
}
