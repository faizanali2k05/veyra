use serde::{Deserialize, Serialize};
use std::fmt::{self, Write};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        Self::new(self.start.min(other.start), self.end.max(other.end))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    name: String,
    text: String,
}

impl Source {
    #[must_use]
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn location(&self, offset: usize) -> (usize, usize) {
        let safe = offset.min(self.text.len());
        let prefix = &self.text[..floor_char_boundary(&self.text, safe)];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix
            .rsplit('\n')
            .next()
            .map_or(1, |part| part.chars().count() + 1);
        (line, column)
    }

    #[must_use]
    pub fn render(&self, message: &str, span: Span) -> String {
        let start = floor_char_boundary(&self.text, span.start.min(self.text.len()));
        let end = ceil_char_boundary(&self.text, span.end.min(self.text.len()));
        let (line, column) = self.location(start);
        let line_start = self.text[..start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = self.text[start..]
            .find('\n')
            .map_or(self.text.len(), |i| start + i);
        let excerpt = &self.text[line_start..line_end];
        let underline_start = self.text[line_start..start].chars().count();
        let underline_len = self.text[start..end.min(line_end)].chars().count().max(1);
        let gutter = line.to_string().len();
        let mut output = String::new();
        let _ = writeln!(output, "error: {message}");
        let _ = writeln!(output, " --> {}:{line}:{column}", self.name);
        let _ = writeln!(output, "{} |", " ".repeat(gutter));
        let _ = writeln!(output, "{line} | {excerpt}");
        let _ = write!(
            output,
            "{} | {}{}",
            " ".repeat(gutter),
            " ".repeat(underline_start),
            "^".repeat(underline_len)
        );
        output
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn ceil_char_boundary(text: &str, mut index: usize) -> usize {
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_columns_count_characters() {
        let source = Source::new("test.vy", "é + @");
        assert_eq!(source.location(5), (1, 5));
        assert!(
            source
                .render("bad token", Span::new(5, 6))
                .contains("test.vy:1:5")
        );
    }
}
