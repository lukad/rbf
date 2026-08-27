use dynasmrt::{AssemblyOffset, ExecutableBuffer};
use std::io::Read;
use std::io::{self, Write};
use std::mem;

pub(crate) extern "C" fn putchar(c: u8) {
    io::stdout().write_all(&[c]).unwrap();
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

/// A runtime helper the generated code calls into.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Helper {
    PutChar,
    PutBytes,
    GetChar,
    MemZero,
}

impl Helper {
    pub(crate) const ALL: [Helper; 4] = [
        Helper::PutChar,
        Helper::PutBytes,
        Helper::GetChar,
        Helper::MemZero,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Helper::PutChar => "putchar",
            Helper::PutBytes => "putbytes",
            Helper::GetChar => "getchar",
            Helper::MemZero => "memzero",
        }
    }

    /// The address the generated code calls.
    ///
    /// Both the backends and [`Function::symbol`] take addresses here so that
    /// they agree on one address per helper. Writing `memzero as *const ()` at
    /// each site does not agree: a function that small is cheaper to duplicate
    /// than to call across codegen units, so an optimized build gave the
    /// backend a different copy than the one `symbol` compared against and the
    /// disassembly left the call unnamed. `#[inline(never)]` keeps this
    /// function itself to a single copy, and with it a single set of answers.
    #[inline(never)]
    pub(crate) fn address(self) -> usize {
        match self {
            Helper::PutChar => putchar as *const () as usize,
            Helper::PutBytes => putbytes as *const () as usize,
            Helper::GetChar => getchar as *const () as usize,
            Helper::MemZero => memzero as *const () as usize,
        }
    }
}

/// An address the generated code refers to.
#[derive(Clone, Copy, Debug)]
pub enum Symbol<'a> {
    /// One of the runtime helpers the generated code calls into.
    Helper(&'static str),
    /// A byte literal the generated code writes to STDOUT.
    Literal(&'a [u8]),
}

#[derive(Debug)]
pub struct Function {
    buf: ExecutableBuffer,
    start: AssemblyOffset,
    // Keeps byte literals alive while generated code stores their raw pointers.
    literals: Vec<Box<[u8]>>,
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
            literals,
        }
    }

    pub fn run(&self) {
        let fun: extern "C" fn() = unsafe { mem::transmute(self.buf.ptr(self.start)) };
        (fun)();
    }

    /// The generated machine code, starting at the function's entry point
    pub fn code(&self) -> &[u8] {
        &self.buf[self.start.0..]
    }

    /// Names `address` if the generated code refers to it
    pub fn symbol(&self, address: usize) -> Option<Symbol<'_>> {
        if let Some(helper) = Helper::ALL
            .iter()
            .find(|helper| helper.address() == address)
        {
            return Some(Symbol::Helper(helper.name()));
        }

        self.literals
            .iter()
            .find(|literal| literal.as_ptr() as usize == address)
            .map(|literal| Symbol::Literal(literal))
    }
}

#[cfg(test)]
impl Function {
    pub(crate) fn literal_count(&self) -> usize {
        self.literals.len()
    }

    pub(crate) fn code_size(&self) -> usize {
        self.code().len()
    }
}
