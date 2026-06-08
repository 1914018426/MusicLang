use musiclang_core::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) text: String,
    pub(crate) span: Span,
}

impl Token {
    pub fn kind(&self) -> TokenKind {
        self.kind
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub(crate) fn is_keyword_text(&self, text: &str) -> bool {
        self.kind.keyword_text() == Some(text)
    }
}

impl TokenKind {
    pub fn keyword_text(self) -> Option<&'static str> {
        match self {
            TokenKind::Style => Some("style"),
            TokenKind::Score => Some("score"),
            TokenKind::Voice => Some("voice"),
            TokenKind::Title => Some("title"),
            TokenKind::Composer => Some("composer"),
            TokenKind::Tempo => Some("tempo"),
            TokenKind::Meter => Some("meter"),
            TokenKind::Key => Some("key"),
            TokenKind::Program => Some("program"),
            TokenKind::Instrument => Some("instrument"),
            TokenKind::Channel => Some("channel"),
            TokenKind::Volume => Some("volume"),
            TokenKind::Pan => Some("pan"),
            TokenKind::Note => Some("note"),
            TokenKind::Chord => Some("chord"),
            TokenKind::Drum => Some("drum"),
            TokenKind::Rest => Some("rest"),
            TokenKind::Glissando => Some("glissando"),
            TokenKind::Tremolo => Some("tremolo"),
            TokenKind::Degree => Some("degree"),
            TokenKind::Scale => Some("scale"),
            TokenKind::Pedal => Some("pedal"),
            TokenKind::Ostinato => Some("ostinato"),
            TokenKind::Sequence => Some("sequence"),
            TokenKind::Tuplet => Some("tuplet"),
            TokenKind::Transpose => Some("transpose"),
            TokenKind::Arpeggio => Some("arpeggio"),
            TokenKind::Strum => Some("strum"),
            TokenKind::Roman => Some("roman"),
            TokenKind::Progression => Some("progression"),
            TokenKind::Cadence => Some("cadence"),
            TokenKind::Modulate => Some("modulate"),
            TokenKind::Dynamic => Some("dynamic"),
            TokenKind::Velocity => Some("velocity"),
            TokenKind::Articulation => Some("articulation"),
            TokenKind::Section => Some("section"),
            TokenKind::Ornament => Some("ornament"),
            TokenKind::NonChordTone => Some("non_chord_tone"),
            TokenKind::TuningSystem => Some("tuning_system"),
            TokenKind::WorldTradition => Some("world_tradition"),
            TokenKind::HistoricalEra => Some("historical_era"),
            TokenKind::HarmonicFunction => Some("harmonic_function"),
            TokenKind::Let => Some("let"),
            TokenKind::Fn => Some("fn"),
            TokenKind::Call => Some("call"),
            TokenKind::For => Some("for"),
            TokenKind::In => Some("in"),
            TokenKind::If => Some("if"),
            TokenKind::Then => Some("then"),
            TokenKind::Else => Some("else"),
            TokenKind::And => Some("and"),
            TokenKind::Or => Some("or"),
            TokenKind::Not => Some("not"),
            TokenKind::Override => Some("override"),
            TokenKind::Allow => Some("allow"),
            TokenKind::Reason => Some("reason"),
            TokenKind::Play => Some("play"),
            TokenKind::With => Some("with"),
            TokenKind::To => Some("to"),
            TokenKind::Steps => Some("steps"),
            TokenKind::Repeats => Some("repeats"),
            TokenKind::By => Some("by"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Ident,
    Number,
    Pitch,
    Interval,
    Duration,
    String,
    Style,
    Score,
    Voice,
    Title,
    Composer,
    Tempo,
    Meter,
    Key,
    Program,
    Instrument,
    Channel,
    Volume,
    Pan,
    Note,
    Chord,
    Drum,
    Rest,
    Glissando,
    Tremolo,
    Degree,
    Scale,
    Pedal,
    Ostinato,
    Sequence,
    Tuplet,
    Transpose,
    Arpeggio,
    Strum,
    Roman,
    Progression,
    Cadence,
    Modulate,
    Dynamic,
    Velocity,
    Articulation,
    Section,
    Ornament,
    NonChordTone,
    TuningSystem,
    WorldTradition,
    HistoricalEra,
    HarmonicFunction,
    Let,
    Fn,
    Call,
    For,
    In,
    If,
    Then,
    Else,
    And,
    Or,
    Not,
    Override,
    Allow,
    Reason,
    Play,
    With,
    To,
    Steps,
    Repeats,
    By,
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
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Dot,
    DotDot,
    Pipe,
    Plus,
    Minus,
    Star,
    Slash,
    Eof,
}

pub(crate) fn classify_word(text: &str) -> TokenKind {
    if text.parse::<i32>().is_ok() {
        return TokenKind::Number;
    }
    if looks_like_duration(text) {
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
    match text {
        "style" => TokenKind::Style,
        "score" => TokenKind::Score,
        "voice" => TokenKind::Voice,
        "title" => TokenKind::Title,
        "composer" => TokenKind::Composer,
        "tempo" => TokenKind::Tempo,
        "meter" => TokenKind::Meter,
        "key" => TokenKind::Key,
        "program" => TokenKind::Program,
        "instrument" => TokenKind::Instrument,
        "channel" => TokenKind::Channel,
        "volume" => TokenKind::Volume,
        "pan" => TokenKind::Pan,
        "note" => TokenKind::Note,
        "chord" => TokenKind::Chord,
        "drum" => TokenKind::Drum,
        "rest" => TokenKind::Rest,
        "glissando" => TokenKind::Glissando,
        "tremolo" => TokenKind::Tremolo,
        "degree" => TokenKind::Degree,
        "scale" => TokenKind::Scale,
        "pedal" => TokenKind::Pedal,
        "ostinato" => TokenKind::Ostinato,
        "sequence" => TokenKind::Sequence,
        "tuplet" => TokenKind::Tuplet,
        "transpose" => TokenKind::Transpose,
        "arpeggio" => TokenKind::Arpeggio,
        "strum" => TokenKind::Strum,
        "roman" => TokenKind::Roman,
        "progression" => TokenKind::Progression,
        "cadence" => TokenKind::Cadence,
        "modulate" => TokenKind::Modulate,
        "dynamic" => TokenKind::Dynamic,
        "velocity" => TokenKind::Velocity,
        "articulation" => TokenKind::Articulation,
        "section" => TokenKind::Section,
        "ornament" => TokenKind::Ornament,
        "non_chord_tone" => TokenKind::NonChordTone,
        "tuning_system" => TokenKind::TuningSystem,
        "world_tradition" => TokenKind::WorldTradition,
        "historical_era" => TokenKind::HistoricalEra,
        "harmonic_function" => TokenKind::HarmonicFunction,
        "let" => TokenKind::Let,
        "fn" => TokenKind::Fn,
        "call" => TokenKind::Call,
        "for" => TokenKind::For,
        "in" => TokenKind::In,
        "if" => TokenKind::If,
        "then" => TokenKind::Then,
        "else" => TokenKind::Else,
        "and" => TokenKind::And,
        "or" => TokenKind::Or,
        "not" => TokenKind::Not,
        "override" => TokenKind::Override,
        "allow" => TokenKind::Allow,
        "reason" => TokenKind::Reason,
        "play" => TokenKind::Play,
        "with" => TokenKind::With,
        "to" => TokenKind::To,
        "steps" => TokenKind::Steps,
        "repeats" => TokenKind::Repeats,
        "by" => TokenKind::By,
        _ => TokenKind::Ident,
    }
}

fn looks_like_duration(text: &str) -> bool {
    let Some((numerator, denominator)) = text.split_once('/') else {
        return false;
    };
    !numerator.is_empty()
        && !denominator.is_empty()
        && numerator.chars().all(|ch| ch.is_ascii_digit())
        && denominator.chars().all(|ch| ch.is_ascii_digit())
}

fn looks_like_pitch(text: &str) -> bool {
    let Some(first) = text.chars().next() else {
        return false;
    };
    matches!(first, 'A'..='G') && text.chars().any(|ch| ch.is_ascii_digit())
}

pub(crate) fn is_word_start(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '#' | '/')
}

pub(crate) fn is_word_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '#' | '/' | 'b')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_music_literals() {
        assert_eq!(classify_word("42"), TokenKind::Number);
        assert_eq!(classify_word("1/8"), TokenKind::Duration);
        assert_eq!(classify_word("1//8"), TokenKind::Ident);
        assert_eq!(classify_word("/8"), TokenKind::Ident);
        assert_eq!(classify_word("1/"), TokenKind::Ident);
        assert_eq!(classify_word("a/b"), TokenKind::Ident);
        assert_eq!(classify_word("M3"), TokenKind::Interval);
        assert_eq!(classify_word("C4"), TokenKind::Pitch);
        assert_eq!(classify_word("score"), TokenKind::Score);
        assert_eq!(classify_word("if"), TokenKind::If);
        assert_eq!(classify_word("then"), TokenKind::Then);
        assert_eq!(classify_word("and"), TokenKind::And);
        assert_eq!(classify_word("lead"), TokenKind::Ident);
    }

    #[test]
    fn exposes_token_fields_through_accessors() {
        let token = Token {
            kind: TokenKind::Pitch,
            text: "C4".to_string(),
            span: Span {
                source_id: musiclang_core::SourceId(0),
                start: 0,
                end: 2,
                line: 1,
                column: 1,
            },
        };

        assert_eq!(token.kind(), TokenKind::Pitch);
        assert_eq!(token.text(), "C4");
        assert_eq!(token.span().start, 0);
        assert_eq!(token.span().end, 2);
    }
}
