use ast::{Instruction::*, Program};
use dynasm::dynasm;
use dynasmrt::{DynasmApi, DynasmLabelApi};
use std::io::Read;
use std::mem;

const PAGE_SIZE: usize = 4096;

extern "sysv64" fn putchar(c: u8) {
    print!("{}", c as char);
}

extern "sysv64" fn getchar() -> u8 {
    match std::io::stdin().bytes().next() {
        Some(Ok(c)) => c as _,
        _ => 0,
    }
}

extern "C" {
    fn memset(s: *mut libc::c_void, c: libc::uint32_t, n: libc::size_t) -> *mut libc::c_void;
}

/// Generates code and manages memory.
pub struct Jit {
    memory: *mut u8,
    ops: dynasmrt::x64::Assembler,
    start: dynasmrt::AssemblyOffset,
}

fn allocate(size: usize, prot: libc::c_int) -> *mut u8 {
    unsafe {
        let mut buffer: *mut libc::c_void = mem::uninitialized();
        libc::posix_memalign(&mut buffer, PAGE_SIZE, size);

        libc::mprotect(buffer, size, prot);

        memset(buffer, 0, size);

        mem::transmute(buffer)
    }
}

impl Jit {
    /// Initializes a `Jit` and allocates memory for the generated code
    /// plus 30000 bytes for the tape.
    pub fn allocate() -> Jit {
        let memory_size = ((30_000 + PAGE_SIZE) / PAGE_SIZE) * PAGE_SIZE;

        let memory = allocate(memory_size, libc::PROT_READ | libc::PROT_WRITE);

        let ops = dynasmrt::x64::Assembler::new().unwrap();

        Jit {
            memory: memory,
            start: ops.offset(),
            ops: ops,
        }
    }

    /// Finalizes the generated code and executes it.
    pub fn run(self) {
        let buf = self.ops.finalize().unwrap();
        let fun: extern "sysv64" fn() = unsafe { mem::transmute(buf.ptr(self.start)) };
        fun();
    }

    /// Generates machine code for the given program.
    pub fn generate(&mut self, program: &Program) {
        let mem_address: u64 = unsafe { mem::transmute(self.memory) };

        self.start = self.ops.offset();
        dynasm!(self.ops
                ; push rbp
                ; mov rbp, rsp
                ; mov rbx, QWORD mem_address as _
        );

        self.gen(program);

        dynasm!(self.ops
                ; mov rsp, rbp
                ; pop rbp
                ; ret
        );
    }

    fn gen(&mut self, program: &Program) {
        for ins in program.iter() {
            match ins {
                &Move(i) => {
                    dynasm!(self.ops
                            ; add rbx, i as _
                    );
                }
                &Add(i) => {
                    dynasm!(self.ops
                            ; add BYTE [rbx], i as _
                    );
                }
                Write => {
                    dynasm!(self.ops
                            ; movzx rdi, [rbx]
                            ; mov rax, QWORD putchar as _
                            ; call rax
                    );
                }
                Read => {
                    dynasm!(self.ops
                            ; mov rax, QWORD getchar as _
                            ; call rax
                            ; mov [rbx], al
                    );
                }
                Set(i) => {
                    dynasm!(self.ops
                            ; mov BYTE [rbx], (i % 0xFF) as _
                    );
                }
                Mul(offset, mul) => {
                    dynasm!(self.ops
                            ; mov al, *mul as _
                            ; mul BYTE [rbx]
                            ; add [rbx + *offset as _], al
                    );
                }
                &Scan(i) => {
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
                Loop(body) => {
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
