use std::io::Read;

/// Holds a `Vec<Instruction>`.
#[derive(Debug, PartialEq)]
pub struct Program(pub Vec<Instruction>);

/// AST for the for the Brainfuck Language.
#[derive(Debug, PartialEq)]
pub enum Instruction {
    /// Used to represent non-brainfuck characters.
    Noop,
    /// Increment/Decrement the the current data cell.
    IncD(i64),
    /// Move the data pointer.
    IncP(i64),
    /// Read one byte from STDIN into the current data cell.
    Read,
    /// Write the ascii value of current data cell's content to STDOUT.
    Write,
    /// Repeat the `Loop` body until the current data cell is `0`.
    Loop(Vec<Instruction>),
}

use Instruction::*;

pub trait SourceRead {
    /// Returns the current buffered `char` as `Some(char)` or `None` if
    /// there are no remaining `char`s.
    fn current(&mut self) -> Option<char>;
    /// Buffers the next `char`.
    fn consume(&mut self);
}

/// Generic implementation for the `SourceRead` trait for all types implementing `std::io::Read`.
///
/// # Examples
///
/// ```
/// use rbf_lib::Instruction::*;
/// use rbf_lib::{Program, SourceReader};
///
/// let mut file = std::fs::File::open("test/example.bf").unwrap();
/// let mut source = SourceReader::new(&mut file);
///
/// let ast = Program::parse(&mut source).unwrap();
/// let expected = Program(vec![IncD(2), Read, Write, IncP(-1), Loop(vec![Read, Loop(vec![IncD(1)])])]);
///
/// assert_eq!(expected, ast);
/// ```
pub struct SourceReader<'a, T: Read + 'a> {
    reader: &'a mut T,
    current: Option<char>,
}

impl<'a, T> SourceReader<'a, T>
    where T: Read + 'a
{
    /// Constructs a new `SourceReader` and consumes the first token.
    pub fn new(r: &'a mut T) -> SourceReader<'a, T> {
        let mut source = SourceReader {
            reader: r,
            current: None,
        };
        source.consume();
        source
    }
}

impl SourceRead for String {
    fn current(&mut self) -> Option<char> {
        self.chars().nth(0)
    }

    fn consume(&mut self) {
        loop {
            self.remove(0);
            match self.chars().nth(0) {
                Some('+') | Some('-') | Some('>') | Some('<') | Some(',') | Some('.') |
                Some('[') | Some(']') | None => break,
                _ => (),
            };
        }
    }
}

impl<'a, T> SourceRead for SourceReader<'a, T>
    where T: Read + 'a
{
    fn current(&mut self) -> Option<char> {
        self.current
    }

    fn consume(&mut self) {
        let mut buf = [0u8];
        let mut current: Option<char>;

        loop {
            current = match self.reader.read(&mut buf) {
                Ok(1) => Some(char::from(buf[0])),
                _ => None,
            };

            let is_code = match current {
                Some('+') | Some('-') | Some('>') | Some('<') | Some(',') | Some('.') |
                Some('[') | Some(']') | None => true,
                _ => false,
            };

            if is_code {
                break;
            }
        }

        self.current = current;
    }
}

impl Program {
    /// Parses brainfuck source code via `SourceRead + Read` and returns either `Ok(Self)`
    /// or Err(String) with an error message.
    ///
    /// # Examples
    ///
    /// ```
    /// use rbf_lib::*;
    /// use rbf_lib::Instruction::*;
    ///
    /// let mut source = "+++.".to_owned();
    ///
    /// assert_eq!(Program(vec![IncD(3), Write]), Program::parse(&mut source).unwrap());
    /// ```
    pub fn parse<T: SourceRead>(source: &mut T) -> Result<Self, String> {
        let prog = Self::parse_program(source, 0)?;
        Ok(Program(prog))
    }

    fn parse_program<T: SourceRead>(source: &mut T,
                                    depth: usize)
                                    -> Result<Vec<Instruction>, String> {
        let mut prog: Vec<Instruction> = vec![];

        loop {
            let c = match source.current() {
                Some(x) => x,
                None => return Ok(prog),
            };

            let instruction = match c {
                '+' | '-' => Ok(Self::parse_incd(source)),
                '>' | '<' => Ok(Self::parse_incp(source)),
                '[' => {
                    source.consume();
                    match Self::parse_program(source, depth + 1) {
                        Ok(p) => {
                            match p.len() {
                                0 => Ok(Noop),
                                _ => Ok(Loop(p)),
                            }
                        }
                        Err(s) => Err(s),
                    }
                }
                ',' => {
                    source.consume();
                    Ok(Read)
                }
                '.' => {
                    source.consume();
                    Ok(Write)
                }
                ']' => {
                    source.consume();
                    match depth {
                        0 => return Err(String::from("Unmatched ]")),
                        _ => return Ok(prog),
                    }
                }
                _ => {
                    source.consume();
                    Ok(Noop)
                }
            };

            match instruction {
                Ok(Noop) => (),
                Ok(ins) => prog.push(ins),
                Err(s) => return Err(s),
            };
        }
    }

    fn parse_incd<T: SourceRead>(source: &mut T) -> Instruction {
        let mut acc = 0i64;
        loop {
            match source.current() {
                Some('+') => acc += 1,
                Some('-') => acc -= 1,
                _ => break,
            }
            source.consume();
        }
        match acc {
            0 => Noop,
            _ => IncD(acc),
        }
    }

    fn parse_incp<T: SourceRead>(source: &mut T) -> Instruction {
        let mut acc = 0i64;
        loop {
            match source.current() {
                Some('>') => acc += 1,
                Some('<') => acc -= 1,
                _ => break,
            }
            source.consume();
        }
        match acc {
            0 => Noop,
            _ => IncP(acc),
        }
    }
}
