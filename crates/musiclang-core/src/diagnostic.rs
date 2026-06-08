use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub id: SourceId,
    pub name: String,
    pub text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    fn new(id: SourceId, name: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let mut line_starts = vec![0];
        for (index, ch) in text.char_indices() {
            if ch == '\n' {
                line_starts.push(index + ch.len_utf8());
            }
        }
        Self {
            id,
            name: name.into(),
            text,
            line_starts,
        }
    }

    pub fn span(&self, start: usize, end: usize) -> Span {
        let start = start.min(self.text.len());
        let end = end.min(self.text.len()).max(start);
        let (line, column) = self.line_column(start);
        Span {
            source_id: self.id,
            start,
            end,
            line,
            column,
        }
    }

    pub fn line_column(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.text.len());
        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };
        let line_start = self.line_starts[line_index];
        (line_index + 1, offset - line_start + 1)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, name: impl Into<String>, text: impl Into<String>) -> SourceId {
        let id = SourceId(self.files.len());
        self.files.push(SourceFile::new(id, name, text));
        id
    }

    pub fn get(&self, id: SourceId) -> Option<&SourceFile> {
        self.files.get(id.0)
    }

    pub fn span(&self, id: SourceId, start: usize, end: usize) -> Option<Span> {
        self.get(id).map(|file| file.span(start, end))
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub source_id: SourceId,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub const fn point(line: usize, column: usize) -> Self {
        Self {
            source_id: SourceId(0),
            start: 0,
            end: 0,
            line,
            column,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub const fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticLabel {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticRelated {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub span: Option<Span>,
    pub labels: Vec<DiagnosticLabel>,
    pub related: Vec<DiagnosticRelated>,
    pub rule: Option<String>,
    pub style: Option<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        line: usize,
        column: usize,
    ) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            message: message.into(),
            line,
            column,
            span: Some(Span::point(line, column)),
            labels: Vec::new(),
            related: Vec::new(),
            rule: None,
            style: None,
            help: None,
        }
    }

    pub fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        line: usize,
        column: usize,
    ) -> Self {
        let mut diagnostic = Self::error(code, message, line, column);
        diagnostic.severity = Severity::Warning;
        diagnostic
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.line = span.line;
        self.column = span.column;
        self.span = Some(span);
        self
    }

    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(DiagnosticLabel {
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_related(mut self, span: Span, message: impl Into<String>) -> Self {
        self.related.push(DiagnosticRelated {
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_rule(mut self, rule: impl Into<String>) -> Self {
        self.rule = Some(rule.into());
        self
    }

    pub fn with_style(mut self, style: impl Into<String>) -> Self {
        self.style = Some(style.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}[{}]: {}", self.severity, self.code, self.message)?;
        writeln!(f, "  at {}:{}", self.line, self.column)?;
        if let Some(style) = &self.style {
            writeln!(f, "  style: {style}")?;
        }
        if let Some(rule) = &self.rule {
            writeln!(f, "  rule: {rule}")?;
        }
        if let Some(help) = &self.help {
            writeln!(f, "  help: {help}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => f.write_str("error"),
            Self::Warning => f.write_str("warning"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_error_keeps_legacy_location_and_span() {
        let diagnostic = Diagnostic::error("ML_TEST", "message", 3, 5);

        assert_eq!(diagnostic.line, 3);
        assert_eq!(diagnostic.column, 5);
        assert_eq!(diagnostic.span, Some(Span::point(3, 5)));
    }

    #[test]
    fn source_map_registers_files_and_resolves_spans() {
        let mut sources = SourceMap::new();
        let id = sources.add("demo.music", "score demo {\n  note C4, 1/4\n}");
        let start = sources.get(id).unwrap().text.find("note").unwrap();
        let span = sources.span(id, start, start + "note".len()).unwrap();

        assert_eq!(sources.len(), 1);
        assert_eq!(sources.get(id).unwrap().name, "demo.music");
        assert_eq!(span.source_id, id);
        assert_eq!(span.line, 2);
        assert_eq!(span.column, 3);
        assert_eq!(span.start, start);
        assert_eq!(span.end, start + 4);
    }

    #[test]
    fn source_map_columns_use_utf8_byte_offsets() {
        let mut sources = SourceMap::new();
        let id = sources.add("utf8.music", "π\nnote C4");
        let start = "π\n".len();
        let span = sources.span(id, start, start + 4).unwrap();

        assert_eq!(span.line, 2);
        assert_eq!(span.column, 1);
        assert_eq!(sources.get(SourceId(99)), None);
        assert_eq!(sources.span(SourceId(99), 0, 1), None);
    }
}
