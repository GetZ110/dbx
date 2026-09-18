mod config;
mod driver;
mod model;
mod runtime;
mod value;

#[cfg(target_env = "ohos")]
mod ohos_ncp;

pub use runtime::run;

/// OHOS native child process 入口的驱动侧实现（入口本身见 `ohos_ncp.rs`）。
///
/// NCP 子进程的主线程由 appspawn 直接调用 `Main`，没有现成的 tokio 运行时，
/// 所以这里自建一个再 `block_on`。
#[cfg(target_env = "ohos")]
pub(crate) fn run_stdio_agent() {
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("TDengine driver runtime failed: {error:#}");
            return;
        }
    };
    if let Err(error) = runtime.block_on(run()) {
        eprintln!("TDengine driver failed: {error:#}");
    }
}
