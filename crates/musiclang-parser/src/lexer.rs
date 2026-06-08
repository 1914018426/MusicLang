use musiclang_core::{Diagnostic, SourceId, Span};

use crate::token::{classify_word, is_word_continue, is_word_start, Token, TokenKind};

pub(crate) struct Lexer {
    source_id: SourceId,
    chars: Vec<char>,
    index: usize,
    byte_index: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    pub(crate) fn new(source: &str) -> Self {
        Self::with_source_id(SourceId(0), source)
    }

    pub(crate) fn with_source_id(source_id: SourceId, source: &str) -> Self {
        Self {
            source_id,
            chars: source.chars().collect(),
            index: 0,
            byte_index: 0,
            line: 1,
            column: 1,
        }
    }

    pub(crate) fn tokenize(mut self) -> Result<Vec<Token>, Vec<Diagnostic>> {
        let mut tokens = Vec::new();
        let mut diagnostics = Vec::new();
        while let Some(ch) = self.peek() {
            match ch {
                c if c.is_whitespace() => self.advance_whitespace(),
                '/' if self.peek_next() == Some('/') => self.advance_comment(),
                '{' => tokens.push(self.simple(TokenKind::LBrace)),
                '}' => tokens.push(self.simple(TokenKind::RBrace)),
                '[' => tokens.push(self.simple(TokenKind::LBracket)),
                ']' => tokens.push(self.simple(TokenKind::RBracket)),
                '(' => tokens.push(self.simple(TokenKind::LParen)),
                ')' => tokens.push(self.simple(TokenKind::RParen)),
                ',' => tokens.push(self.simple(TokenKind::Comma)),
                ':' => tokens.push(self.simple(TokenKind::Colon)),
                '+' => tokens.push(self.simple(TokenKind::Plus)),
                '-' => tokens.push(self.simple(TokenKind::Minus)),
                '*' => tokens.push(self.simple(TokenKind::Star)),
                '/' => tokens.push(self.simple(TokenKind::Slash)),
                '|' if self.peek_next() == Some('>') => tokens.push(self.double(TokenKind::Pipe)),
                '=' if self.peek_next() == Some('=') => tokens.push(self.double(TokenKind::EqEq)),
                '!' if self.peek_next() == Some('=') => tokens.push(self.double(TokenKind::NotEq)),
                '<' if self.peek_next() == Some('=') => tokens.push(self.double(TokenKind::LtEq)),
                '>' if self.peek_next() == Some('=') => tokens.push(self.double(TokenKind::GtEq)),
                '<' => tokens.push(self.simple(TokenKind::Lt)),
                '>' => tokens.push(self.simple(TokenKind::Gt)),
                '=' => tokens.push(self.simple(TokenKind::Eq)),
                '.' if self.peek_next() == Some('.') => tokens.push(self.double(TokenKind::DotDot)),
                '.' => tokens.push(self.simple(TokenKind::Dot)),
                '"' => {
                    if let Some(token) = self.string(&mut diagnostics) {
                        tokens.push(token);
                    }
                }
                c if is_word_start(c) || c.is_ascii_digit() => tokens.push(self.word()),
                _ => {
                    let token = self.simple(TokenKind::Ident);
                    diagnostics.push(
                        Diagnostic::error(
                            "ML_LEX_TOKEN",
                            format!("unexpected character `{ch}`"),
                            token.span.line,
                            token.span.column,
                        )
                        .with_span(token.span),
                    );
                }
            }
        }
        tokens.push(Token {
            kind: TokenKind::Eof,
            text: String::new(),
            span: self.span_from(self.line, self.column, self.byte_index),
        });
        if diagnostics.is_empty() {
            Ok(tokens)
        } else {
            Err(diagnostics)
        }
    }

    fn simple(&mut self, kind: TokenKind) -> Token {
        let line = self.line;
        let column = self.column;
        let start = self.byte_index;
        let text = self.advance().unwrap().to_string();
        let span = self.span(line, column, start, self.byte_index);
        Token { kind, text, span }
    }

    fn double(&mut self, kind: TokenKind) -> Token {
        let line = self.line;
        let column = self.column;
        let start = self.byte_index;
        let first = self.advance().unwrap();
        let second = self.advance().unwrap();
        Token {
            kind,
            text: format!("{first}{second}"),
            span: self.span(line, column, start, self.byte_index),
        }
    }

    fn string(&mut self, diagnostics: &mut Vec<Diagnostic>) -> Option<Token> {
        let line = self.line;
        let column = self.column;
        let start = self.byte_index;
        self.advance();
        let mut text = String::new();
        while let Some(ch) = self.peek() {
            if ch == '"' {
                self.advance();
                return Some(Token {
                    kind: TokenKind::String,
                    text,
                    span: self.span(line, column, start, self.byte_index),
                });
            }
            if ch == '\\' {
                self.advance();
                let Some(escaped) = self.peek() else {
                    break;
                };
                text.push(match escaped {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '"' => '"',
                    '\\' => '\\',
                    other => other,
                });
                self.advance();
            } else {
                text.push(ch);
                self.advance();
            }
        }
        diagnostics.push(
            Diagnostic::error("ML_LEX_STRING", "unterminated string literal", line, column)
                .with_span(self.span(line, column, start, self.byte_index)),
        );
        None
    }

    fn word(&mut self) -> Token {
        let line = self.line;
        let column = self.column;
        let start = self.byte_index;
        let mut text = String::new();
        while let Some(ch) = self.peek() {
            if is_word_continue(ch) {
                text.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        let kind = classify_word(&text);
        let span = self.span(line, column, start, self.byte_index);
        Token { kind, text, span }
    }

    fn advance_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.advance();
        }
    }

    fn advance_comment(&mut self) {
        while let Some(ch) = self.peek() {
            self.advance();
            if ch == '\n' {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.index + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.index += 1;
        self.byte_index += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn span_from(&self, line: usize, column: usize, start: usize) -> Span {
        self.span(line, column, start, start)
    }

    fn span(&self, line: usize, column: usize, start: usize, end: usize) -> Span {
        Span {
            source_id: self.source_id,
            start,
            end,
            line,
            column,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_comments_and_eof() {
        let tokens = Lexer::new("note C4, 1/4 // lead\n").tokenize().unwrap();

        assert_eq!(tokens.last().unwrap().kind, TokenKind::Eof);
        assert!(tokens.iter().any(|token| token.text == "note"));
        assert!(!tokens.iter().any(|token| token.text == "//"));
    }

    #[test]
    fn tokenizes_string_escapes() {
        let tokens = Lexer::new("title \"A \\\"quote\\\"\\nline\"")
            .tokenize()
            .unwrap();
        let string = tokens
            .iter()
            .find(|token| token.kind == TokenKind::String)
            .unwrap();

        assert_eq!(string.text, "A \"quote\"\nline");
    }
}
