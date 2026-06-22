use dynasmrt::{AssemblyOffset, ExecutableBuffer};
use std::io::Read;
use std::io::{self, Write};
use std::mem;

pub(crate) extern "C" fn putchar(c: u8) {
    print!("{}", c as char);
}

pub(crate) extern "C" fn getchar() -> u8 {
    io::stdout().flush().unwrap();
    let mut buf = [0];
    match io::stdin().lock().read(&mut buf) {
        Ok(1) => buf[0],
        _ => 0,
    }
}

pub(crate) extern "C" fn memzero(dst: *mut u8, count: usize) {
    unsafe { std::ptr::write_bytes(dst, 0, count) }
}

#[derive(Debug)]
pub struct Function {
    _buf: ExecutableBuffer,
    fun: extern "C" fn(),
}

impl Function {
    pub(super) fn new(buf: ExecutableBuffer, start: AssemblyOffset) -> Self {
        let fun: extern "C" fn() = unsafe { mem::transmute(buf.ptr(start)) };

        Self { _buf: buf, fun }
    }

    pub fn run(&self) {
        (self.fun)();
    }
}
