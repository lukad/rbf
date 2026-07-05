use dynasmrt::{AssemblyOffset, ExecutableBuffer};
use std::io::Read;
use std::io::{self, Write};
use std::mem;

pub(crate) extern "C" fn putchar(c: u8) {
    print!("{}", c as char);
}

pub(crate) extern "C" fn putbytes(buf: *const u8, count: u64) {
    let bytes = unsafe { std::slice::from_raw_parts(buf, count as usize) };
    std::io::stdout().write_all(bytes).unwrap();
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
    buf: ExecutableBuffer,
    start: AssemblyOffset,
    // Keeps byte literals alive when generated code stores their raw pointers.
    _literals: Vec<Box<[u8]>>,
}

impl Function {
    pub(super) fn new(
        buf: ExecutableBuffer,
        start: AssemblyOffset,
        literals: Vec<Box<[u8]>>,
    ) -> Self {
        Self {
            buf,
            start,
            _literals: literals,
        }
    }

    pub fn run(&self) {
        let fun: extern "C" fn() = unsafe { mem::transmute(self.buf.ptr(self.start)) };
        (fun)();
    }
}

#[cfg(test)]
impl Function {
    pub(crate) fn literal_count(&self) -> usize {
        self._literals.len()
    }
}
