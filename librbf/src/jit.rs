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

#[cfg(test)]
mod tests {
    use super::Jit;
    use crate::Instruction::WriteBytes;

    #[test]
    fn compiled_function_owns_bulk_write_literals() {
        let program = vec![WriteBytes(vec![b'A', b'B'])];
        let function = Jit::new().compile(&program);

        drop(program);

        assert_eq!(function.literal_count(), 1);
    }
}
