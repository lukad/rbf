mod common;
mod lower;

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
    use crate::Instruction::{Loop, Move, Read, Set, WriteBytes};

    #[test]
    fn compiled_function_owns_bulk_write_literals() {
        let program = vec![WriteBytes(vec![b'A', b'B'])];
        let function = Jit::new().compile(&program);

        drop(program);

        assert_eq!(function.literal_count(), 1);
    }

    #[test]
    fn codegen_folds_canceling_pointer_moves() {
        let empty = Jit::new().compile(&vec![]);
        let moves = Jit::new().compile(&vec![Move(1), Move(-1)]);

        assert_eq!(moves.code_size(), empty.code_size());
    }

    #[test]
    fn codegen_folds_redundant_sets_inside_loops() {
        let one_set = Jit::new().compile(&vec![Read, Loop(vec![Set(1)])]);
        let repeated_set = Jit::new().compile(&vec![Read, Loop(vec![Set(1), Set(1)])]);

        assert_eq!(repeated_set.code_size(), one_set.code_size());
    }
}
