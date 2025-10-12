use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SourceId(u32);

impl SourceId {
    pub fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub source: SourceId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(source: SourceId, start: usize, end: usize) -> Self {
        Self {
            source,
            start: start as u32,
            end: end as u32,
        }
    }

    pub fn merge(self, other: Span) -> Span {
        assert_eq!(self.source, other.source);
        Span {
            source: self.source,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}
