use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct CompileError {
    pub message: String,
    pub file: Option<PathBuf>,
    pub line: usize,
    pub col: usize,
    pub snippet: Option<String>,
}

pub fn locate(src: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(src.len());
    let before = &src[..offset];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    (line, src[line_start..offset].chars().count() + 1)
}

impl CompileError {
    pub fn plain(message: impl Into<String>) -> Self {
        CompileError {
            message: message.into(),
            file: None,
            line: 0,
            col: 0,
            snippet: None,
        }
    }

    pub fn at(file: &Path, src: &str, offset: usize, message: impl Into<String>) -> Self {
        let offset = offset.min(src.len());
        let before = &src[..offset];
        let (line, col) = locate(src, offset);
        let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line_end = src[line_start..]
            .find('\n')
            .map(|i| line_start + i)
            .unwrap_or(src.len());
        CompileError {
            message: message.into(),
            file: Some(file.to_path_buf()),
            line,
            col,
            snippet: Some(src[line_start..line_end].trim_end().to_string()),
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.file {
            Some(p) => writeln!(f, "error: {}\n  --> {}:{}:{}", self.message, p.display(), self.line, self.col)?,
            None => writeln!(f, "error: {}", self.message)?,
        }
        if let Some(snippet) = &self.snippet {
            let gutter = format!("{}", self.line);
            let pad = " ".repeat(gutter.len());
            writeln!(f, "{} |", pad)?;
            writeln!(f, "{} | {}", gutter, snippet)?;
            writeln!(f, "{} | {}^", pad, " ".repeat(self.col.saturating_sub(1)))?;
        }
        Ok(())
    }
}

impl std::error::Error for CompileError {}

pub type Result<T> = std::result::Result<T, CompileError>;
