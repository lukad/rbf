use ast::Instruction::*;
use ast::Program;

use std::io::{self, Read, Write};

type Cell = u8;

pub struct Machine<'a> {
    mem: [Cell; 30000],
    ptr: i64,
    program: &'a Program,
}

impl<'a> Machine<'a> {
    pub fn new(program: &'a Program) -> Machine {
        Machine {
            mem: [0; 30000],
            ptr: 0,
            program: program,
        }
    }

    fn current(&mut self) -> Cell {
        self.mem[self.ptr as usize]
    }

    fn set(&mut self, x: Cell) {
        self.mem[self.ptr as usize] = x
    }

    fn add(&mut self, x: i64) {
        let y = self.current().wrapping_add(x as Cell);
        self.set(y)
    }

    fn sub(&mut self, x: i64) {
        let y = self.current().wrapping_sub(x as Cell);
        self.set(y)
    }

    fn print(&self) {
        print!("{}", self.mem[self.ptr as usize] as char);
        io::stdout().flush().unwrap();
    }

    /// Reset the machine and run it.
    pub fn run(&mut self) {
        self.r(self.program);
    }

    fn r(&mut self, prog: &Program) {
        let mut iter = prog.iter();
        while let Some(ins) = iter.next() {
            match ins {
                Add(x) if *x >= 0 => self.add(*x),
                Add(x) if *x < 0 => self.sub(-*x),
                Add(_) => (),
                Move(x) => self.ptr = (self.ptr + x) % 30000,
                Set(x) => self.mem[self.ptr as usize] = *x as Cell,
                Loop(ref body) => {
                    while self.current() != 0 {
                        self.r(body)
                    }
                }
                Write => self.print(),
                Read => {
                    if let Some(c) = std::io::stdin()
                        .bytes()
                        .next()
                        .and_then(|result| result.ok())
                        .map(|byte| byte as Cell)
                    {
                        self.mem[self.ptr as usize] = c;
                    }
                }
                Scan(x) => {
                    while self.mem[self.ptr as usize] != 0 {
                        self.ptr = (self.ptr + x) % 30000
                    }
                }
            }
        }
    }
}
