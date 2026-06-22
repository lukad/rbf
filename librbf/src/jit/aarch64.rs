use super::Function;
use super::common::{getchar, memzero, putchar};
use crate::ast::{Instruction::*, Program};
use dynasm::dynasm;
use dynasmrt::{DynasmApi, DynasmLabelApi};

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
                ; stp x29, x30, [sp, #-16]!
                ; mov x29, sp
                ; stp x19, x20, [sp, #-16]!
        );

        self.load_x9(self.tape_size as u64);
        dynasm!(self.ops
                ; .arch aarch64
                ; sub sp, sp, x9
                ; mov x19, sp
        );

        self.load_x1(self.tape_size as u64);
        self.load_x16(memzero as *const () as u64);
        dynasm!(self.ops
                ; .arch aarch64
                ; mov x0, x19
                ; blr x16
        );

        self.generate(program);

        self.load_x9(self.tape_size as u64);
        dynasm!(self.ops
                ; .arch aarch64
                ; add sp, sp, x9
                ; ldp x19, x20, [sp], #16
                ; ldp x29, x30, [sp], #16
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
                    self.load_x10(i as u8 as u64);
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; ldrb w9, [x19]
                            ; add w9, w9, w10
                            ; strb w9, [x19]
                    );
                }
                Write => {
                    self.load_x16(putchar as *const () as u64);
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; ldrb w0, [x19]
                            ; blr x16
                    );
                }
                Read => {
                    self.load_x16(getchar as *const () as u64);
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; blr x16
                            ; strb w0, [x19]
                    );
                }
                Set(i) => {
                    self.load_x9((i % 0xFF) as u8 as u64);
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; strb w9, [x19]
                    );
                }
                &Mul(offset, mul) => {
                    self.compute_offset_x11(offset);
                    self.load_x10(mul as u8 as u64);
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; ldrb w9, [x19]
                            ; mul w9, w9, w10
                            ; ldrb w10, [x11]
                            ; add w10, w10, w9
                            ; strb w10, [x11]
                    );
                }
                &Scan(i) => {
                    let move_label = self.ops.new_dynamic_label();
                    let rest_label = self.ops.new_dynamic_label();
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; ldrb w9, [x19]
                            ; cbz w9, =>rest_label
                            ; =>move_label
                    );

                    self.move_tape(i);

                    dynasm!(self.ops
                            ; .arch aarch64
                            ; ldrb w9, [x19]
                            ; cbnz w9, =>move_label
                            ; =>rest_label
                    );
                }
                Loop(body) => {
                    let body_label = self.ops.new_dynamic_label();
                    let rest_label = self.ops.new_dynamic_label();
                    dynasm!(self.ops
                            ; .arch aarch64
                            ; ldrb w9, [x19]
                            ; cbz w9, =>rest_label
                            ; =>body_label
                    );

                    self.generate(body);

                    dynasm!(self.ops
                            ; .arch aarch64
                            ; ldrb w9, [x19]
                            ; cbnz w9, =>body_label
                            ; =>rest_label
                    );
                }
            }
        }
    }

    fn move_tape(&mut self, offset: i64) {
        if offset == 0 {
            return;
        }

        self.load_x9(offset.unsigned_abs());
        if offset > 0 {
            dynasm!(self.ops
                    ; .arch aarch64
                    ; add x19, x19, x9
            );
        } else {
            dynasm!(self.ops
                    ; .arch aarch64
                    ; sub x19, x19, x9
            );
        }
    }

    fn compute_offset_x11(&mut self, offset: i64) {
        dynasm!(self.ops
                ; .arch aarch64
                ; mov x11, x19
        );

        if offset == 0 {
            return;
        }

        self.load_x9(offset.unsigned_abs());
        if offset > 0 {
            dynasm!(self.ops
                    ; .arch aarch64
                    ; add x11, x11, x9
            );
        } else {
            dynasm!(self.ops
                    ; .arch aarch64
                    ; sub x11, x11, x9
            );
        }
    }

    fn load_x1(&mut self, value: u64) {
        let p0 = (value & 0xFFFF) as u32;
        let p1 = ((value >> 16) & 0xFFFF) as u32;
        let p2 = ((value >> 32) & 0xFFFF) as u32;
        let p3 = ((value >> 48) & 0xFFFF) as u32;

        dynasm!(self.ops
                ; .arch aarch64
                ; movz x1, #p0
                ; movk x1, #p1, lsl #16
                ; movk x1, #p2, lsl #32
                ; movk x1, #p3, lsl #48
        );
    }

    fn load_x9(&mut self, value: u64) {
        let p0 = (value & 0xFFFF) as u32;
        let p1 = ((value >> 16) & 0xFFFF) as u32;
        let p2 = ((value >> 32) & 0xFFFF) as u32;
        let p3 = ((value >> 48) & 0xFFFF) as u32;

        dynasm!(self.ops
                ; .arch aarch64
                ; movz x9, #p0
                ; movk x9, #p1, lsl #16
                ; movk x9, #p2, lsl #32
                ; movk x9, #p3, lsl #48
        );
    }

    fn load_x10(&mut self, value: u64) {
        let p0 = (value & 0xFFFF) as u32;
        let p1 = ((value >> 16) & 0xFFFF) as u32;
        let p2 = ((value >> 32) & 0xFFFF) as u32;
        let p3 = ((value >> 48) & 0xFFFF) as u32;

        dynasm!(self.ops
                ; .arch aarch64
                ; movz x10, #p0
                ; movk x10, #p1, lsl #16
                ; movk x10, #p2, lsl #32
                ; movk x10, #p3, lsl #48
        );
    }

    fn load_x16(&mut self, value: u64) {
        let p0 = (value & 0xFFFF) as u32;
        let p1 = ((value >> 16) & 0xFFFF) as u32;
        let p2 = ((value >> 32) & 0xFFFF) as u32;
        let p3 = ((value >> 48) & 0xFFFF) as u32;

        dynasm!(self.ops
                ; .arch aarch64
                ; movz x16, #p0
                ; movk x16, #p1, lsl #16
                ; movk x16, #p2, lsl #32
                ; movk x16, #p3, lsl #48
        );
    }
}

impl Default for Jit {
    fn default() -> Self {
        Self::new()
    }
}
