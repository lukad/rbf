/// A Vector of [instructions](enum.Instruction.html) representing a Brainfuck program.
pub type Program = Vec<Instruction>;

/// Brainfuck instructions.
#[derive(Clone, Debug, PartialEq)]
pub enum Instruction {
    /// Add to the current data cell.
    Add(i64),
    /// Move the data pointer.
    Move(i64),
    /// Go to the next `0` data cell, moving in specified increments.
    Set(i64),
    /// Set the current data cell to this value.
    Scan(i64),
    /// Read one byte from STDIN into the current data cell.
    Read,
    /// Write the ascii value of the current data cell's content to STDOUT.
    Write,
    /// Repeat the `Loop` body until the current data cell is `0`.
    Loop(Vec<Instruction>),
}
