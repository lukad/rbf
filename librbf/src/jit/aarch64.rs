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

#[derive(Clone, Copy)]
enum CellState {
    Default,
    Known(u8),
    Unknown,
}

const MAX_FACT_CELLS: usize = 4096;

struct CellFacts {
    default_zero: bool,
    base: i64,
    cells: Vec<CellState>,
}

impl CellFacts {
    fn new(default_zero: bool) -> Self {
        Self {
            default_zero,
            base: 0,
            cells: Vec::new(),
        }
    }

    fn index(&self, offset: i64) -> Option<usize> {
        let index = offset - self.base;

        (index >= 0 && (index as usize) < self.cells.len()).then_some(index as usize)
    }

    fn state(&self, offset: i64) -> CellState {
        if let Some(index) = self.index(offset) {
            self.cells[index]
        } else {
            CellState::Default
        }
    }

    fn known(&self, offset: i64) -> Option<u8> {
        match self.state(offset) {
            CellState::Known(n) => Some(n),
            CellState::Default if self.default_zero => Some(0),
            CellState::Unknown | CellState::Default => None,
        }
    }

    fn set_known(&mut self, offset: i64, value: u8) {
        if self.default_zero && value == 0 {
            self.set_default(offset);
            return;
        }
        if let Some(i) = self.ensure_offset(offset) {
            self.cells[i] = CellState::Known(value)
        }
    }

    fn set_unknown(&mut self, offset: i64) {
        if self.default_zero {
            if let Some(i) = self.ensure_offset(offset) {
                self.cells[i] = CellState::Unknown;
            }
        } else if let Some(i) = self.index(offset) {
            self.cells[i] = CellState::Default;
        }
    }

    fn set_default(&mut self, offset: i64) {
        if let Some(i) = self.index(offset) {
            self.cells[i] = CellState::Default;
        }
    }

    fn forget_all(&mut self) {
        self.default_zero = false;
        self.base = 0;
        self.cells.clear();
    }

    fn ensure_offset(&mut self, offset: i64) -> Option<usize> {
        if self.cells.is_empty() {
            self.base = offset;
            self.cells.push(CellState::Default);
            return Some(0);
        }

        let start = self.base.min(offset);
        let end = (self.base + self.cells.len() as i64 - 1).max(offset);
        let len = (end - start + 1) as usize;

        if len > MAX_FACT_CELLS {
            self.forget_all();
            return None;
        }

        if start != self.base {
            let prepend = (self.base - start) as usize;
            let mut cells = vec![CellState::Default; prepend];
            cells.extend_from_slice(&self.cells);
            self.cells = cells;
            self.base = start;
        }

        if end >= self.base + self.cells.len() as i64 {
            self.cells.resize(len, CellState::Default);
        }

        Some((offset - self.base) as usize)
    }

    fn reset_to_current_zero(&mut self) {
        self.default_zero = false;
        self.base = 0;
        self.cells.clear();
        self.cells.push(CellState::Known(0));
    }

    fn rebase(&mut self, shift: i64) {
        self.base -= shift;
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
    literals: Vec<Box<[u8]>>,
}

impl Jit {
    /// Initializes a `Jit` with a tape size of `30_000`.
    pub fn new() -> Jit {
        let ops = dynasmrt::aarch64::Assembler::new().unwrap();

        Jit {
            tape_size: 30_000,
            start: ops.offset(),
            ops,
            literals: Vec::new(),
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
        Function::new(buf, self.start, self.literals)
    }

    fn generate(&mut self, program: &Program) {
        self.generate_with_facts(program, true);
    }

    fn generate_with_facts(&mut self, program: &Program, default_zero: bool) {
        let mut offset = 0;
        let mut facts = CellFacts::new(default_zero);

        for ins in program.iter() {
            match ins {
                &Move(i) => offset += i,
                &Add(n) => {
                    let delta = n as u8;

                    if let Some(old) = facts.known(offset) {
                        let new = old.wrapping_add(delta);

                        if new != old {
                            self.set(offset, new as i64);
                        }

                        facts.set_known(offset, new);
                    } else {
                        self.add(offset, n);
                        facts.set_unknown(offset);
                    }
                }
                &Set(n) => {
                    let value = n as u8;

                    if facts.known(offset) != Some(value) {
                        self.set(offset, n);
                    }

                    facts.set_known(offset, value);
                }
                &Mul(o, f) => {
                    let source = offset;
                    let dest = offset + o;

                    if let Some(src) = facts.known(source) {
                        let delta = src.wrapping_mul(f as u8);

                        if delta == 0 {
                            continue;
                        }

                        if let Some(dst) = facts.known(dest) {
                            let new = dst.wrapping_add(delta);
                            self.set(dest, new as i64);
                            facts.set_known(dest, new);
                        } else {
                            self.add(dest, delta as i64);
                            facts.set_unknown(dest);
                        }
                    } else {
                        self.mul(source, dest, f);
                        facts.set_unknown(dest);
                    }
                }
                MulRun(muls) => {
                    if let Some(src) = facts.known(offset) {
                        if src != 0 {
                            for &(o, factor) in muls {
                                let dest = offset + o;
                                let delta = src.wrapping_mul(factor as u8);

                                if delta == 0 {
                                    continue;
                                }

                                if let Some(dst) = facts.known(dest) {
                                    let new = dst.wrapping_add(delta);
                                    self.set(dest, new as i64);
                                    facts.set_known(dest, new);
                                } else {
                                    self.add(dest, delta as i64);
                                    facts.set_unknown(dest);
                                }
                            }

                            self.zero_cell(Reg::Scratch2, offset);
                        }

                        facts.set_known(offset, 0);
                    } else {
                        self.mul_run(offset, muls);

                        for &(o, _) in muls {
                            facts.set_unknown(offset + o);
                        }

                        facts.set_known(offset, 0);
                    }
                }
                Write => {
                    if let Some(value) = facts.known(offset) {
                        self.write_byte(value);
                    } else {
                        self.write(offset);
                    }
                }
                Read => {
                    self.read(offset);
                    facts.set_unknown(offset);
                }
                &WriteConst(n) => {
                    let value = n as u8;

                    if facts.known(offset) != Some(value) {
                        self.set(offset, value as i64);
                        facts.set_known(offset, value);
                    }

                    self.write_byte(value);
                }
                WriteBytes(bytes) => {
                    let last = *bytes.last().unwrap();

                    if facts.known(offset) != Some(last) {
                        self.set(offset, last as i64);
                        facts.set_known(offset, last);
                    }

                    self.write_bytes(bytes);
                }
                &Scan(n) => {
                    if facts.known(offset) == Some(0) {
                        continue;
                    }

                    self.flush_offset(&mut offset, &mut facts);
                    self.scan(n);
                    facts.reset_to_current_zero();
                }
                Loop(body) => {
                    if facts.known(offset) == Some(0) {
                        continue;
                    }

                    self.flush_offset(&mut offset, &mut facts);
                    self.r#loop(body);

                    facts.reset_to_current_zero();
                }
            }
        }

        self.flush_offset(&mut offset, &mut facts);
    }

    fn generate_without_facts(&mut self, program: &Program) {
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
                &WriteConst(n) => {
                    let value = n as u8;
                    self.set(offset, value as i64);
                    self.write_byte(value);
                }
                WriteBytes(bytes) => {
                    let last = *bytes.last().unwrap();
                    self.set(offset, last as i64);
                    self.write_bytes(bytes);
                }
                &Scan(n) => {
                    self.flush_offset_without_facts(&mut offset);
                    self.scan(n);
                }
                Loop(body) => {
                    self.flush_offset_without_facts(&mut offset);
                    self.r#loop(body);
                }
            }
        }

        self.flush_offset_without_facts(&mut offset);
    }

    /// Flushes the offset to the tape pointer and resets it to 0.
    fn flush_offset(&mut self, offset: &mut i64, facts: &mut CellFacts) {
        let shift = *offset;
        self.move_tape(shift);
        facts.rebase(shift);
        *offset = 0;
    }

    fn flush_offset_without_facts(&mut self, offset: &mut i64) {
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

        if let Some(offset) = direct_signed_byte_offset(offset) {
            dynasm!(self.ops
                ; .arch aarch64
                ; ldurb W(dst), [X(Reg::TapePtr), #offset]
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

        if let Some(offset) = direct_signed_byte_offset(offset) {
            dynasm!(self.ops
                ; .arch aarch64
                ; sturb W(src), [X(Reg::TapePtr), #offset]
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

        if let Some(offset) = direct_signed_byte_offset(offset) {
            dynasm!(self.ops
                ; .arch aarch64
                ; sturb wzr, [X(Reg::TapePtr), #offset]
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
        let value = n as u8;

        if value == 0 {
            return;
        }

        let value = value as u32;

        self.load_cell(Reg::Scratch0, Reg::Scratch2, offset);
        dynasm!(self.ops
            ; .arch aarch64
            ; add WSP(Reg::Scratch0), WSP(Reg::Scratch0), #value
        );
        self.store_cell(Reg::Scratch0, Reg::Scratch2, offset);
    }

    fn set(&mut self, offset: i64, n: i64) {
        let value = n as u8;

        if value == 0 {
            self.zero_cell(Reg::Scratch2, offset);
            return;
        }

        self.load_x(Reg::Scratch0, value as u64);
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

    fn write_byte(&mut self, byte: u8) {
        self.load_x(Reg::Arg0, byte as u64);
        dynasm!(self.ops
            ; .arch aarch64
            ; blr X(Reg::PutCharTarget)
        );
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        let (ptr, len) = self.retain_bytes(bytes);

        self.load_x(Reg::Arg0, ptr as u64);
        self.load_x(Reg::Arg1, len as u64);

        dynasm!(self.ops
            ; .arch aarch64
            ; blr X(Reg::PutBytesTarget)
        );
    }

    fn retain_bytes(&mut self, bytes: &[u8]) -> (*const u8, usize) {
        let bytes = bytes.to_vec().into_boxed_slice();
        let ptr = bytes.as_ptr();
        let len = bytes.len();
        self.literals.push(bytes);
        (ptr, len)
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

        self.generate_without_facts(body);

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

fn direct_signed_byte_offset(offset: i64) -> Option<i32> {
    if (-256..0).contains(&offset) {
        Some(offset as i32)
    } else {
        None
    }
}
