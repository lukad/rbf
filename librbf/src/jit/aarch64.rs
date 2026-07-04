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
        let mut offset = 0;

        for ins in program.iter() {
            match ins {
                &Move(i) => offset += i,
                &Add(n) => self.add(offset, n),
                &Set(n) => self.set(offset, n),
                &Mul(o, f) => self.mul(offset, offset + o, f),
                MulRun(muls) => self.mul_run(offset, muls),
                Write => self.write(offset),
                Read => self.read(offset),
                &WriteConst(n) => self.write_const(offset, n),
                WriteBytes(bytes) => self.write_bytes(offset, bytes),
                Scan(n) => {
                    self.flush_offset(&mut offset);
                    self.scan(*n);
                }
                Loop(body) => {
                    self.flush_offset(&mut offset);
                    self.r#loop(body);
                }
            }
        }

        self.flush_offset(&mut offset);
    }

    /// Flushes the offset to the tape pointer and resets it to 0.
    fn flush_offset(&mut self, offset: &mut i64) {
        self.move_tape(*offset);
        *offset = 0;
    }

    fn load_cell(&mut self, dst: Reg, scratch: Reg, offset: i64) {
        debug_assert!(dst != scratch);

        if let Some(offset) = direct_byte_offset(offset) {
            dynasm!(self.ops
                ; .arch aarch64
                ; ldrb W(dst), [X(Reg::TapePtr), #offset]
            );
            return;
        }

        self.compute_offset(scratch, offset);
        dynasm!(self.ops
            ; .arch aarch64
            ; ldrb W(dst), [X(scratch)]
        );
    }

    fn store_cell(&mut self, src: Reg, scratch: Reg, offset: i64) {
        debug_assert!(src != scratch);

        if let Some(offset) = direct_byte_offset(offset) {
            dynasm!(self.ops
                ; .arch aarch64
                ; strb W(src), [X(Reg::TapePtr), #offset]
            );
            return;
        }

        self.compute_offset(scratch, offset);
        dynasm!(self.ops
            ; .arch aarch64
            ; strb W(src), [X(scratch)]
        );
    }

    fn zero_cell(&mut self, scratch: Reg, offset: i64) {
        if let Some(offset) = direct_byte_offset(offset) {
            dynasm!(self.ops
                ; .arch aarch64
                ; strb wzr, [X(Reg::TapePtr), #offset]
            );
            return;
        }

        self.compute_offset(scratch, offset);
        dynasm!(self.ops
            ; .arch aarch64
            ; strb wzr, [X(scratch)]
        );
    }

    fn add(&mut self, offset: i64, n: i64) {
        self.load_x(Reg::Scratch1, n as u8 as u64);
        self.load_cell(Reg::Scratch0, Reg::Scratch2, offset);
        dynasm!(self.ops
            ; .arch aarch64
            ; add W(Reg::Scratch0), W(Reg::Scratch0), W(Reg::Scratch1)
        );
        self.store_cell(Reg::Scratch0, Reg::Scratch2, offset);
    }

    fn set(&mut self, offset: i64, n: i64) {
        if n == 0 {
            self.zero_cell(Reg::Scratch2, offset);
            return;
        }

        self.load_x(Reg::Scratch0, n as u8 as u64);
        self.store_cell(Reg::Scratch0, Reg::Scratch2, offset);
    }

    fn write(&mut self, offset: i64) {
        self.load_cell(Reg::Arg0, Reg::Scratch2, offset);
        dynasm!(self.ops
            ; .arch aarch64
            ; blr X(Reg::PutCharTarget)
        );
    }

    fn read(&mut self, offset: i64) {
        dynasm!(self.ops
            ; .arch aarch64
            ; blr X(Reg::GetCharTarget)
        );
        self.store_cell(Reg::Arg0, Reg::Scratch2, offset);
    }

    fn write_const(&mut self, offset: i64, n: i64) {
        self.load_x(Reg::Arg0, n as u8 as u64);
        self.store_cell(Reg::Arg0, Reg::Scratch2, offset);

        dynasm!(self.ops
            ; .arch aarch64
            ; blr X(Reg::PutCharTarget)
        );
    }

    fn write_bytes(&mut self, offset: i64, bytes: &[u8]) {
        let last = *bytes.last().unwrap();

        self.load_x(Reg::Scratch0, last as u64);
        self.store_cell(Reg::Scratch0, Reg::Scratch2, offset);

        self.load_x(Reg::Arg0, bytes.as_ptr() as u64);
        self.load_x(Reg::Arg1, bytes.len() as u64);

        dynasm!(self.ops
            ; .arch aarch64
            ; blr X(Reg::PutBytesTarget)
        );
    }

    /// Generates code for `Instruction::Mul`.
    ///
    /// The general case is roughly `tape[ptr + offset] += tape[ptr] * factor`.
    /// When `factor` is 1 or -1 this is effectively just an addition or subtraction
    /// and a specialized path without multiplication is taken.
    fn mul(&mut self, source: i64, dest: i64, factor: i64) {
        self.load_cell(Reg::MulSource, Reg::Scratch2, source);

        match factor {
            1 => {
                self.load_cell(Reg::Scratch1, Reg::Scratch2, dest);
                dynasm!(self.ops
                    ; .arch aarch64
                    ; add W(Reg::Scratch1), W(Reg::Scratch1), W(Reg::MulSource)
                );
                self.store_cell(Reg::Scratch1, Reg::Scratch2, dest);
            }
            -1 => {
                self.load_cell(Reg::Scratch1, Reg::Scratch2, dest);
                dynasm!(self.ops
                    ; .arch aarch64
                    ; sub W(Reg::Scratch1), W(Reg::Scratch1), W(Reg::MulSource)
                );
                self.store_cell(Reg::Scratch1, Reg::Scratch2, dest);
            }
            _ => {
                self.load_x(Reg::Scratch1, factor as u8 as u64);
                dynasm!(self.ops
                    ; .arch aarch64
                    ; mul W(Reg::Scratch0), W(Reg::MulSource), W(Reg::Scratch1)
                );
                self.load_cell(Reg::Scratch1, Reg::Scratch2, dest);
                dynasm!(self.ops
                    ; .arch aarch64
                    ; add W(Reg::Scratch1), W(Reg::Scratch1), W(Reg::Scratch0)
                );
                self.store_cell(Reg::Scratch1, Reg::Scratch2, dest);
            }
        }
    }

    /// Generates code for `Instruction::MulRun`.
    ///
    /// Optimized multiplication loops read from one source cell, apply one or more
    /// transfers, then clear the source. This loads the source once and reuses it
    /// for every destination update.
    fn mul_run(&mut self, base: i64, muls: &[(i64, i64)]) {
        self.load_cell(Reg::MulSource, Reg::Scratch2, base);

        for &(offset, factor) in muls {
            let dest = base + offset;

            match factor {
                1 => {
                    self.load_cell(Reg::Scratch1, Reg::Scratch2, dest);
                    dynasm!(self.ops
                        ; .arch aarch64
                        ; add W(Reg::Scratch1), W(Reg::Scratch1), W(Reg::MulSource)
                    );
                    self.store_cell(Reg::Scratch1, Reg::Scratch2, dest);
                }
                -1 => {
                    self.load_cell(Reg::Scratch1, Reg::Scratch2, dest);
                    dynasm!(self.ops
                        ; .arch aarch64
                        ; sub W(Reg::Scratch1), W(Reg::Scratch1), W(Reg::MulSource)
                    );
                    self.store_cell(Reg::Scratch1, Reg::Scratch2, dest);
                }
                _ => {
                    self.load_x(Reg::Scratch1, factor as u8 as u64);
                    dynasm!(self.ops
                        ; .arch aarch64
                        ; mul W(Reg::Scratch0), W(Reg::MulSource), W(Reg::Scratch1)
                    );
                    self.load_cell(Reg::Scratch1, Reg::Scratch2, dest);
                    dynasm!(self.ops
                        ; .arch aarch64
                        ; add W(Reg::Scratch1), W(Reg::Scratch1), W(Reg::Scratch0)
                    );
                    self.store_cell(Reg::Scratch1, Reg::Scratch2, dest);
                }
            }
        }

        self.zero_cell(Reg::Scratch2, base);
    }

    fn scan(&mut self, n: i64) {
        let move_label = self.ops.new_dynamic_label();
        let rest_label = self.ops.new_dynamic_label();

        dynasm!(self.ops
            ; .arch aarch64
            ; ldrb W(Reg::Scratch0), [X(Reg::TapePtr)]
            ; cbz W(Reg::Scratch0), =>rest_label
            ; =>move_label
        );

        self.move_tape(n);

        dynasm!(self.ops
            ; .arch aarch64
            ; ldrb W(Reg::Scratch0), [X(Reg::TapePtr)]
            ; cbnz W(Reg::Scratch0), =>move_label
            ; =>rest_label
        );
    }

    fn r#loop(&mut self, body: &Program) {
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

fn direct_byte_offset(offset: i64) -> Option<u32> {
    if (0..4096).contains(&offset) {
        Some(offset as u32)
    } else {
        None
    }
}
