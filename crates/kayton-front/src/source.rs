use crate::span::SourceId;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: SourceId,
    pub path: PathBuf,
    pub text: String,
    pub line_offsets: Vec<usize>,
}

impl SourceFile {
    pub fn new(id: SourceId, path: PathBuf, text: String) -> Self {
        let line_offsets = compute_line_offsets(&text);
        Self {
            id,
            path,
            text,
            line_offsets,
        }
    }

    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        match self
            .line_offsets
            .binary_search_by(|probe| probe.cmp(&offset))
        {
            Ok(idx) => (idx + 1, 1),
            Err(idx) => {
                let line = idx;
                let col = offset - self.line_offsets[idx - 1] + 1;
                (line, col)
            }
        }
    }
}

fn compute_line_offsets(text: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (idx, ch) in text.char_indices() {
        if ch == '\n' {
            offsets.push(idx + 1);
        }
    }
    offsets
}

#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    sources: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    pub fn add_source(&mut self, path: PathBuf, text: String) -> SourceId {
        let id = SourceId::new(self.sources.len() as u32 + 1);
        let file = SourceFile::new(id, path, text);
        self.sources.push(file);
        id
    }

    pub fn get(&self, id: SourceId) -> Option<&SourceFile> {
        self.sources.iter().find(|file| file.id == id)
    }
}
