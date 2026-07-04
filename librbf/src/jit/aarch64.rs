use super::Function;
use super::common::{getchar, memzero, putchar};
use crate::ast::{Instruction::*, Program};
use crate::jit::common::putbytes;
use dynasm::dynasm;
use dynasmrt::{DynasmApi, DynasmLabelApi};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reg {
    Arg0 = 0,
    Arg1 = 1,
    Scratch0 = 9,
    Scratch1 = 10,
    Scratch2 = 11,
    MulSource = 12,
    HelperTarget = 16,
    TapePtr = 19,
    PutCharTarget = 20,
    PutBytesTarget = 21,
    GetCharTarget = 22,
    FramePtr = 29,
    Link = 30,
    StackPtr = 31,
}

impl From<Reg> for u8 {
    fn from(reg: Reg) -> Self {
        reg as u8
    }
}

/// Compiles brainfuck code and returns a `Function`.
///
/// The AArch64 backend follows AAPCS64. The tape pointer lives in x19, which is
/// callee-saved, so calls to Rust helper functions can use x0-x18 freely.
pub struct Jit {
    tape_size: usize,
    ops: dynasmrt::aarch64::Assembler,
    start: dynasmrt::AssemblyOffset,
}

impl Jit {
    /// Initializes a `Jit` with a tape size of `30_000`.
    pub fn new() -> Jit {
        let ops = dynasmrt::aarch64::Assembler::new().unwrap();

        Jit {
            tape_size: 30_000,
            start: ops.offset(),
            ops,
        }
    }

    /// Sets the tape size. Will be aligned to 16 bytes.
    pub fn set_tape_size(mut self, tape_size: usize) -> Self {
        self.tape_size = tape_size.div_ceil(16) * 16;
        self
    }

    /// Generates machine code for the given program.
    pub fn compile(mut self, program: &Program) -> Function {
        dynasm!(self.ops
                ; .arch aarch64
                ; stp X(Reg::FramePtr), X(Reg::Link), [XSP(Reg::StackPtr), #-16]!
                ; mov XSP(Reg::FramePtr), XSP(Reg::StackPtr)
                ; stp X(Reg::TapePtr), X(Reg::PutCharTarget), [XSP(Reg::StackPtr), #-16]!
                ; stp X(Reg::PutBytesTarget), X(Reg::GetCharTarget), [XSP(Reg::StackPtr), #-16]!
        );

        self.load_x(Reg::Scratch0, self.tape_size as u64);
        dynasm!(self.ops
                ; .arch aarch64
                ; sub XSP(Reg::StackPtr), XSP(Reg::StackPtr), X(Reg::Scratch0)
                ; mov XSP(Reg::TapePtr), XSP(Reg::StackPtr)
        );

        self.load_x(Reg::Arg1, self.tape_size as u64);
        self.load_x(Reg::HelperTarget, memzero as *const () as u64);
        dynasm!(self.ops
                ; .arch aarch64
                ; mov X(Reg::Arg0), X(Reg::TapePtr)
                ; blr X(Reg::HelperTarget)
        );

        self.load_x(Reg::PutCharTarget, putchar as *const () as u64);
        self.load_x(Reg::PutBytesTarget, putbytes as *const () as u64);
        self.load_x(Reg::GetCharTarget, getchar as *const () as u64);

        self.generate(program);

        self.load_x(Reg::Scratch0, self.tape_size as u64);
        dynasm!(self.ops
                ; .arch aarch64
                ; add XSP(Reg::StackPtr), XSP(Reg::StackPtr), X(Reg::Scratch0)
        );

        dynasm!(self.ops
                ; .arch aarch64
                ; ldp X(Reg::PutBytesTarget), X(Reg::GetCharTarget), [XSP(Reg::StackPtr)], #16
                ; ldp X(Reg::TapePtr), X(Reg::PutCharTarget), [XSP(Reg::StackPtr)], #16
                ; ldp X(Reg::FramePtr), X(Reg::Link), [XSP(Reg::StackPtr)], #16
                ; ret
        );

        let buf = self.ops.finalize().unwrap();
        Function::new(buf, self.start)
    }

    fn generate(&mut self, program: &Program) {
        for ins in program.iter() {
            match ins {
                &Move(i) => self.move_tape(i),
                &Add(i) => {
                    self.load_x(Reg::Scratch1, i as u8 as u64);
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; ldrb W(Reg::Scratch0), [X(Reg::TapePtr)]
                            ; add W(Reg::Scratch0), W(Reg::Scratch0), W(Reg::Scratch1)
                            ; strb W(Reg::Scratch0), [X(Reg::TapePtr)]
                    );
                }
                Write => {
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; ldrb W(Reg::Arg0), [X(Reg::TapePtr)]
                            ; blr X(Reg::PutCharTarget)
                    );
                }
                WriteConst(i) => {
                    self.load_x(Reg::Arg0, (i % 0xFF) as u8 as u64);
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; strb W(Reg::Arg0), [X(Reg::TapePtr)]
                            ; blr X(Reg::PutCharTarget)
                    );
                }
                WriteBytes(bytes) => {
                    let last = *bytes.last().unwrap();
                    self.load_x(Reg::Scratch0, last as u64);
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; strb W(Reg::Scratch0), [X(Reg::TapePtr)]
                    );

                    self.load_x(Reg::Arg0, bytes.as_ptr() as u64);
                    self.load_x(Reg::Arg1, bytes.len() as u64);

                    dynasm!(self.ops
                            ; .arch aarch64
                            ; blr X(Reg::PutBytesTarget)
                    );
                }
                Read => {
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; blr X(Reg::GetCharTarget)
                            ; strb W(Reg::Arg0), [X(Reg::TapePtr)]
                    );
                }
                Set(0) => {
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; strb wzr, [X(Reg::TapePtr)]
                    );
                }
                Set(i) => {
                    self.load_x(Reg::Scratch0, (i % 0xFF) as u8 as u64);
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; strb W(Reg::Scratch0), [X(Reg::TapePtr)]
                    );
                }
                &Mul(offset, factor) => {
                    self.generate_mul(offset, factor);
                }
                MulRun(muls) => {
                    self.generate_mul_run(muls);
                }
                &Scan(i) => {
                    let move_label = self.ops.new_dynamic_label();
                    let rest_label = self.ops.new_dynamic_label();
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; ldrb W(Reg::Scratch0), [X(Reg::TapePtr)]
                            ; cbz W(Reg::Scratch0), =>rest_label
                            ; =>move_label
                    );

                    self.move_tape(i);

                    dynasm!(self.ops
                            ; .arch aarch64
                            ; ldrb W(Reg::Scratch0), [X(Reg::TapePtr)]
                            ; cbnz W(Reg::Scratch0), =>move_label
                            ; =>rest_label
                    );
                }
                Loop(body) => {
                    let body_label = self.ops.new_dynamic_label();
                    let rest_label = self.ops.new_dynamic_label();
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; ldrb W(Reg::Scratch0), [X(Reg::TapePtr)]
                            ; cbz W(Reg::Scratch0), =>rest_label
                            ; =>body_label
                    );

                    self.generate(body);

                    dynasm!(self.ops
                            ; .arch aarch64
                            ; ldrb W(Reg::Scratch0), [X(Reg::TapePtr)]
                            ; cbnz W(Reg::Scratch0), =>body_label
                            ; =>rest_label
                    );
                }
            }
        }
    }

    /// Generates code for `Instruction::Mul`.
    ///
    /// The general case is roughly `tape[ptr + offset] += tape[ptr] * factor`.
    /// When `factor` is 1 or -1 this is effectively just an addition or subtraction
    /// and a specialized path without multiplication is taken.
    fn generate_mul(&mut self, offset: i64, factor: i64) {
        let addr = Reg::Scratch2;
        self.compute_offset(addr, offset);

        match factor {
            1 => dynasm!(self.ops
                ; .arch aarch64
                ; ldrb W(Reg::Scratch0), [X(Reg::TapePtr)]
                ; ldrb W(Reg::Scratch1), [X(addr)]
                ; add W(Reg::Scratch1), W(Reg::Scratch1), W(Reg::Scratch0)
                ; strb W(Reg::Scratch1), [X(addr)]
            ),
            -1 => dynasm!(self.ops
                ; .arch aarch64
                ; ldrb W(Reg::Scratch0), [X(Reg::TapePtr)]
                ; ldrb W(Reg::Scratch1), [X(addr)]
                ; sub W(Reg::Scratch1), W(Reg::Scratch1), W(Reg::Scratch0)
                ; strb W(Reg::Scratch1), [X(addr)]
            ),
            _ => {
                self.load_x(Reg::Scratch1, factor as u8 as u64);
                dynasm!(self.ops
                    ; .arch aarch64
                    ; ldrb W(Reg::Scratch0), [X(Reg::TapePtr)]
                    ; mul W(Reg::Scratch0), W(Reg::Scratch0), W(Reg::Scratch1)
                    ; ldrb W(Reg::Scratch1), [X(addr)]
                    ; add W(Reg::Scratch1), W(Reg::Scratch1), W(Reg::Scratch0)
                    ; strb W(Reg::Scratch1), [X(addr)]
                );
            }
        }
    }

    /// Generates code for `Instruction::MulRun`.
    ///
    /// Optimized multiplication loops read from one source cell, apply one or more
    /// transfers, then clear the source. This loads the source once and reuses it
    /// for every destination update.
    fn generate_mul_run(&mut self, muls: &[(i64, i64)]) {
        // Keep the original source cell live across all destination updates.
        dynasm!(self.ops
            ; .arch aarch64
            ; ldrb W(Reg::MulSource), [X(Reg::TapePtr)]
        );

        for &(offset, factor) in muls {
            let addr = Reg::Scratch2;
            self.compute_offset(addr, offset);

            match factor {
                1 => dynasm!(self.ops
                    ; .arch aarch64
                    ; ldrb W(Reg::Scratch1), [X(addr)]
                    ; add W(Reg::Scratch1), W(Reg::Scratch1), W(Reg::MulSource)
                    ; strb W(Reg::Scratch1), [X(addr)]
                ),
                -1 => dynasm!(self.ops
                    ; .arch aarch64
                    ; ldrb W(Reg::Scratch1), [X(addr)]
                    ; sub W(Reg::Scratch1), W(Reg::Scratch1), W(Reg::MulSource)
                    ; strb W(Reg::Scratch1), [X(addr)]
                ),
                _ => {
                    self.load_x(Reg::Scratch1, factor as u8 as u64);
                    dynasm!(self.ops
                        ; .arch aarch64
                        ; mul W(Reg::Scratch0), W(Reg::MulSource), W(Reg::Scratch1)
                        ; ldrb W(Reg::Scratch1), [X(addr)]
                        ; add W(Reg::Scratch1), W(Reg::Scratch1), W(Reg::Scratch0)
                        ; strb W(Reg::Scratch1), [X(addr)]
                    );
                }
            }
        }

        // Finally clear the original source cell
        dynasm!(self.ops
            ; .arch aarch64
            ; strb wzr, [X(Reg::TapePtr)]
        );
    }

    fn move_tape(&mut self, offset: i64) {
        if offset == 0 {
            return;
        }

        let amount = offset.unsigned_abs();

        // Small moves can use the AArch64 immediate add/sub form directly.
        if amount < 4096 {
            let amount = amount as u32;

            if offset > 0 {
                dynasm!(self.ops
                    ; .arch aarch64
                    ; add XSP(Reg::TapePtr), XSP(Reg::TapePtr), #amount
                );
            } else {
                dynasm!(self.ops
                    ; .arch aarch64
                    ; sub XSP(Reg::TapePtr), XSP(Reg::TapePtr), #amount
                );
            }
            return;
        }

        self.load_x(Reg::Scratch0, amount);

        if offset > 0 {
            dynasm!(self.ops
                    ; .arch aarch64
                    ; add X(Reg::TapePtr), X(Reg::TapePtr), X(Reg::Scratch0)
            );
        } else {
            dynasm!(self.ops
                    ; .arch aarch64
                    ; sub X(Reg::TapePtr), X(Reg::TapePtr), X(Reg::Scratch0)
            );
        }
    }

    fn compute_offset(&mut self, addr: Reg, offset: i64) {
        debug_assert!(addr != Reg::Scratch0);

        if offset == 0 {
            dynasm!(self.ops
                    ; .arch aarch64
                    ; mov X(addr), X(Reg::TapePtr)
            );
            return;
        }

        let amount = offset.unsigned_abs();

        if amount < 4096 {
            let amount = amount as u32;

            if offset > 0 {
                dynasm!(self.ops
                        ; .arch aarch64
                        ; add XSP(addr), XSP(Reg::TapePtr), #amount
                );
            } else {
                dynasm!(self.ops
                        ; .arch aarch64
                        ; sub XSP(addr), XSP(Reg::TapePtr), #amount
                );
            }
            return;
        }

        dynasm!(self.ops
                ; .arch aarch64
                ; mov X(addr), X(Reg::TapePtr)
        );

        self.load_x(Reg::Scratch0, amount);
        if offset > 0 {
            dynasm!(self.ops
                    ; .arch aarch64
                    ; add X(addr), X(addr), X(Reg::Scratch0)
            );
        } else {
            dynasm!(self.ops
                    ; .arch aarch64
                    ; sub X(addr), X(addr), X(Reg::Scratch0)
            );
        }
    }

    fn load_x(&mut self, reg: Reg, value: u64) {
        let p0 = (value & 0xFFFF) as u32;
        let p1 = ((value >> 16) & 0xFFFF) as u32;
        let p2 = ((value >> 32) & 0xFFFF) as u32;
        let p3 = ((value >> 48) & 0xFFFF) as u32;

        dynasm!(self.ops
            ; .arch aarch64
            ; movz X(reg), #p0
        );

        if p1 != 0 {
            dynasm!(self.ops
                ; .arch aarch64
                ; movk X(reg), #p1, lsl #16
            );
        }
        if p2 != 0 {
            dynasm!(self.ops
                ; .arch aarch64
                ; movk X(reg), #p2, lsl #32
            );
        }
        if p3 != 0 {
            dynasm!(self.ops
                ; .arch aarch64
                ; movk X(reg), #p3, lsl #48
            );
        }
    }
}

impl Default for Jit {
    fn default() -> Self {
        Self::new()
    }
}
