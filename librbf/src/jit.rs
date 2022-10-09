use std::io::Read;
use std::io::{self, Write};
use std::mem;

use dynasm::dynasm;
use dynasmrt::{DynasmApi, DynasmLabelApi, ExecutableBuffer};

use crate::ast::{Instruction::*, Program};

extern "C" fn putchar(c: u8) {
    print!("{}", c as char);
}

extern "C" fn getchar() -> u8 {
    io::stdout().flush().unwrap();
    match std::io::stdin().bytes().next() {
        Some(Ok(c)) => c as _,
        _ => 0,
    }
}

extern "C" fn memzero(dst: *mut u8, count: usize) {
    unsafe { std::ptr::write_bytes(dst, 0, count) }
}

#[derive(Debug)]
pub struct Function {
    _buf: ExecutableBuffer,
    fun: fn() -> (),
}

impl Function {
    fn new(jit: Jit) -> Self {
        let buf = jit.ops.finalize().unwrap();
        let fun: fn() -> () = unsafe { mem::transmute(buf.ptr(jit.start)) };

        Self {
            _buf: buf,
            fun: fun,
        }
    }

    pub fn run(&self) {
        (self.fun)();
    }
}

/// Compiles code and creates a `Function`
pub struct Jit {
    tape_size: usize,
    ops: dynasmrt::x64::Assembler,
    start: dynasmrt::AssemblyOffset,
}

impl Jit {
    /// Initializes a `Jit` with a tape size of `30_000`
    pub fn new() -> Jit {
        let ops = dynasmrt::x64::Assembler::new().unwrap();

        Jit {
            tape_size: 30_000,
            start: ops.offset(),
            ops: ops,
        }
    }

    /// Sets the tape size. Will be aligned to 16 bytes
    pub fn set_tape_size(mut self, tape_size: usize) -> Self {
        self.tape_size = ((tape_size + 16 - 1) / 16) * 16;
        self
    }

    /// Generates machine code for the given program
    pub fn compile(mut self, program: &Program) -> Function {
        // Prologue
        dynasm!(self.ops
                ; push rbp // Store frame pointer
                ; mov rbp, rsp // Address of current stack frame
                ; sub rsp, self.tape_size as _ // Reserve memory for tape on the stack
                ; lea rbx, [rsp] // Save memory address in rbx
        );

        // Zero tape
        dynasm!(self.ops
                ; mov rax, QWORD memzero as _
                ; mov rdi, rbx
                ; mov rsi, self.tape_size as _
                ; call rax
        );

        self.gen(program);

        // Epilogue
        dynasm!(self.ops
                ; leave // Restore frame pointer
                ; ret
        );

        Function::new(self)
    }

    fn gen(&mut self, program: &Program) {
        for ins in program.iter() {
            match ins {
                &(Move(i), _) => {
                    dynasm!(self.ops
                            ; add rbx, i as _
                    );
                }
                &(Add(i), _) => {
                    dynasm!(self.ops
                            ; add BYTE [rbx], i as _
                    );
                }
                (Write, _) => {
                    dynasm!(self.ops
                            ; movzx rdi, [rbx]
                            ; mov rax, QWORD putchar as _
                            ; call rax
                    );
                }
                (Read, _) => {
                    dynasm!(self.ops
                            ; mov rax, QWORD getchar as _
                            ; call rax
                            ; mov [rbx], al
                    );
                }
                (Set(i), _) => {
                    dynasm!(self.ops
                            ; mov BYTE [rbx], (i % 0xFF) as _
                    );
                }
                &(Mul(offset, mul), _) => {
                    dynasm!(self.ops
                            ; mov al, mul as _
                            ; mul BYTE [rbx]
                            ; add [rbx + offset as _], al
                    );
                }
                &(Scan(i), _) => {
                    let move_label = self.ops.new_dynamic_label();
                    let rest_label = self.ops.new_dynamic_label();
                    dynasm!(self.ops
                            ; cmp BYTE [rbx], 0
                            ; je =>rest_label
                            ; =>move_label
                            ; add rbx, i as _
                            ; cmp BYTE [rbx], 0
                            ; jne =>move_label
                            ; =>rest_label
                    );
                }
                (Loop(body), _) => {
                    let body_label = self.ops.new_dynamic_label();
                    let rest_label = self.ops.new_dynamic_label();
                    dynasm!(self.ops
                            ; cmp BYTE [rbx], 0
                            ; je =>rest_label
                            ; =>body_label
                    );

                    self.gen(body);

                    dynasm!(self.ops
                            ; cmp BYTE [rbx], 0
                            ; jne =>body_label
                            ; =>rest_label
                    );
                }
            }
        }
    }
}
