use std::fmt::{Display, Write as _};

use librbf::{Function, Symbol};
use yaxpeax_arch::{Arch, Decoder, LengthedInstruction, Reader, U8Reader};

/// How many bytes of a literal a comment shows before it is cut short
const LITERAL_PREVIEW: usize = 32;

/// A single decoded instruction of the generated code
struct Instruction {
    offset: usize,
    len: usize,
    text: String,
    comment: Option<String>,
}

/// Disassembles a compiled function into an offset prefixed listing
pub fn disassemble(function: &Function) -> String {
    format_listing(function.code(), &decode(function))
}

#[cfg(target_arch = "x86_64")]
fn decode(function: &Function) -> Vec<Instruction> {
    decode_arch::<yaxpeax_x86::long_mode::Arch>(function)
}

#[cfg(target_arch = "aarch64")]
fn decode(function: &Function) -> Vec<Instruction> {
    decode_arch::<yaxpeax_arm::armv8::a64::ARMv8>(function)
}

/// Decodes instructions one at a time, skipping ahead by the architecture's
/// smallest instruction whenever a word does not decode
fn decode_arch<A>(function: &Function) -> Vec<Instruction>
where
    A: Arch<Address = u64, Word = u8>,
    A::Instruction: Display,
{
    let code = function.code();
    let decoder = A::Decoder::default();
    let min_size = (A::Instruction::min_size().to_const() as usize).max(1);

    let mut addresses = Addresses::new();
    let mut instructions = Vec::new();
    let mut offset = 0;

    while offset < code.len() {
        let rest = &code[offset..];
        let mut reader = U8Reader::new(rest);

        let (len, text) = match decoder.decode(&mut reader) {
            Ok(instruction) => (
                Reader::<u64, u8>::total_offset(&mut reader) as usize,
                instruction.to_string(),
            ),
            Err(error) => (min_size, format!("(bad: {error})")),
        };

        let len = len.clamp(1, rest.len());
        let comment = addresses
            .in_instruction(&rest[..len])
            .into_iter()
            .find_map(|address| function.symbol(address as usize))
            .map(describe);

        instructions.push(Instruction {
            offset,
            len,
            text,
            comment,
        });
        offset += len;
    }

    instructions
}

/// Recovers the addresses an instruction materializes, so that the calls into
/// the runtime helpers and the bulk writes of byte literals can be named
#[cfg(target_arch = "x86_64")]
struct Addresses;

#[cfg(target_arch = "x86_64")]
impl Addresses {
    fn new() -> Self {
        Self
    }

    /// x86_64 materializes an address as one immediate, so it can sit in any
    /// eight byte window of the instruction
    fn in_instruction(&mut self, bytes: &[u8]) -> Vec<u64> {
        bytes
            .windows(8)
            .map(|window| u64::from_le_bytes(window.try_into().expect("window is eight bytes")))
            .collect()
    }
}

/// Recovers the addresses an instruction materializes, so that the calls into
/// the runtime helpers and the bulk writes of byte literals can be named
#[cfg(target_arch = "aarch64")]
struct Addresses {
    registers: [u64; 32],
}

#[cfg(target_arch = "aarch64")]
impl Addresses {
    fn new() -> Self {
        Self { registers: [0; 32] }
    }

    /// aarch64 builds up an address with a `movz` and up to three `movk`
    /// instructions, so the register they write has to be followed
    fn in_instruction(&mut self, bytes: &[u8]) -> Vec<u64> {
        let Ok(word) = <[u8; 4]>::try_from(bytes) else {
            return Vec::new();
        };

        let word = u32::from_le_bytes(word);
        let register = (word & 0x1f) as usize;
        let shift = 16 * ((word >> 21) & 0x3);
        let value = u64::from((word >> 5) & 0xffff) << shift;

        match word & 0xff80_0000 {
            // movz X(register), #value, lsl #shift
            0xd280_0000 => self.registers[register] = value,
            // movk X(register), #value, lsl #shift
            0xf280_0000 => {
                self.registers[register] &= !(0xffff << shift);
                self.registers[register] |= value;
            }
            _ => return Vec::new(),
        }

        vec![self.registers[register]]
    }
}

/// Describes a symbol for the comment column
fn describe(symbol: Symbol) -> String {
    match symbol {
        Symbol::Helper(name) => name.to_string(),
        Symbol::Literal(bytes) => {
            let shown = &bytes[..bytes.len().min(LITERAL_PREVIEW)];
            let mut text = String::from('"');

            for &byte in shown {
                match byte {
                    b'"' => text.push_str("\\\""),
                    b'\\' => text.push_str("\\\\"),
                    b'\n' => text.push_str("\\n"),
                    b'\r' => text.push_str("\\r"),
                    b'\t' => text.push_str("\\t"),
                    0x20..=0x7e => text.push(byte as char),
                    _ => write!(text, "\\x{byte:02x}").expect("writing to a string succeeds"),
                }
            }

            text.push('"');

            if shown.len() < bytes.len() {
                text.push_str("...");
            }

            text
        }
    }
}

fn format_listing(code: &[u8], instructions: &[Instruction]) -> String {
    let offset_width = format!("{:x}", code.len().saturating_sub(1)).len().max(4);
    let bytes_width = instructions.iter().map(|i| i.len * 2).max().unwrap_or(0);

    let mut listing = String::new();

    for instruction in instructions {
        let mut bytes = String::new();
        for byte in &code[instruction.offset..instruction.offset + instruction.len] {
            write!(bytes, "{byte:02x}").expect("writing to a string succeeds");
        }

        let comment = match &instruction.comment {
            Some(comment) => format!("  ; {comment}"),
            None => String::new(),
        };

        writeln!(
            listing,
            "{:0offset_width$x}  {:bytes_width$}  {}{comment}",
            instruction.offset, bytes, instruction.text
        )
        .expect("writing to a string succeeds");
    }

    if !instructions.is_empty() {
        writeln!(
            listing,
            "\n; {} instructions, {} bytes",
            instructions.len(),
            code.len()
        )
        .expect("writing to a string succeeds");
    }

    listing
}

#[cfg(all(test, target_arch = "aarch64"))]
mod tests {
    use super::Addresses;
    use yaxpeax_arch::{Decoder, U8Reader};
    use yaxpeax_arm::armv8::a64::InstDecoder;

    /// Encodes an address the way the aarch64 backend's `load_x` lowers it
    fn load_x(register: u32, value: u64) -> Vec<u8> {
        let mut words = vec![0xd280_0000 | ((value as u32 & 0xffff) << 5) | register];

        for chunk in 1..4 {
            let part = ((value >> (16 * chunk)) & 0xffff) as u32;

            if part != 0 {
                words.push(0xf280_0000 | (chunk << 21) | (part << 5) | register);
            }
        }

        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    #[test]
    fn tracks_movz_movk_sequences() {
        let address = 0x55c3_1402_2970;
        let code = load_x(16, address);
        let decoder = InstDecoder::default();

        let mut addresses = Addresses::new();
        let mut instructions = Vec::new();
        let mut recovered = Vec::new();

        for word in code.chunks(4) {
            instructions.push(
                decoder
                    .decode(&mut U8Reader::new(word))
                    .expect("word decodes")
                    .to_string(),
            );
            recovered = addresses.in_instruction(word);
        }

        assert_eq!(
            instructions,
            [
                "mov x16, #0x2970",
                "movk x16, #0x1402, lsl #16",
                "movk x16, #0x55c3, lsl #32"
            ]
        );
        assert_eq!(recovered, vec![address]);
    }

    #[test]
    fn later_loads_replace_earlier_ones() {
        let mut addresses = Addresses::new();

        for word in load_x(16, u64::MAX).chunks(4) {
            addresses.in_instruction(word);
        }

        assert_eq!(
            addresses.in_instruction(&load_x(16, 0x1234)),
            vec![0x1234],
            "a later load did not clear the register"
        );
    }

    #[test]
    fn ignores_other_instructions() {
        // ret
        assert!(
            Addresses::new()
                .in_instruction(&0xd65f_03c0u32.to_le_bytes())
                .is_empty()
        );
    }
}
