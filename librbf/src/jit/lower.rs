use crate::ast::{Instruction::*, Program};

pub(super) trait Emitter {
    fn move_tape(&mut self, offset: i64);
    fn add(&mut self, offset: i64, value: u8);
    fn set(&mut self, offset: i64, value: u8);
    fn mul(&mut self, source: i64, dest: i64, factor: i64);
    fn mul_run(&mut self, base: i64, muls: &[(i64, i64)]);
    fn write(&mut self, offset: i64);
    fn read(&mut self, offset: i64);
    fn write_byte(&mut self, byte: u8);
    fn write_bytes(&mut self, bytes: &[u8]);
    fn scan(&mut self, step: i64);
    fn r#loop(&mut self, body: &Program);
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
    fn new() -> Self {
        Self {
            default_zero: true,
            base: 0,
            cells: Vec::new(),
        }
    }

    fn index(&self, offset: i64) -> Option<usize> {
        let index = offset - self.base;

        (index >= 0 && (index as usize) < self.cells.len()).then_some(index as usize)
    }

    fn state(&self, offset: i64) -> CellState {
        self.index(offset)
            .map_or(CellState::Default, |index| self.cells[index])
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
        } else if let Some(i) = self.ensure_offset(offset) {
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

pub(super) fn generate(emitter: &mut impl Emitter, program: &Program) {
    let mut offset = 0;
    let mut facts = CellFacts::new();

    for ins in program {
        match ins {
            &Move(i) => offset += i,
            &Add(n) => {
                let delta = n as u8;

                if let Some(old) = facts.known(offset) {
                    let new = old.wrapping_add(delta);

                    if new != old {
                        emitter.set(offset, new);
                    }

                    facts.set_known(offset, new);
                } else {
                    emitter.add(offset, delta);
                    facts.set_unknown(offset);
                }
            }
            &Set(n) => {
                let value = n as u8;

                if facts.known(offset) != Some(value) {
                    emitter.set(offset, value);
                }

                facts.set_known(offset, value);
            }
            &Mul(o, factor) => {
                let source = offset;
                let dest = offset + o;

                if let Some(src) = facts.known(source) {
                    let delta = src.wrapping_mul(factor as u8);

                    if delta == 0 {
                        continue;
                    }

                    if let Some(dst) = facts.known(dest) {
                        let new = dst.wrapping_add(delta);
                        emitter.set(dest, new);
                        facts.set_known(dest, new);
                    } else {
                        emitter.add(dest, delta);
                        facts.set_unknown(dest);
                    }
                } else {
                    emitter.mul(source, dest, factor);
                    facts.set_unknown(dest);
                }
            }
            MulRun(muls) => {
                if let Some(src) = facts.known(offset) {
                    if src != 0 {
                        for &(relative, factor) in muls {
                            let dest = offset + relative;
                            let delta = src.wrapping_mul(factor as u8);

                            if delta == 0 {
                                continue;
                            }

                            if let Some(dst) = facts.known(dest) {
                                let new = dst.wrapping_add(delta);
                                emitter.set(dest, new);
                                facts.set_known(dest, new);
                            } else {
                                emitter.add(dest, delta);
                                facts.set_unknown(dest);
                            }
                        }

                        emitter.set(offset, 0);
                    }

                    facts.set_known(offset, 0);
                } else {
                    emitter.mul_run(offset, muls);

                    for &(relative, _) in muls {
                        facts.set_unknown(offset + relative);
                    }

                    facts.set_known(offset, 0);
                }
            }
            Write => match facts.known(offset) {
                Some(value) => emitter.write_byte(value),
                None => emitter.write(offset),
            },
            Read => {
                emitter.read(offset);
                facts.set_unknown(offset);
            }
            &WriteConst(n) => {
                let value = n as u8;

                if facts.known(offset) != Some(value) {
                    emitter.set(offset, value);
                    facts.set_known(offset, value);
                }

                emitter.write_byte(value);
            }
            WriteBytes(bytes) => {
                let last = *bytes.last().unwrap();

                if facts.known(offset) != Some(last) {
                    emitter.set(offset, last);
                    facts.set_known(offset, last);
                }

                emitter.write_bytes(bytes);
            }
            &Scan(step) => {
                if facts.known(offset) == Some(0) {
                    continue;
                }

                flush(emitter, &mut offset, &mut facts);
                emitter.scan(step);
                facts.reset_to_current_zero();
            }
            Loop(body) => {
                if facts.known(offset) == Some(0) {
                    continue;
                }

                flush(emitter, &mut offset, &mut facts);
                emitter.r#loop(body);
                facts.reset_to_current_zero();
            }
        }
    }

    flush(emitter, &mut offset, &mut facts);
}

pub(super) fn generate_without_facts(emitter: &mut impl Emitter, program: &Program) {
    let mut offset = 0;

    for ins in program {
        match ins {
            &Move(i) => offset += i,
            &Add(n) => emitter.add(offset, n as u8),
            &Set(n) => emitter.set(offset, n as u8),
            &Mul(relative, factor) => emitter.mul(offset, offset + relative, factor),
            MulRun(muls) => emitter.mul_run(offset, muls),
            Write => emitter.write(offset),
            Read => emitter.read(offset),
            &WriteConst(n) => {
                let value = n as u8;
                emitter.set(offset, value);
                emitter.write_byte(value);
            }
            WriteBytes(bytes) => {
                emitter.set(offset, *bytes.last().unwrap());
                emitter.write_bytes(bytes);
            }
            &Scan(step) => {
                flush_without_facts(emitter, &mut offset);
                emitter.scan(step);
            }
            Loop(body) => {
                flush_without_facts(emitter, &mut offset);
                emitter.r#loop(body);
            }
        }
    }

    flush_without_facts(emitter, &mut offset);
}

fn flush(emitter: &mut impl Emitter, offset: &mut i64, facts: &mut CellFacts) {
    emitter.move_tape(*offset);
    facts.rebase(*offset);
    *offset = 0;
}

fn flush_without_facts(emitter: &mut impl Emitter, offset: &mut i64) {
    emitter.move_tape(*offset);
    *offset = 0;
}
