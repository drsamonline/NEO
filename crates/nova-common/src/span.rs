//! Source code span and position tracking

/// A position in source code (line, column)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl Position {
    pub fn new(line: usize, column: usize, offset: usize) -> Self {
        Self { line, column, offset }
    }

    pub fn zero() -> Self {
        Self { line: 0, column: 0, offset: 0 }
    }
}

impl Default for Position {
    fn default() -> Self {
        Self::zero()
    }
}

/// A span representing a range in source code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub fn at(pos: Position) -> Self {
        Self { start: pos, end: pos }
    }

    pub fn zero() -> Self {
        Self { start: Position::zero(), end: Position::zero() }
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start,
            end: other.end,
        }
    }
}

impl Default for Span {
    fn default() -> Self {
        Self::zero()
    }
}

/// Source file information for span resolution
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub name: String,
    pub content: String,
}

impl SourceFile {
    pub fn new(name: String, content: String) -> Self {
        Self { name, content }
    }

    pub fn get_line(&self, line_num: usize) -> Option<&str> {
        self.content.lines().nth(line_num)
    }
}
