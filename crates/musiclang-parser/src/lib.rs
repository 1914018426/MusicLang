use musiclang_core::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub style: Option<StyleDecl>,
    pub styles: Vec<StyleDecl>,
    pub functions: Vec<FunctionDecl>,
    pub score: ScoreDecl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleDecl {
    pub name: String,
    pub parent: Option<String>,
    pub entries: Vec<StyleEntry>,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDecl {
    pub name: String,
    pub statements: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreDecl {
    pub name: String,
    pub style: Option<String>,
    pub metadata: Vec<ScoreMeta>,
    pub statements: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScoreMeta {
    Tempo(TempoDecl),
    Meter(MeterDecl),
    Key(KeyDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Voice(VoiceDecl),
    Note(NoteStmt),
    Chord(ChordStmt),
    Dynamic(DynamicStmt),
    Velocity(VelocityStmt),
    Articulation(ArticulationStmt),
    Section(SectionStmt),
    NonChordTone(NonChordToneStmt),
    TuningSystem(TuningSystemStmt),
    WorldTradition(WorldTraditionStmt),
    HistoricalEra(HistoricalEraStmt),
    For(ForStmt),
    If(IfStmt),
    Let(LetStmt),
    Call(CallStmt),
    Override(OverrideStmt),
    WithStyle(WithStyleStmt),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Ident(String),
    Int(i32),
    Bool(bool),
    PitchLiteral(String),
    IntervalLiteral(String),
    DurationLiteral(String),
    StringLiteral(String),
    List(Vec<Expr>),
    Call {
        callee: String,
        args: Vec<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Eq,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceDecl {
    pub name: String,
    pub program: Option<u8>,
    pub statements: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempoDecl {
    pub bpm: u16,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeterDecl {
    pub numerator: u8,
    pub denominator: u8,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyDecl {
    pub tonic: String,
    pub mode: String,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteStmt {
    pub pitch: String,
    pub duration: String,
    pub pitch_expr: Expr,
    pub duration_expr: Expr,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChordStmt {
    pub pitches: Vec<String>,
    pub duration: String,
    pub pitch_exprs: Vec<Expr>,
    pub duration_expr: Expr,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicStmt {
    pub mark: String,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VelocityStmt {
    pub velocity: u8,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArticulationStmt {
    pub mark: String,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionStmt {
    pub label: String,
    pub statements: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonChordToneStmt {
    pub kind: String,
    pub statements: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuningSystemStmt {
    pub kind: String,
    pub statements: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldTraditionStmt {
    pub kind: String,
    pub statements: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalEraStmt {
    pub kind: String,
    pub statements: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForStmt {
    pub variable: String,
    pub start: i32,
    pub end: i32,
    pub statements: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfStmt {
    pub left: String,
    pub equals: i32,
    pub condition: Expr,
    pub statements: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetStmt {
    pub name: String,
    pub value: String,
    pub value_expr: Expr,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallStmt {
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideStmt {
    pub rule: String,
    pub reason: Option<String>,
    pub statements: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithStyleStmt {
    pub style: String,
    pub statements: Vec<Stmt>,
    pub line: usize,
    pub column: usize,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    kind: TokenKind,
    text: String,
    span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Ident,
    Number,
    Pitch,
    Interval,
    Duration,
    String,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LParen,
    RParen,
    Comma,
    Colon,
    Eq,
    EqEq,
    DotDot,
    Plus,
    Minus,
    Eof,
}

pub fn parse_source(source: &str) -> Result<Program, Vec<Diagnostic>> {
    let tokens = Lexer::new(source).tokenize()?;
    Parser::new(tokens).parse_program()
}

pub fn tokenize_source(source: &str) -> Result<Vec<Token>, Vec<Diagnostic>> {
    Lexer::new(source).tokenize()
}

struct Lexer {
    chars: Vec<char>,
    index: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            index: 0,
            line: 1,
            column: 1,
        }
    }

    fn tokenize(mut self) -> Result<Vec<Token>, Vec<Diagnostic>> {
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
                '=' if self.peek_next() == Some('=') => tokens.push(self.double(TokenKind::EqEq)),
                '=' => tokens.push(self.simple(TokenKind::Eq)),
                '.' if self.peek_next() == Some('.') => tokens.push(self.double(TokenKind::DotDot)),
                '"' => {
                    if let Some(token) = self.string(&mut diagnostics) {
                        tokens.push(token);
                    }
                }
                c if is_word_start(c) || c.is_ascii_digit() => tokens.push(self.word()),
                _ => {
                    diagnostics.push(Diagnostic::error(
                        "ML_LEX_TOKEN",
                        format!("unexpected character `{ch}`"),
                        self.line,
                        self.column,
                    ));
                    self.advance();
                }
            }
        }
        tokens.push(Token {
            kind: TokenKind::Eof,
            text: String::new(),
            span: Span::point(self.line, self.column),
        });
        if diagnostics.is_empty() {
            Ok(tokens)
        } else {
            Err(diagnostics)
        }
    }

    fn simple(&mut self, kind: TokenKind) -> Token {
        let span = Span::point(self.line, self.column);
        let text = self.advance().unwrap().to_string();
        Token { kind, text, span }
    }

    fn double(&mut self, kind: TokenKind) -> Token {
        let span = Span::point(self.line, self.column);
        let first = self.advance().unwrap();
        let second = self.advance().unwrap();
        Token {
            kind,
            text: format!("{first}{second}"),
            span,
        }
    }

    fn string(&mut self, diagnostics: &mut Vec<Diagnostic>) -> Option<Token> {
        let span = Span::point(self.line, self.column);
        self.advance();
        let mut text = String::new();
        while let Some(ch) = self.peek() {
            if ch == '"' {
                self.advance();
                return Some(Token {
                    kind: TokenKind::String,
                    text,
                    span,
                });
            }
            text.push(ch);
            self.advance();
        }
        diagnostics.push(Diagnostic::error(
            "ML_LEX_STRING",
            "unterminated string literal",
            span.line,
            span.column,
        ));
        None
    }

    fn word(&mut self) -> Token {
        let span = Span::point(self.line, self.column);
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
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            index: 0,
            diagnostics: Vec::new(),
        }
    }

    fn parse_program(mut self) -> Result<Program, Vec<Diagnostic>> {
        let mut styles = Vec::new();
        let mut functions = Vec::new();
        while self.check_ident("style") || self.check_ident("fn") {
            if self.check_ident("style") {
                if let Some(style) = self.parse_style() {
                    styles.push(style);
                }
            } else if let Some(function) = self.parse_function() {
                functions.push(function);
            }
        }
        let style = styles.first().cloned();

        let score = self.parse_score();
        if !self.check(TokenKind::Eof) {
            let token = self.peek().clone();
            self.diagnostics.push(Diagnostic::error(
                "ML_PARSE_TRAILING",
                "unexpected input after score",
                token.span.line,
                token.span.column,
            ));
        }
        if !self.diagnostics.is_empty() {
            return Err(self.diagnostics);
        }
        score
            .map(|score| Program {
                style,
                styles,
                functions,
                score,
            })
            .ok_or(self.diagnostics)
    }

    fn parse_style(&mut self) -> Option<StyleDecl> {
        let start = self.expect_ident_text("style")?;
        let name = self.expect_name()?;
        let parent = if self.check_ident("extends") {
            self.advance();
            Some(self.expect_name()?)
        } else {
            None
        };
        let mut entries = Vec::new();
        if self.consume(TokenKind::LBrace).is_some() {
            while !self.check(TokenKind::RBrace) && !self.check(TokenKind::Eof) {
                let key = self.expect_name()?;
                self.expect(TokenKind::Colon, "expected `:` in style entry")?;
                let mut value = Vec::new();
                while !self.check(TokenKind::RBrace)
                    && !self.check(TokenKind::Eof)
                    && !self.current_starts_style_entry()
                {
                    value.push(self.advance().text.clone());
                }
                if value.is_empty() {
                    self.diagnostics.push(Diagnostic::error(
                        "ML_PARSE_STYLE_ENTRY",
                        "expected style entry value",
                        self.peek().span.line,
                        self.peek().span.column,
                    ));
                } else {
                    entries.push(StyleEntry {
                        key,
                        value: value.join(" "),
                    });
                }
            }
            self.expect(TokenKind::RBrace, "expected `}` to close style block")?;
        }
        Some(StyleDecl {
            name,
            parent,
            entries,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_function(&mut self) -> Option<FunctionDecl> {
        let start = self.expect_ident_text("fn")?;
        let name = self.expect_name()?;
        let statements = self.parse_required_block()?;
        Some(FunctionDecl {
            name,
            statements,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_score(&mut self) -> Option<ScoreDecl> {
        let start = self.expect_ident_text("score")?;
        let name = self.expect_name()?;
        let style = if self.check_ident("style") {
            self.advance();
            Some(self.expect_name()?)
        } else {
            None
        };
        self.expect(TokenKind::LBrace, "expected `{` to start score block")?;
        let mut metadata = Vec::new();
        let mut statements = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.check(TokenKind::Eof) {
            if self.check_ident("tempo") {
                metadata.push(ScoreMeta::Tempo(self.parse_tempo()?));
            } else if self.check_ident("meter") {
                metadata.push(ScoreMeta::Meter(self.parse_meter()?));
            } else if self.check_ident("key") {
                metadata.push(ScoreMeta::Key(self.parse_key()?));
            } else if let Some(stmt) = self.parse_stmt() {
                statements.push(stmt);
            } else {
                self.advance();
            }
        }
        self.expect(TokenKind::RBrace, "expected `}` to close score block")?;
        Some(ScoreDecl {
            name,
            style,
            metadata,
            statements,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_required_block(&mut self) -> Option<Vec<Stmt>> {
        self.expect(TokenKind::LBrace, "expected `{` to start block")?;
        let mut statements = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.check(TokenKind::Eof) {
            if let Some(stmt) = self.parse_stmt() {
                statements.push(stmt);
            } else {
                self.advance();
            }
        }
        self.expect(TokenKind::RBrace, "expected `}` to close block")?;
        Some(statements)
    }

    fn parse_stmt(&mut self) -> Option<Stmt> {
        if self.check_ident("voice") {
            return self.parse_voice().map(Stmt::Voice);
        }
        if self.check_ident("note") {
            return self.parse_note().map(Stmt::Note);
        }
        if self.check_ident("chord") {
            return self.parse_chord().map(Stmt::Chord);
        }
        if self.check_ident("dynamic") {
            return self.parse_dynamic().map(Stmt::Dynamic);
        }
        if self.check_ident("velocity") {
            return self.parse_velocity().map(Stmt::Velocity);
        }
        if self.check_ident("articulation") {
            return self.parse_articulation().map(Stmt::Articulation);
        }
        if self.check_ident("section") {
            return self.parse_section().map(Stmt::Section);
        }
        if self.check_ident("non_chord_tone") {
            return self.parse_non_chord_tone().map(Stmt::NonChordTone);
        }
        if self.check_ident("tuning_system") {
            return self.parse_tuning_system().map(Stmt::TuningSystem);
        }
        if self.check_ident("world_tradition") {
            return self.parse_world_tradition().map(Stmt::WorldTradition);
        }
        if self.check_ident("historical_era") {
            return self.parse_historical_era().map(Stmt::HistoricalEra);
        }
        if self.check_ident("for") {
            return self.parse_for().map(Stmt::For);
        }
        if self.check_ident("if") {
            return self.parse_if().map(Stmt::If);
        }
        if self.check_ident("let") {
            return self.parse_let().map(Stmt::Let);
        }
        if self.check_ident("call") {
            return self.parse_call().map(Stmt::Call);
        }
        if self.check_ident("override") {
            return self.parse_override().map(Stmt::Override);
        }
        if self.check_ident("with") {
            return self.parse_with_style().map(Stmt::WithStyle);
        }
        let token = self.peek().clone();
        self.diagnostics.push(Diagnostic::error(
            "ML_PARSE_STMT",
            format!("unrecognized statement `{}`", token.text),
            token.span.line,
            token.span.column,
        ));
        None
    }

    fn parse_voice(&mut self) -> Option<VoiceDecl> {
        let start = self.expect_ident_text("voice")?;
        let name = self.expect_name()?;
        self.expect(TokenKind::LBrace, "expected `{` to start voice block")?;
        let mut program = None;
        let mut statements = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.check(TokenKind::Eof) {
            if self.check_ident("program") || self.check_ident("instrument") {
                program = Some(self.parse_program_number()?);
            } else if let Some(stmt) = self.parse_stmt() {
                statements.push(stmt);
            } else {
                self.advance();
            }
        }
        self.expect(TokenKind::RBrace, "expected `}` to close voice block")?;
        Some(VoiceDecl {
            name,
            program,
            statements,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_tempo(&mut self) -> Option<TempoDecl> {
        let start = self.expect_ident_text("tempo")?;
        let bpm = self.expect_number()?;
        Some(TempoDecl {
            bpm: bpm.max(1) as u16,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_meter(&mut self) -> Option<MeterDecl> {
        let start = self.expect_ident_text("meter")?;
        let value = self.expect_duration_literal()?;
        let (numerator, denominator) = value.split_once('/')?;
        Some(MeterDecl {
            numerator: numerator.parse().ok()?,
            denominator: denominator.parse().ok()?,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_key(&mut self) -> Option<KeyDecl> {
        let start = self.expect_ident_text("key")?;
        let tonic = self.expect_name()?;
        let mode = if self.current_starts_statement()
            || self.check(TokenKind::RBrace)
            || self.check(TokenKind::Eof)
        {
            "major".to_string()
        } else {
            self.expect_name()?
        };
        Some(KeyDecl {
            tonic,
            mode,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_program_number(&mut self) -> Option<u8> {
        self.advance();
        let program = self.expect_number()?;
        Some(program.clamp(0, 127) as u8)
    }

    fn parse_note(&mut self) -> Option<NoteStmt> {
        let start = self.expect_ident_text("note")?;
        let pitch_expr = self.parse_expr_until(&[TokenKind::Comma])?;
        self.expect(TokenKind::Comma, "expected `,` after note pitch")?;
        let duration_expr = self.parse_expr_until_stmt_end()?;
        Some(NoteStmt {
            pitch: expr_to_source(&pitch_expr),
            duration: expr_to_source(&duration_expr),
            pitch_expr,
            duration_expr,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_chord(&mut self) -> Option<ChordStmt> {
        let start = self.expect_ident_text("chord")?;
        self.expect(TokenKind::LBracket, "expected `[` after chord")?;
        let mut pitch_exprs = Vec::new();
        while !self.check(TokenKind::RBracket) && !self.check(TokenKind::Eof) {
            pitch_exprs.push(self.parse_expr_until(&[TokenKind::Comma, TokenKind::RBracket])?);
            if self.check(TokenKind::Comma) {
                self.advance();
            }
        }
        self.expect(TokenKind::RBracket, "expected `]` after chord pitches")?;
        self.expect(TokenKind::Comma, "expected `,` after chord pitches")?;
        let duration_expr = self.parse_expr_until_stmt_end()?;
        Some(ChordStmt {
            pitches: pitch_exprs.iter().map(expr_to_source).collect(),
            duration: expr_to_source(&duration_expr),
            pitch_exprs,
            duration_expr,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_dynamic(&mut self) -> Option<DynamicStmt> {
        let start = self.expect_ident_text("dynamic")?;
        let mark = self.expect_name()?;
        Some(DynamicStmt {
            mark,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_velocity(&mut self) -> Option<VelocityStmt> {
        let start = self.expect_ident_text("velocity")?;
        let velocity = self.expect_number()?.clamp(0, 127) as u8;
        Some(VelocityStmt {
            velocity,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_articulation(&mut self) -> Option<ArticulationStmt> {
        let start = self.expect_ident_text("articulation")?;
        let mark = self.expect_name()?;
        Some(ArticulationStmt {
            mark,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_section(&mut self) -> Option<SectionStmt> {
        let start = self.expect_ident_text("section")?;
        let label = self.expect_name()?;
        let statements = self.parse_required_block()?;
        Some(SectionStmt {
            label,
            statements,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_non_chord_tone(&mut self) -> Option<NonChordToneStmt> {
        let start = self.expect_ident_text("non_chord_tone")?;
        let kind = self.expect_name()?;
        let statements = self.parse_required_block()?;
        Some(NonChordToneStmt {
            kind,
            statements,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_tuning_system(&mut self) -> Option<TuningSystemStmt> {
        let start = self.expect_ident_text("tuning_system")?;
        let kind = self.expect_name()?;
        let statements = self.parse_required_block()?;
        Some(TuningSystemStmt {
            kind,
            statements,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_world_tradition(&mut self) -> Option<WorldTraditionStmt> {
        let start = self.expect_ident_text("world_tradition")?;
        let kind = self.expect_name()?;
        let statements = self.parse_required_block()?;
        Some(WorldTraditionStmt {
            kind,
            statements,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_historical_era(&mut self) -> Option<HistoricalEraStmt> {
        let start = self.expect_ident_text("historical_era")?;
        let kind = self.expect_name()?;
        let statements = self.parse_required_block()?;
        Some(HistoricalEraStmt {
            kind,
            statements,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_for(&mut self) -> Option<ForStmt> {
        let start = self.expect_ident_text("for")?;
        let variable = self.expect_name()?;
        self.expect_ident_text("in")?;
        let range_start = self.expect_number()?;
        self.expect(TokenKind::DotDot, "expected `..` in for range")?;
        let range_end = self.expect_number()?;
        let statements = self.parse_required_block()?;
        Some(ForStmt {
            variable,
            start: range_start,
            end: range_end,
            statements,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_if(&mut self) -> Option<IfStmt> {
        let start = self.expect_ident_text("if")?;
        let condition = self.parse_expr_until(&[TokenKind::LBrace])?;
        let (left, equals) = match &condition {
            Expr::Binary {
                op: BinaryOp::Eq,
                left,
                right,
            } => match (&**left, &**right) {
                (Expr::Ident(name), Expr::Int(value)) => (name.clone(), *value),
                _ => (expr_to_source(&condition), 0),
            },
            _ => (expr_to_source(&condition), 0),
        };
        let statements = self.parse_required_block()?;
        Some(IfStmt {
            left,
            equals,
            condition,
            statements,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_let(&mut self) -> Option<LetStmt> {
        let start = self.expect_ident_text("let")?;
        let name = self.expect_name()?;
        self.expect(TokenKind::Eq, "expected `=` in let statement")?;
        let value_expr = self.parse_expr_until_stmt_end()?;
        Some(LetStmt {
            name,
            value: expr_to_source(&value_expr),
            value_expr,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_call(&mut self) -> Option<CallStmt> {
        let start = self.expect_ident_text("call")?;
        let name = self.expect_name()?;
        Some(CallStmt {
            name,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_override(&mut self) -> Option<OverrideStmt> {
        let start = self.expect_ident_text("override")?;
        let rule = self.expect_name()?;
        self.expect_ident_text("allow")?;
        let reason = if self.check_ident("reason") {
            self.advance();
            Some(self.expect_string()?)
        } else {
            None
        };
        let statements = self.parse_required_block()?;
        Some(OverrideStmt {
            rule,
            reason,
            statements,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_with_style(&mut self) -> Option<WithStyleStmt> {
        let start = self.expect_ident_text("with")?;
        self.expect_ident_text("style")?;
        let style = self.expect_name()?;
        let statements = self.parse_required_block()?;
        Some(WithStyleStmt {
            style,
            statements,
            line: start.span.line,
            column: start.span.column,
            span: start.span,
        })
    }

    fn parse_expr_until(&mut self, stops: &[TokenKind]) -> Option<Expr> {
        let mut tokens = Vec::new();
        let mut depth = 0usize;
        while !self.check(TokenKind::Eof) {
            if depth == 0 && stops.iter().any(|kind| self.check(kind.clone())) {
                break;
            }
            let token = self.advance().clone();
            match token.kind {
                TokenKind::LBracket | TokenKind::LParen => depth += 1,
                TokenKind::RBracket | TokenKind::RParen => depth = depth.saturating_sub(1),
                _ => {}
            }
            tokens.push(token);
        }
        parse_expr_tokens(&tokens).or_else(|| {
            if let Some(token) = tokens.first() {
                self.diagnostics.push(Diagnostic::error(
                    "ML_PARSE_EXPR",
                    "expected expression",
                    token.span.line,
                    token.span.column,
                ));
            }
            None
        })
    }

    fn parse_expr_until_stmt_end(&mut self) -> Option<Expr> {
        let mut tokens = Vec::new();
        let mut depth = 0usize;
        while !self.check(TokenKind::Eof) {
            if depth == 0 && (self.check(TokenKind::RBrace) || self.current_starts_statement()) {
                break;
            }
            let token = self.advance().clone();
            match token.kind {
                TokenKind::LBracket | TokenKind::LParen => depth += 1,
                TokenKind::RBracket | TokenKind::RParen => depth = depth.saturating_sub(1),
                _ => {}
            }
            tokens.push(token);
        }
        parse_expr_tokens(&tokens).or_else(|| {
            if let Some(token) = tokens.first() {
                self.diagnostics.push(Diagnostic::error(
                    "ML_PARSE_EXPR",
                    "expected expression",
                    token.span.line,
                    token.span.column,
                ));
            }
            None
        })
    }

    fn current_starts_statement(&self) -> bool {
        matches!(
            self.peek().text.as_str(),
            "voice"
                | "note"
                | "chord"
                | "dynamic"
                | "velocity"
                | "articulation"
                | "section"
                | "non_chord_tone"
                | "tuning_system"
                | "world_tradition"
                | "historical_era"
                | "for"
                | "if"
                | "let"
                | "call"
                | "override"
                | "with"
                | "tempo"
                | "meter"
                | "program"
                | "instrument"
        )
    }

    fn current_starts_style_entry(&self) -> bool {
        self.peek().kind == TokenKind::Ident
            && self
                .tokens
                .get(self.index + 1)
                .is_some_and(|token| token.kind == TokenKind::Colon)
    }

    fn expect_name(&mut self) -> Option<String> {
        let token = self.peek().clone();
        if matches!(
            token.kind,
            TokenKind::Ident | TokenKind::Pitch | TokenKind::Interval
        ) {
            self.advance();
            Some(token.text)
        } else {
            self.diagnostics.push(Diagnostic::error(
                "ML_PARSE_NAME",
                "expected name",
                token.span.line,
                token.span.column,
            ));
            None
        }
    }

    fn expect_number(&mut self) -> Option<i32> {
        let token = self.peek().clone();
        if token.kind == TokenKind::Number {
            self.advance();
            token.text.parse().ok()
        } else {
            self.diagnostics.push(Diagnostic::error(
                "ML_PARSE_NUMBER",
                "expected number",
                token.span.line,
                token.span.column,
            ));
            None
        }
    }

    fn expect_duration_literal(&mut self) -> Option<String> {
        let token = self.peek().clone();
        if token.kind == TokenKind::Duration {
            self.advance();
            Some(token.text)
        } else {
            self.diagnostics.push(Diagnostic::error(
                "ML_PARSE_DURATION",
                "expected duration literal",
                token.span.line,
                token.span.column,
            ));
            None
        }
    }

    fn expect_string(&mut self) -> Option<String> {
        let token = self.peek().clone();
        if token.kind == TokenKind::String {
            self.advance();
            Some(token.text)
        } else {
            self.diagnostics.push(Diagnostic::error(
                "ML_PARSE_STRING",
                "expected string",
                token.span.line,
                token.span.column,
            ));
            None
        }
    }

    fn expect_ident_text(&mut self, text: &str) -> Option<Token> {
        let token = self.peek().clone();
        if token.kind == TokenKind::Ident && token.text == text {
            self.advance();
            Some(token)
        } else {
            self.diagnostics.push(Diagnostic::error(
                "ML_PARSE_KEYWORD",
                format!("expected `{text}`"),
                token.span.line,
                token.span.column,
            ));
            None
        }
    }

    fn expect(&mut self, kind: TokenKind, message: &str) -> Option<Token> {
        let token = self.peek().clone();
        if token.kind == kind {
            self.advance();
            Some(token)
        } else {
            self.diagnostics.push(Diagnostic::error(
                "ML_PARSE_TOKEN",
                message,
                token.span.line,
                token.span.column,
            ));
            None
        }
    }

    fn consume(&mut self, kind: TokenKind) -> Option<Token> {
        if self.check(kind) {
            Some(self.advance().clone())
        } else {
            None
        }
    }

    fn check_ident(&self, text: &str) -> bool {
        self.peek().kind == TokenKind::Ident && self.peek().text == text
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.index]
    }

    fn advance(&mut self) -> &Token {
        let index = self.index;
        if self.index + 1 < self.tokens.len() {
            self.index += 1;
        }
        &self.tokens[index]
    }
}

fn parse_expr_tokens(tokens: &[Token]) -> Option<Expr> {
    if tokens.is_empty() {
        return None;
    }
    if tokens.first()?.kind == TokenKind::LBracket && tokens.last()?.kind == TokenKind::RBracket {
        return split_expr_list(&tokens[1..tokens.len() - 1])
            .into_iter()
            .map(parse_expr_tokens)
            .collect::<Option<Vec<_>>>()
            .map(Expr::List);
    }
    if tokens.len() >= 3
        && tokens[0].kind == TokenKind::Ident
        && tokens[1].kind == TokenKind::LParen
        && tokens.last()?.kind == TokenKind::RParen
    {
        return split_expr_list(&tokens[2..tokens.len() - 1])
            .into_iter()
            .map(parse_expr_tokens)
            .collect::<Option<Vec<_>>>()
            .map(|args| Expr::Call {
                callee: tokens[0].text.clone(),
                args,
            });
    }
    if tokens.first()?.kind == TokenKind::LParen && tokens.last()?.kind == TokenKind::RParen {
        return parse_expr_tokens(&tokens[1..tokens.len() - 1]);
    }
    if let Some(index) = find_top_level_operator(tokens, TokenKind::EqEq) {
        return Some(Expr::Binary {
            op: BinaryOp::Eq,
            left: Box::new(parse_expr_tokens(&tokens[..index])?),
            right: Box::new(parse_expr_tokens(&tokens[index + 1..])?),
        });
    }
    if let Some(index) = find_top_level_operator(tokens, TokenKind::Plus) {
        return Some(Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(parse_expr_tokens(&tokens[..index])?),
            right: Box::new(parse_expr_tokens(&tokens[index + 1..])?),
        });
    }
    if let Some(index) = find_top_level_operator(tokens, TokenKind::Minus) {
        return Some(Expr::Binary {
            op: BinaryOp::Sub,
            left: Box::new(parse_expr_tokens(&tokens[..index])?),
            right: Box::new(parse_expr_tokens(&tokens[index + 1..])?),
        });
    }
    if tokens.len() == 2 && tokens[0].text == "duration" {
        return Some(token_to_expr(&tokens[1]));
    }
    if tokens.len() == 2 && tokens[0].text == "pitch" {
        return Some(token_to_expr(&tokens[1]));
    }
    if tokens.len() == 1 {
        return Some(token_to_expr(&tokens[0]));
    }
    Some(Expr::Ident(
        tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
    ))
}

fn find_top_level_operator(tokens: &[Token], kind: TokenKind) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::LBracket | TokenKind::LParen => depth += 1,
            TokenKind::RBracket | TokenKind::RParen => depth = depth.saturating_sub(1),
            _ if depth == 0 && token.kind == kind => return Some(index),
            _ => {}
        }
    }
    None
}

fn split_expr_list(tokens: &[Token]) -> Vec<&[Token]> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::LBracket | TokenKind::LParen => depth += 1,
            TokenKind::RBracket | TokenKind::RParen => depth = depth.saturating_sub(1),
            TokenKind::Comma if depth == 0 => {
                if start < index {
                    parts.push(&tokens[start..index]);
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    if start < tokens.len() {
        parts.push(&tokens[start..]);
    }
    parts
}

fn token_to_expr(token: &Token) -> Expr {
    match token.kind {
        TokenKind::Number => Expr::Int(token.text.parse().unwrap_or_default()),
        TokenKind::Pitch => Expr::PitchLiteral(token.text.clone()),
        TokenKind::Interval => Expr::IntervalLiteral(token.text.clone()),
        TokenKind::Duration => Expr::DurationLiteral(token.text.clone()),
        TokenKind::String => Expr::StringLiteral(token.text.clone()),
        _ if token.text == "true" => Expr::Bool(true),
        _ if token.text == "false" => Expr::Bool(false),
        _ => Expr::Ident(token.text.clone()),
    }
}

fn expr_to_source(expr: &Expr) -> String {
    match expr {
        Expr::Ident(value)
        | Expr::PitchLiteral(value)
        | Expr::IntervalLiteral(value)
        | Expr::DurationLiteral(value)
        | Expr::StringLiteral(value) => value.clone(),
        Expr::Int(value) => value.to_string(),
        Expr::Bool(value) => value.to_string(),
        Expr::List(values) => format!(
            "[{}]",
            values
                .iter()
                .map(expr_to_source)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Call { callee, args } => format!(
            "{}({})",
            callee,
            args.iter()
                .map(expr_to_source)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::Binary { op, left, right } => {
            let op = match op {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Eq => "==",
            };
            format!("{} {op} {}", expr_to_source(left), expr_to_source(right))
        }
    }
}

fn classify_word(text: &str) -> TokenKind {
    if text.parse::<i32>().is_ok() {
        return TokenKind::Number;
    }
    if text.contains('/') && text.split_once('/').is_some() {
        return TokenKind::Duration;
    }
    if matches!(
        text,
        "m2" | "M2" | "m3" | "M3" | "P4" | "TT" | "P5" | "m6" | "M6" | "m7" | "M7" | "P8"
    ) {
        return TokenKind::Interval;
    }
    if looks_like_pitch(text) {
        return TokenKind::Pitch;
    }
    TokenKind::Ident
}

fn looks_like_pitch(text: &str) -> bool {
    let Some(first) = text.chars().next() else {
        return false;
    };
    matches!(first, 'A'..='G') && text.chars().any(|ch| ch.is_ascii_digit())
}

fn is_word_start(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '#' | '/')
}

fn is_word_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '#' | '/' | 'b')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_pitch_arithmetic() {
        let tokens = tokenize_source("note C4 + M3, 1/4").unwrap();

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Plus));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Interval));
    }

    #[test]
    fn parses_minimal_score() {
        let source = r#"
score demo {
  voice lead {
    note C4, 1/4
    chord [C4, E4, G4], 1/2
  }
}
"#;
        let program = parse_source(source).unwrap();

        assert_eq!(program.score.name, "demo");
        assert_eq!(program.score.statements.len(), 1);
    }

    #[test]
    fn parses_style_and_override() {
        let source = r#"
style Classical
score demo {
  override scale allow reason "color" {
    note F#4, 1/4
  }
}
"#;
        let program = parse_source(source).unwrap();

        assert_eq!(program.style.unwrap().name, "Classical");
        assert_eq!(program.score.statements.len(), 1);
    }

    #[test]
    fn parses_multiple_styles_and_score_style_selection() {
        let source = r#"
style Classical
style Sparse {
  scale: C E G
}
score demo style Sparse {
  voice lead {
    note E4, 1/4
  }
}
"#;
        let program = parse_source(source).unwrap();

        assert_eq!(program.styles.len(), 2);
        assert_eq!(program.style.unwrap().name, "Classical");
        assert_eq!(program.score.style.as_deref(), Some("Sparse"));
    }

    #[test]
    fn parses_style_inheritance() {
        let source = r#"
style Classical {
  scale: C D E F G A B
}
style Chamber extends Classical {
  tempo_range: 60..120
}
score demo style Chamber {
  voice lead {
    note C4, 1/4
  }
}
"#;
        let program = parse_source(source).unwrap();

        assert_eq!(program.styles[1].name, "Chamber");
        assert_eq!(program.styles[1].parent.as_deref(), Some("Classical"));
    }

    #[test]
    fn parses_local_style_scope() {
        let source = r#"
style Classical
style Sparse {
  scale: C E G
}
score demo {
  voice lead {
    with style Sparse {
      note E4, 1/4
    }
  }
}
"#;
        let program = parse_source(source).unwrap();

        let Stmt::Voice(voice) = &program.score.statements[0] else {
            panic!("expected voice");
        };
        assert!(matches!(voice.statements[0], Stmt::WithStyle(_)));
    }

    #[test]
    fn parses_function_control_flow_and_config_style() {
        let source = r#"
style Custom {
  scale: C D E F G A B
}
fn motif {
  note C4, 1/8
}
score demo {
  voice lead {
    let root = C4
    for i in 0..2 {
      if i == 1 {
        call motif
      }
    }
  }
}
"#;
        let program = parse_source(source).unwrap();

        assert_eq!(program.functions.len(), 1);
        assert_eq!(program.style.unwrap().entries.len(), 1);
    }

    #[test]
    fn parses_dynamic_velocity_and_articulation_statements() {
        let source = r#"
score demo {
  voice lead {
    dynamic f
    articulation staccato
    note C4, 1/4
    velocity 32
    chord [C4, E4, G4], 1/4
  }
}
"#;
        let program = parse_source(source).unwrap();
        let Stmt::Voice(voice) = &program.score.statements[0] else {
            panic!("expected voice");
        };

        assert!(matches!(voice.statements[0], Stmt::Dynamic(_)));
        assert!(matches!(voice.statements[1], Stmt::Articulation(_)));
        assert!(matches!(voice.statements[3], Stmt::Velocity(_)));
    }

    #[test]
    fn parses_section_statement() {
        let source = r#"
score demo {
  section A {
    note C4, 1/4
  }
  section B {
    note D4, 1/4
  }
}
"#;
        let program = parse_source(source).unwrap();

        let Stmt::Section(section) = &program.score.statements[0] else {
            panic!("expected section");
        };
        assert_eq!(section.label, "A");
        assert_eq!(section.statements.len(), 1);
    }

    #[test]
    fn parses_non_chord_tone_statement() {
        let source = r#"
score demo {
  voice lead {
    non_chord_tone passing_tone {
      note D4, 1/8
    }
  }
}
"#;
        let program = parse_source(source).unwrap();
        let Stmt::Voice(voice) = &program.score.statements[0] else {
            panic!("expected voice");
        };
        let Stmt::NonChordTone(non_chord_tone) = &voice.statements[0] else {
            panic!("expected non-chord tone");
        };

        assert_eq!(non_chord_tone.kind, "passing_tone");
        assert_eq!(non_chord_tone.statements.len(), 1);
    }

    #[test]
    fn parses_tuning_system_statement() {
        let source = r#"
score demo {
  voice lead {
    tuning_system just_intonation {
      note D4, 1/8
    }
  }
}
"#;
        let program = parse_source(source).unwrap();
        let Stmt::Voice(voice) = &program.score.statements[0] else {
            panic!("expected voice");
        };
        let Stmt::TuningSystem(tuning_system) = &voice.statements[0] else {
            panic!("expected tuning system");
        };

        assert_eq!(tuning_system.kind, "just_intonation");
        assert_eq!(tuning_system.statements.len(), 1);
    }

    #[test]
    fn parses_world_tradition_statement() {
        let source = r#"
score demo {
  voice lead {
    world_tradition maqam {
      note D4, 1/8
    }
  }
}
"#;
        let program = parse_source(source).unwrap();
        let Stmt::Voice(voice) = &program.score.statements[0] else {
            panic!("expected voice");
        };
        let Stmt::WorldTradition(world_tradition) = &voice.statements[0] else {
            panic!("expected world tradition");
        };

        assert_eq!(world_tradition.kind, "maqam");
        assert_eq!(world_tradition.statements.len(), 1);
    }

    #[test]
    fn parses_historical_era_statement() {
        let source = r#"
score demo {
  voice lead {
    historical_era baroque {
      note D4, 1/8
    }
  }
}
"#;
        let program = parse_source(source).unwrap();
        let Stmt::Voice(voice) = &program.score.statements[0] else {
            panic!("expected voice");
        };
        let Stmt::HistoricalEra(historical_era) = &voice.statements[0] else {
            panic!("expected historical era");
        };

        assert_eq!(historical_era.kind, "baroque");
        assert_eq!(historical_era.statements.len(), 1);
    }

    #[test]
    fn reports_missing_closing_brace() {
        let diagnostics =
            parse_source("score demo {\n  voice lead {\n    note C4, 1/4\n").unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_PARSE_TOKEN"));
    }
}
