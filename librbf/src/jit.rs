mod common;

#[cfg(all(target_arch = "aarch64", any(target_os = "linux", target_os = "macos")))]
mod aarch64;
#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(all(target_arch = "aarch64", any(target_os = "linux", target_os = "macos")))]
pub use aarch64::Jit;
pub use common::Function;
#[cfg(target_arch = "x86_64")]
pub use x86_64::Jit;

#[cfg(not(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", any(target_os = "linux", target_os = "macos"))
)))]
compile_error!("rbf JIT supports only x86_64 and Unix AArch64 targets");
