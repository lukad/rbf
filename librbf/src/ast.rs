/// A Vector of [`Instruction`](enum.Instruction.html) representing a Brainfuck program.
pub type Program = Vec<Spanned<Instruction>>;

pub type Span = std::ops::Range<usize>;
pub type Spanned<T> = (T, Span);

/// An enum representing all brainfuck instructions.
#[derive(Clone, Debug, PartialEq)]
pub enum Instruction {
    /// Adds to the current data cell.
    Add(i64),
    /// Moves the data pointer.
    Move(i64),
    /// Sets the current data cell to the given value.
    Set(i64),
    /// Multiplies the current cell with the second parameter and adds it to the cell at the
    /// offset given by the first parameter.
    Mul(i64, i64),
    /// Goes to the next `0` data cell, moving in specified increments.
    Scan(i64),
    /// Reads one byte from STDIN into the current data cell.
    Read,
    /// Writes the current data cell's content to STDOUT as ASCII.
    Write,
    /// Repeats the `Loop` body until the current data cell is `0`.
    Loop(Program),
}
