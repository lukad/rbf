use std::io::Read;
use std::fs::File;

#[derive(Debug)]
struct Program(Vec<Instruction>);

#[derive(Debug)]
enum Instruction {
    Noop,
    IncD(i64),
    IncP(i64),
    Read,
    Write,
    Loop(Vec<Instruction>),
}

use Instruction::*;

trait SourceRead {
    fn current(&self) -> Option<char>;
    fn consume(&mut self) -> Option<char>;
}

struct SourceReader<'a, T: Read + 'a> {
    reader: &'a mut T,
    current: Option<char>,
}

impl<'a, T> SourceReader<'a, T>
    where T: Read + 'a
{
    fn new(r: &'a mut T) -> SourceReader<'a, T> {
        SourceReader {
            reader: r,
            current: None,
        }
    }
}

impl<'a, T> SourceRead for SourceReader<'a, T>
    where T: Read + 'a
{
    fn current(&self) -> Option<char> {
        self.current
    }

    fn consume(&mut self) -> Option<char> {
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
        current
    }
}

impl Program {
    pub fn parse<T: SourceRead>(source: &mut T) -> Result<Self, String> {
        source.consume();
        let prog = Self::parse_program(source, 0)?;
        Ok(Program(prog))
    }

    fn parse_program<T: SourceRead>(source: &mut T, depth: usize) -> Result<Vec<Instruction>, String> {
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
                },
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

fn main() {
    let mut file = File::open("foo.bf").unwrap();
    let mut reader = SourceReader::new(&mut file);

    println!("{:?}", Program::parse(&mut reader).unwrap().0);
}
