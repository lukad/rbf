use super::Function;
use super::common::Helper;
use super::lower::{self, Emitter};
use crate::ast::Program;
use dynasm::dynasm;
use dynasmrt::{DynasmApi, DynasmLabelApi};

/// Compiles brainfuck code and returns a `Function`.
///
pub struct Jit {
    tape_size: usize,
    ops: dynasmrt::x64::Assembler,
    start: dynasmrt::AssemblyOffset,
    literals: Vec<Box<[u8]>>,
}

impl Jit {
    /// Initializes a `Jit` with a tape size of `30_000`
    pub fn new() -> Jit {
        let ops = dynasmrt::x64::Assembler::new().unwrap();

        Jit {
            tape_size: 30_000,
            start: ops.offset(),
            ops,
            literals: Vec::new(),
        }
    }

    /// Sets the tape size. Will be aligned to 16 bytes
    pub fn set_tape_size(mut self, tape_size: usize) -> Self {
        self.tape_size = tape_size.div_ceil(16) * 16;
        self
    }

    /// Generates machine code for the given program
    pub fn compile(mut self, program: &Program) -> Function {
        let frame_size = self.tape_size + 8;

        // Prologue
        dynasm!(self.ops
                ; .arch x64
                ; push rbp // Store frame pointer
                ; mov rbp, rsp // Address of current stack frame
                ; push rbx // Preserve callee-saved tape pointer register
                ; sub rsp, frame_size as _ // Reserve memory for tape and keep stack aligned
                ; lea rbx, [rsp] // Save memory address in rbx
        );

        // Zero tape
        let memzero = Helper::MemZero.address();
        dynasm!(self.ops
                ; .arch x64
                ; mov rax, QWORD memzero as _
                ; mov rdi, rbx
                ; mov rsi, self.tape_size as _
                ; call rax
        );

        lower::generate(&mut self, program);

        // Epilogue
        dynasm!(self.ops
                ; .arch x64
                ; add rsp, frame_size as _
                ; pop rbx // Restore callee-saved tape pointer register
                ; pop rbp // Restore frame pointer
                ; ret
        );

        let buf = self.ops.finalize().unwrap();
        Function::new(buf, self.start, self.literals)
    }

    fn move_tape(&mut self, offset: i64) {
        if offset == 0 {
            return;
        }

        if let Ok(offset) = i32::try_from(offset) {
            dynasm!(self.ops
                ; .arch x64
                ; add rbx, offset as _
            );
        } else {
            dynasm!(self.ops
                ; .arch x64
                ; mov rax, QWORD offset as _
                ; add rbx, rax
            );
        }
    }

    fn compute_offset(&mut self, offset: i64) {
        dynasm!(self.ops
            ; .arch x64
            ; mov r10, QWORD offset as _
            ; add r10, rbx
        );
    }

    fn add(&mut self, offset: i64, value: u8) {
        if value == 0 {
            return;
        }

        let value = value as i8;
        if let Some(offset) = direct_offset(offset) {
            dynasm!(self.ops
                ; .arch x64
                ; add BYTE [rbx + offset as _], value as _
            );
        } else {
            self.compute_offset(offset);
            dynasm!(self.ops
                ; .arch x64
                ; add BYTE [r10], value as _
            );
        }
    }

    fn set(&mut self, offset: i64, value: u8) {
        let value = value as i8;
        if let Some(offset) = direct_offset(offset) {
            dynasm!(self.ops
                ; .arch x64
                ; mov BYTE [rbx + offset as _], value as _
            );
        } else {
            self.compute_offset(offset);
            dynasm!(self.ops
                ; .arch x64
                ; mov BYTE [r10], value as _
            );
        }
    }

    fn write(&mut self, offset: i64) {
        if let Some(offset) = direct_offset(offset) {
            dynasm!(self.ops
                ; .arch x64
                ; movzx rdi, BYTE [rbx + offset as _]
            );
        } else {
            self.compute_offset(offset);
            dynasm!(self.ops
                ; .arch x64
                ; movzx rdi, BYTE [r10]
            );
        }

        let putchar = Helper::PutChar.address();
        dynasm!(self.ops
            ; .arch x64
            ; mov rax, QWORD putchar as _
            ; call rax
        );
    }

    fn read(&mut self, offset: i64) {
        let getchar = Helper::GetChar.address();
        dynasm!(self.ops
            ; .arch x64
            ; mov rax, QWORD getchar as _
            ; call rax
        );

        if let Some(offset) = direct_offset(offset) {
            dynasm!(self.ops
                ; .arch x64
                ; mov BYTE [rbx + offset as _], al
            );
        } else {
            self.compute_offset(offset);
            dynasm!(self.ops
                ; .arch x64
                ; mov BYTE [r10], al
            );
        }
    }

    fn write_byte(&mut self, byte: u8) {
        let putchar = Helper::PutChar.address();
        dynasm!(self.ops
            ; .arch x64
            ; mov rdi, byte as _
            ; mov rax, QWORD putchar as _
            ; call rax
        );
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        let (ptr, len) = self.retain_bytes(bytes);
        let putbytes = Helper::PutBytes.address();

        dynasm!(self.ops
            ; .arch x64
            ; mov rdi, QWORD ptr as _
            ; mov rsi, len as _
            ; mov rax, QWORD putbytes as _
            ; call rax
        );
    }

    fn load_source(&mut self, offset: i64) {
        if let Some(offset) = direct_offset(offset) {
            dynasm!(self.ops
                ; .arch x64
                ; movzx ecx, BYTE [rbx + offset as _]
            );
        } else {
            self.compute_offset(offset);
            dynasm!(self.ops
                ; .arch x64
                ; movzx ecx, BYTE [r10]
            );
        }
    }

    fn add_source(&mut self, offset: i64, subtract: bool) {
        if let Some(offset) = direct_offset(offset) {
            if subtract {
                dynasm!(self.ops
                    ; .arch x64
                    ; sub BYTE [rbx + offset as _], cl
                );
            } else {
                dynasm!(self.ops
                    ; .arch x64
                    ; add BYTE [rbx + offset as _], cl
                );
            }
        } else {
            self.compute_offset(offset);
            if subtract {
                dynasm!(self.ops
                    ; .arch x64
                    ; sub BYTE [r10], cl
                );
            } else {
                dynasm!(self.ops
                    ; .arch x64
                    ; add BYTE [r10], cl
                );
            }
        }
    }

    fn add_product(&mut self, offset: i64) {
        if let Some(offset) = direct_offset(offset) {
            dynasm!(self.ops
                ; .arch x64
                ; add BYTE [rbx + offset as _], al
            );
        } else {
            self.compute_offset(offset);
            dynasm!(self.ops
                ; .arch x64
                ; add BYTE [r10], al
            );
        }
    }

    fn apply_mul(&mut self, dest: i64, factor: i64) {
        match factor as u8 {
            0 => (),
            1 => self.add_source(dest, false),
            u8::MAX => self.add_source(dest, true),
            factor => {
                dynasm!(self.ops
                    ; .arch x64
                    ; imul eax, ecx, factor as _
                );
                self.add_product(dest);
            }
        }
    }

    fn mul(&mut self, source: i64, dest: i64, factor: i64) {
        self.load_source(source);
        self.apply_mul(dest, factor);
    }

    fn mul_run(&mut self, base: i64, muls: &[(i64, i64)]) {
        self.load_source(base);

        for &(offset, factor) in muls {
            self.apply_mul(base + offset, factor);
        }

        self.set(base, 0);
    }

    fn scan(&mut self, step: i64) {
        let move_label = self.ops.new_dynamic_label();
        let rest_label = self.ops.new_dynamic_label();

        dynasm!(self.ops
            ; .arch x64
            ; cmp BYTE [rbx], 0
            ; je =>rest_label
            ; =>move_label
        );

        self.move_tape(step);

        dynasm!(self.ops
            ; .arch x64
            ; cmp BYTE [rbx], 0
            ; jne =>move_label
            ; =>rest_label
        );
    }

    fn r#loop(&mut self, body: &Program) {
        let body_label = self.ops.new_dynamic_label();
        let rest_label = self.ops.new_dynamic_label();

        dynasm!(self.ops
            ; .arch x64
            ; cmp BYTE [rbx], 0
            ; je =>rest_label
            ; =>body_label
        );

        lower::generate_loop(self, body);

        dynasm!(self.ops
            ; .arch x64
            ; cmp BYTE [rbx], 0
            ; jne =>body_label
            ; =>rest_label
        );
    }

    fn retain_bytes(&mut self, bytes: &[u8]) -> (*const u8, usize) {
        let bytes = bytes.to_vec().into_boxed_slice();
        let ptr = bytes.as_ptr();
        let len = bytes.len();
        self.literals.push(bytes);
        (ptr, len)
    }
}

impl Emitter for Jit {
    fn move_tape(&mut self, offset: i64) {
        Jit::move_tape(self, offset);
    }

    fn add(&mut self, offset: i64, value: u8) {
        Jit::add(self, offset, value);
    }

    fn set(&mut self, offset: i64, value: u8) {
        Jit::set(self, offset, value);
    }

    fn mul(&mut self, source: i64, dest: i64, factor: i64) {
        Jit::mul(self, source, dest, factor);
    }

    fn mul_run(&mut self, base: i64, muls: &[(i64, i64)]) {
        Jit::mul_run(self, base, muls);
    }

    fn write(&mut self, offset: i64) {
        Jit::write(self, offset);
    }

    fn read(&mut self, offset: i64) {
        Jit::read(self, offset);
    }

    fn write_byte(&mut self, byte: u8) {
        Jit::write_byte(self, byte);
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        Jit::write_bytes(self, bytes);
    }

    fn scan(&mut self, step: i64) {
        Jit::scan(self, step);
    }

    fn r#loop(&mut self, body: &Program) {
        Jit::r#loop(self, body);
    }
}

impl Default for Jit {
    fn default() -> Self {
        Self::new()
    }
}

fn direct_offset(offset: i64) -> Option<i32> {
    i32::try_from(offset).ok()
}
