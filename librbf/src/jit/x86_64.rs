use super::Function;
use super::common::{getchar, memzero, putbytes, putchar};
use crate::ast::{Instruction::*, Program};
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
        dynasm!(self.ops
                ; .arch x64
                ; mov rax, QWORD memzero as *const () as _
                ; mov rdi, rbx
                ; mov rsi, self.tape_size as _
                ; call rax
        );

        self.generate(program);

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

    fn generate(&mut self, program: &Program) {
        for ins in program.iter() {
            match ins {
                &Move(i) => {
                    dynasm!(self.ops
                            ; .arch x64
                            ; add rbx, i as _
                    );
                }
                &Add(i) => {
                    dynasm!(self.ops
                            ; .arch x64
                            ; add BYTE [rbx], i as _
                    );
                }
                Write => {
                    dynasm!(self.ops
                            ; .arch x64
                            ; movzx rdi, [rbx]
                            ; mov rax, QWORD putchar as *const () as _
                            ; call rax
                    );
                }
                Read => {
                    dynasm!(self.ops
                            ; .arch x64
                            ; mov rax, QWORD getchar as *const () as _
                            ; call rax
                            ; mov [rbx], al
                    );
                }
                &WriteConst(i) => {
                    let value = i as u8;

                    dynasm!(self.ops
                            ; .arch x64
                            ; mov BYTE [rbx], value as _
                            ; mov rdi, value as _
                            ; mov rax, QWORD putchar as *const () as _
                            ; call rax
                    );
                }
                WriteBytes(bytes) => {
                    let last = *bytes.last().unwrap();
                    let (ptr, len) = self.retain_bytes(bytes);

                    dynasm!(self.ops
                            ; .arch x64
                            ; mov BYTE [rbx], last as _
                            ; mov rdi, QWORD ptr as _
                            ; mov rsi, len as _
                            ; mov rax, QWORD putbytes as *const () as _
                            ; call rax
                    );
                }
                Set(i) => {
                    dynasm!(self.ops
                            ; .arch x64
                            ; mov BYTE [rbx], (i % 0xFF) as _
                    );
                }
                &Mul(offset, mul) => {
                    dynasm!(self.ops
                            ; .arch x64
                            ; mov al, mul as _
                            ; mul BYTE [rbx]
                            ; add [rbx + offset as _], al
                    );
                }
                MulRun(muls) => {
                    for &(offset, mul) in muls {
                        dynasm!(self.ops
                                ; .arch x64
                                ; mov al, mul as _
                                ; mul BYTE [rbx]
                                ; add [rbx + offset as _], al
                        );
                    }

                    dynasm!(self.ops
                            ; .arch x64
                            ; mov BYTE [rbx], 0
                    );
                }
                &Scan(i) => {
                    let move_label = self.ops.new_dynamic_label();
                    let rest_label = self.ops.new_dynamic_label();
                    dynasm!(self.ops
                            ; .arch x64
                            ; cmp BYTE [rbx], 0
                            ; je =>rest_label
                            ; =>move_label
                            ; add rbx, i as _
                            ; cmp BYTE [rbx], 0
                            ; jne =>move_label
                            ; =>rest_label
                    );
                }
                Loop(body) => {
                    let body_label = self.ops.new_dynamic_label();
                    let rest_label = self.ops.new_dynamic_label();
                    dynasm!(self.ops
                            ; .arch x64
                            ; cmp BYTE [rbx], 0
                            ; je =>rest_label
                            ; =>body_label
                    );

                    self.generate(body);

                    dynasm!(self.ops
                            ; .arch x64
                            ; cmp BYTE [rbx], 0
                            ; jne =>body_label
                            ; =>rest_label
                    );
                }
            }
        }
    }

    fn retain_bytes(&mut self, bytes: &[u8]) -> (*const u8, usize) {
        let bytes = bytes.to_vec().into_boxed_slice();
        let ptr = bytes.as_ptr();
        let len = bytes.len();
        self.literals.push(bytes);
        (ptr, len)
    }
}

impl Default for Jit {
    fn default() -> Self {
        Self::new()
    }
}
