use std::collections::{BTreeSet, HashMap, HashSet};

use musiclang_core::{
    Chord, CustomStyleRule, CustomTheoryDomain, Diagnostic, Duration, InstrumentRange, Interval,
    KeySignature, MarkerIr, Meter, Note, NoteEventIr, OverrideTrace, Pitch, PitchClass,
    RuleSeverity, ScoreIr, Severity, Span, StyleContext, TheoryDomain, TheoryReference, TrackIr,
    DEFAULT_TICKS_PER_QUARTER,
};
use musiclang_parser::{
    parse_source, ArticulationStmt, BinaryOp, ChordStmt, DynamicStmt, Expr, FunctionDecl, NoteStmt,
    OverrideStmt, Program, Stmt, StyleDecl, VoiceDecl, WithStyleStmt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compilation {
    pub ir: ScoreIr,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn compile_source(source: &str) -> Result<ScoreIr, Vec<Diagnostic>> {
    compile_source_with_diagnostics(source).map(|compilation| compilation.ir)
}

pub fn compile_source_with_diagnostics(source: &str) -> Result<Compilation, Vec<Diagnostic>> {
    let program = parse_source(source)?;
    Compiler::new(program).compile()
}

pub fn diagnose_source(source: &str) -> Vec<Diagnostic> {
    match compile_source_with_diagnostics(source) {
        Ok(compilation) => compilation.diagnostics,
        Err(diagnostics) => diagnostics,
    }
}

mod context;
mod eval;
mod lower;
mod stylecheck;

use eval::Value;

struct Compiler {
    program: Program,
    style: StyleContext,
    functions: HashMap<String, FunctionDecl>,
    function_call_stack: Vec<String>,
    variables: Vec<HashMap<String, Value>>,
    diagnostics: Vec<Diagnostic>,
    override_rules: Vec<String>,
    score_override_rules: HashSet<String>,
    override_traces: Vec<OverrideTrace>,
    section_labels: Vec<String>,
    markers: Vec<MarkerIr>,
    pending_non_chord_tones: Vec<PendingNonChordTone>,
}

struct PendingNonChordTone {
    kind: String,
    previous_event_index: Option<usize>,
    event_start: usize,
    event_end: usize,
    line: usize,
    column: usize,
}

impl Compiler {
    fn new(program: Program) -> Self {
        let (style, mut diagnostics) = context::style(&program);
        let (functions, function_diagnostics) = context::functions(&program);
        diagnostics.extend(function_diagnostics);
        Self {
            program,
            style,
            functions,
            function_call_stack: Vec::new(),
            variables: vec![HashMap::new()],
            diagnostics,
            override_rules: Vec::new(),
            score_override_rules: HashSet::new(),
            override_traces: Vec::new(),
            section_labels: Vec::new(),
            markers: Vec::new(),
            pending_non_chord_tones: Vec::new(),
        }
    }

    fn compile(mut self) -> Result<Compilation, Vec<Diagnostic>> {
        let mut tracks = Vec::new();
        let (tempo_bpm, meter, key) = lower::score_metadata(&self.program);
        self.check_score_style(tempo_bpm, meter);
        let statements = self.program.score.statements.clone();

        for statement in statements {
            match statement {
                Stmt::Voice(voice) => {
                    let mut track = TrackBuilder::new(&voice.name, voice.program);
                    self.compile_voice(&voice, &mut track);
                    tracks.push(track.finish());
                }
                Stmt::Override(override_stmt)
                    if override_stmt
                        .statements
                        .iter()
                        .any(|statement| matches!(statement, Stmt::Voice(_))) =>
                {
                    self.compile_override_tracks(&override_stmt, &mut tracks);
                }
                other => {
                    let mut track = TrackBuilder::new("main", None);
                    self.compile_statement(&other, &mut track);
                    if !track.events.is_empty() {
                        tracks.push(track.finish());
                    }
                }
            }
        }

        self.check_counterpoint_rules(&tracks);
        self.check_texture(&tracks);
        self.check_rhythm_concepts(&tracks);
        self.check_form();
        self.check_harmonic_progression(&tracks);
        self.check_cadence(&tracks);

        if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
        {
            return Err(self.diagnostics);
        }

        Ok(Compilation {
            ir: lower::score_ir(
                self.program,
                tempo_bpm,
                meter,
                key,
                tracks,
                self.markers,
                self.override_traces,
            ),
            diagnostics: self.diagnostics,
        })
    }

    fn compile_voice(&mut self, voice: &VoiceDecl, track: &mut TrackBuilder) {
        let pending_start = self.pending_non_chord_tones.len();
        self.compile_statements(&voice.statements, track);
        self.check_pending_non_chord_tones(track, pending_start);
        self.check_melodic_leap(track, voice.line, voice.column);
    }

    fn compile_statements(&mut self, statements: &[Stmt], track: &mut TrackBuilder) {
        for statement in statements {
            self.compile_statement(statement, track);
        }
    }

    fn compile_statement(&mut self, statement: &Stmt, track: &mut TrackBuilder) {
        match statement {
            Stmt::Voice(voice) => self.compile_voice(voice, track),
            Stmt::Note(note) => self.compile_note(note, track),
            Stmt::Chord(chord) => self.compile_chord(chord, track),
            Stmt::Dynamic(dynamic) => {
                self.check_dynamic_vocab(dynamic);
                if let Some(velocity) = dynamic_velocity(&dynamic.mark) {
                    track.set_velocity(velocity);
                }
            }
            Stmt::Velocity(velocity) => track.set_velocity(velocity.velocity),
            Stmt::Articulation(articulation) => {
                self.check_articulation_vocab(articulation);
                track.set_articulation(&articulation.mark);
            }
            Stmt::Section(section) => {
                self.section_labels.push(section.label.clone());
                self.markers.push(MarkerIr {
                    label: section.label.clone(),
                    tick: track.cursor_tick(),
                });
                self.compile_statements(&section.statements, track);
            }
            Stmt::Ornament(ornament) => {
                self.check_ornament(&ornament.kind, ornament.line, ornament.column);
                let event_start = track.event_count();
                self.compile_statements(&ornament.statements, track);
                self.check_ornament_pattern(
                    &ornament.kind,
                    &track.events()[event_start..],
                    ornament.line,
                    ornament.column,
                );
            }
            Stmt::NonChordTone(non_chord_tone) => {
                self.check_non_chord_tone(
                    &non_chord_tone.kind,
                    non_chord_tone.line,
                    non_chord_tone.column,
                );
                let previous_event_index = track.event_count().checked_sub(1);
                let event_start = track.event_count();
                self.compile_statements(&non_chord_tone.statements, track);
                self.pending_non_chord_tones.push(PendingNonChordTone {
                    kind: non_chord_tone.kind.clone(),
                    previous_event_index,
                    event_start,
                    event_end: track.event_count(),
                    line: non_chord_tone.line,
                    column: non_chord_tone.column,
                });
            }
            Stmt::TuningSystem(tuning_system) => {
                self.check_tuning_system(
                    &tuning_system.kind,
                    tuning_system.line,
                    tuning_system.column,
                );
                self.compile_statements(&tuning_system.statements, track);
            }
            Stmt::WorldTradition(world_tradition) => {
                self.check_world_tradition(
                    &world_tradition.kind,
                    world_tradition.line,
                    world_tradition.column,
                );
                self.compile_statements(&world_tradition.statements, track);
            }
            Stmt::HistoricalEra(historical_era) => {
                self.check_historical_era(
                    &historical_era.kind,
                    historical_era.line,
                    historical_era.column,
                );
                self.compile_statements(&historical_era.statements, track);
            }
            Stmt::HarmonicFunction(harmonic_function) => {
                self.check_harmonic_function(
                    &harmonic_function.kind,
                    harmonic_function.line,
                    harmonic_function.column,
                );
                self.compile_statements(&harmonic_function.statements, track);
            }
            Stmt::For(for_stmt) => {
                for value in for_stmt.start..for_stmt.end {
                    self.push_scope();
                    self.set_var(&for_stmt.variable, Value::Int(value));
                    self.compile_statements(&for_stmt.statements, track);
                    self.pop_scope();
                }
            }
            Stmt::If(if_stmt) => {
                if self.eval_bool(&if_stmt.condition, if_stmt.line, if_stmt.column) == Some(true) {
                    self.compile_statements(&if_stmt.statements, track);
                }
            }
            Stmt::Let(let_stmt) => {
                if let Some(value) =
                    self.eval_expr(&let_stmt.value_expr, let_stmt.line, let_stmt.column)
                {
                    self.set_var(&let_stmt.name, value);
                }
            }
            Stmt::Call(call) => {
                if self.function_call_stack.contains(&call.name) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "ML_RESOLVE_RECURSIVE_CALL",
                            format!("recursive function call `{}`", call.name),
                            call.line,
                            call.column,
                        )
                        .with_span(call.span),
                    );
                    return;
                }

                if let Some(function) = self.functions.get(&call.name).cloned() {
                    self.function_call_stack.push(call.name.clone());
                    self.push_scope();
                    self.compile_statements(&function.statements, track);
                    self.pop_scope();
                    self.function_call_stack.pop();
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "ML_RESOLVE_UNKNOWN_NAME",
                            format!("unknown function `{}`", call.name),
                            call.line,
                            call.column,
                        )
                        .with_span(call.span),
                    );
                }
            }
            Stmt::Override(override_stmt) => self.compile_override(override_stmt, track),
            Stmt::WithStyle(with_style) => self.compile_with_style(with_style, track),
        }
    }

    fn compile_with_style(&mut self, with_style: &WithStyleStmt, track: &mut TrackBuilder) {
        let Some(style) = self.style_context_by_name(
            &with_style.style,
            with_style.line,
            with_style.column,
            Some(with_style.span),
        ) else {
            return;
        };
        let previous = std::mem::replace(&mut self.style, style);
        self.compile_statements(&with_style.statements, track);
        self.style = previous;
    }

    fn compile_note(&mut self, note: &NoteStmt, track: &mut TrackBuilder) {
        let Some(pitch) = self.eval_pitch(&note.pitch_expr, note.line, note.column) else {
            return;
        };
        let Some(duration) = self.eval_duration(&note.duration_expr, note.line, note.column) else {
            return;
        };
        self.check_pitch_style(pitch, note.line, note.column, Some(note.span));
        self.check_rhythm_vocab(duration, note.line, note.column, Some(note.span));
        self.check_instrument_range(
            track.program,
            pitch,
            note.line,
            note.column,
            Some(note.span),
        );
        track.push_note(Note::new(pitch, duration), Some(note.span));
    }

    fn compile_chord(&mut self, chord: &ChordStmt, track: &mut TrackBuilder) {
        let mut pitches = Vec::new();
        for pitch_expr in &chord.pitch_exprs {
            match self.eval_expr(pitch_expr, chord.line, chord.column) {
                Some(Value::Pitch(pitch)) => {
                    self.check_pitch_style(pitch, chord.line, chord.column, Some(chord.span));
                    self.check_instrument_range(
                        track.program,
                        pitch,
                        chord.line,
                        chord.column,
                        Some(chord.span),
                    );
                    pitches.push(pitch);
                }
                Some(Value::List(values)) => {
                    for value in values {
                        if let Value::Pitch(pitch) = value {
                            self.check_pitch_style(
                                pitch,
                                chord.line,
                                chord.column,
                                Some(chord.span),
                            );
                            self.check_instrument_range(
                                track.program,
                                pitch,
                                chord.line,
                                chord.column,
                                Some(chord.span),
                            );
                            pitches.push(pitch);
                        } else {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "ML_TYPE_MISMATCH",
                                    "expected pitch expression",
                                    chord.line,
                                    chord.column,
                                )
                                .with_span(chord.span),
                            );
                        }
                    }
                }
                Some(_) => self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "expected pitch expression",
                        chord.line,
                        chord.column,
                    )
                    .with_span(chord.span),
                ),
                None => {}
            }
        }
        let Some(duration) = self.eval_duration(&chord.duration_expr, chord.line, chord.column)
        else {
            return;
        };
        self.check_chord_vocab(&pitches, chord.line, chord.column, Some(chord.span));
        self.check_chord_quality_vocab(&pitches, chord.line, chord.column, Some(chord.span));
        self.check_set_class_vocab(&pitches, chord.line, chord.column, Some(chord.span));
        self.check_rhythm_vocab(duration, chord.line, chord.column, Some(chord.span));
        match Chord::new(pitches, duration) {
            Ok(compiled_chord) => track.push_chord(compiled_chord, Some(chord.span)),
            Err(error) => self.diagnostics.push(
                Diagnostic::error("ML_CORE_CHORD", error.to_string(), chord.line, chord.column)
                    .with_span(chord.span),
            ),
        }
    }

    fn push_style_diagnostic(
        &mut self,
        rule: &'static str,
        code: &'static str,
        message: String,
        line: usize,
        column: usize,
    ) {
        self.push_style_diagnostic_with_span(rule, code, message, line, column, None);
    }

    fn push_style_diagnostic_with_span(
        &mut self,
        rule: &'static str,
        code: &'static str,
        message: String,
        line: usize,
        column: usize,
        span: Option<Span>,
    ) {
        match self.style.rule_severity(rule) {
            RuleSeverity::Off => {}
            RuleSeverity::Error => {
                let mut diagnostic = Diagnostic::error(code, message, line, column)
                    .with_rule(rule)
                    .with_style(self.style.name.clone());
                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(span);
                }
                self.diagnostics.push(diagnostic);
            }
            RuleSeverity::Warning => {
                let mut diagnostic = Diagnostic::warning(code, message, line, column)
                    .with_rule(rule)
                    .with_style(self.style.name.clone());
                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(span);
                }
                self.diagnostics.push(diagnostic);
            }
        }
    }

    fn check_dynamic_vocab(&mut self, dynamic: &DynamicStmt) {
        if self.style.dynamic_vocab.is_empty()
            || self.has_override("dynamic_vocab")
            || self.has_score_override("dynamic_vocab")
        {
            return;
        }
        if !self
            .style
            .dynamic_vocab
            .iter()
            .any(|mark| mark == &dynamic.mark)
        {
            self.push_style_diagnostic(
                "dynamic_vocab",
                "ML_STYLE_DYNAMIC_VOCAB",
                format!(
                    "dynamic `{}` is outside active style dynamic vocabulary",
                    dynamic.mark
                ),
                dynamic.line,
                dynamic.column,
            );
        }
    }

    fn check_articulation_vocab(&mut self, articulation: &ArticulationStmt) {
        if self.style.articulation_vocab.is_empty()
            || self.has_override("articulation_vocab")
            || self.has_score_override("articulation_vocab")
        {
            return;
        }
        if !self
            .style
            .articulation_vocab
            .iter()
            .any(|mark| mark == &articulation.mark)
        {
            self.push_style_diagnostic(
                "articulation_vocab",
                "ML_STYLE_ARTICULATION_VOCAB",
                format!(
                    "articulation `{}` is outside active style articulation vocabulary",
                    articulation.mark
                ),
                articulation.line,
                articulation.column,
            );
        }
    }

    fn check_ornament(&mut self, kind: &str, line: usize, column: usize) {
        if self.style.ornaments.is_empty()
            || self.has_override("ornament")
            || self.has_score_override("ornament")
        {
            return;
        }
        if !self.style.ornaments.iter().any(|allowed| allowed == kind) {
            self.push_style_diagnostic(
                "ornament",
                "ML_STYLE_ORNAMENT",
                format!("ornament `{kind}` is outside active style vocabulary"),
                line,
                column,
            );
        }
    }

    fn check_ornament_pattern(
        &mut self,
        kind: &str,
        events: &[NoteEventIr],
        line: usize,
        column: usize,
    ) {
        if self.style.ornaments.is_empty()
            || self.has_override("ornament")
            || self.has_score_override("ornament")
            || !self.style.ornaments.iter().any(|allowed| allowed == kind)
        {
            return;
        }
        let message = match kind {
            "trill" if !events_form_trill(events) => {
                Some("trill ornament must alternate rapidly between two pitch classes")
            }
            "mordent" if !events_form_mordent(events) => {
                Some("mordent ornament must move from a main pitch to a neighbor and back")
            }
            "turn" if !events_form_turn(events) => {
                Some("turn ornament must outline upper neighbor, main pitch, lower neighbor, and main pitch")
            }
            _ => None,
        };
        if let Some(message) = message {
            self.push_style_diagnostic(
                "ornament",
                "ML_STYLE_ORNAMENT",
                message.to_string(),
                line,
                column,
            );
        }
    }

    fn check_non_chord_tone(&mut self, kind: &str, line: usize, column: usize) {
        if self.style.non_chord_tones.is_empty()
            || self.has_override("non_chord_tone")
            || self.has_score_override("non_chord_tone")
        {
            return;
        }
        if !self
            .style
            .non_chord_tones
            .iter()
            .any(|allowed| allowed == kind)
        {
            self.push_style_diagnostic(
                "non_chord_tone",
                "ML_STYLE_NON_CHORD_TONE",
                format!("non-chord tone `{kind}` is outside active style vocabulary"),
                line,
                column,
            );
        }
    }

    fn check_pending_non_chord_tones(&mut self, track: &TrackBuilder, pending_start: usize) {
        let pending = self.pending_non_chord_tones.split_off(pending_start);
        for non_chord_tone in pending {
            if self.style.non_chord_tones.is_empty()
                || self.has_override("non_chord_tone")
                || self.has_score_override("non_chord_tone")
                || !self
                    .style
                    .non_chord_tones
                    .iter()
                    .any(|allowed| allowed == &non_chord_tone.kind)
            {
                continue;
            }
            let previous = non_chord_tone
                .previous_event_index
                .and_then(|index| track.events().get(index));
            let tones = &track.events()[non_chord_tone.event_start..non_chord_tone.event_end];
            let next = track.events().get(non_chord_tone.event_end);
            let message = match non_chord_tone.kind.as_str() {
                "passing_tone" if !events_form_passing_tone(previous, tones, next) => Some(
                    "passing tone must connect surrounding tones by stepwise motion in one direction",
                ),
                "neighbor_tone" if !events_form_neighbor_tone(previous, tones, next) => Some(
                    "neighbor tone must step away from and return to the same surrounding pitch",
                ),
                _ => None,
            };
            if let Some(message) = message {
                self.push_style_diagnostic(
                    "non_chord_tone",
                    "ML_STYLE_NON_CHORD_TONE",
                    message.to_string(),
                    non_chord_tone.line,
                    non_chord_tone.column,
                );
            }
        }
    }

    fn check_tuning_system(&mut self, kind: &str, line: usize, column: usize) {
        if self.style.tuning_systems.is_empty()
            || self.has_override("tuning_system")
            || self.has_score_override("tuning_system")
        {
            return;
        }
        if !self
            .style
            .tuning_systems
            .iter()
            .any(|allowed| allowed == kind)
        {
            self.push_style_diagnostic(
                "tuning_system",
                "ML_STYLE_TUNING_SYSTEM",
                format!("tuning system `{kind}` is outside active style vocabulary"),
                line,
                column,
            );
        }
    }

    fn check_world_tradition(&mut self, kind: &str, line: usize, column: usize) {
        if self.style.world_traditions.is_empty()
            || self.has_override("world_tradition")
            || self.has_score_override("world_tradition")
        {
            return;
        }
        if !self
            .style
            .world_traditions
            .iter()
            .any(|allowed| allowed == kind)
        {
            self.push_style_diagnostic(
                "world_tradition",
                "ML_STYLE_WORLD_TRADITION",
                format!("world tradition `{kind}` is outside active style vocabulary"),
                line,
                column,
            );
        }
    }

    fn check_historical_era(&mut self, kind: &str, line: usize, column: usize) {
        if self.style.historical_eras.is_empty()
            || self.has_override("historical_era")
            || self.has_score_override("historical_era")
        {
            return;
        }
        if !self
            .style
            .historical_eras
            .iter()
            .any(|allowed| allowed == kind)
        {
            self.push_style_diagnostic(
                "historical_era",
                "ML_STYLE_HISTORICAL_ERA",
                format!("historical era `{kind}` is outside active style vocabulary"),
                line,
                column,
            );
        }
    }

    fn check_harmonic_function(&mut self, kind: &str, line: usize, column: usize) {
        if self.style.harmonic_functions.is_empty()
            || self.has_override("harmonic_function")
            || self.has_score_override("harmonic_function")
        {
            return;
        }
        if !self
            .style
            .harmonic_functions
            .iter()
            .any(|allowed| allowed == kind)
        {
            self.push_style_diagnostic(
                "harmonic_function",
                "ML_STYLE_HARMONIC_FUNCTION",
                format!("harmonic function `{kind}` is outside active style vocabulary"),
                line,
                column,
            );
        }
    }

    fn check_melodic_leap(&mut self, track: &TrackBuilder, line: usize, column: usize) {
        let Some(max_leap) = self.style.max_melodic_leap else {
            return;
        };
        let mut melody = track.events().to_vec();
        melody.sort_by_key(|event| event.start_tick);
        melody.dedup_by_key(|event| event.start_tick);
        for window in melody.windows(2) {
            let [first, second] = window else {
                continue;
            };
            let (Ok(first_midi), Ok(second_midi)) =
                (first.pitch.midi_number(), second.pitch.midi_number())
            else {
                continue;
            };
            if track.is_event_overridden("max_melodic_leap", first.start_tick)
                || track.is_event_overridden("max_melodic_leap", second.start_tick)
            {
                continue;
            }
            let leap = (i16::from(second_midi) - i16::from(first_midi)).abs();
            if leap > max_leap.semitones().abs() {
                self.push_style_diagnostic(
                    "max_melodic_leap",
                    "ML_STYLE_MAX_MELODIC_LEAP",
                    format!(
                        "melodic leap of {leap} semitones exceeds maximum {}",
                        max_leap.semitones().abs()
                    ),
                    second.source_span.map_or(line, |span| span.line),
                    second.source_span.map_or(column, |span| span.column),
                );
                return;
            }
        }
    }

    fn check_counterpoint_rules(&mut self, tracks: &[TrackIr]) {
        if tracks.len() < 2 {
            return;
        }
        for upper_index in 0..tracks.len() {
            for lower_index in (upper_index + 1)..tracks.len() {
                self.check_voice_crossing(&tracks[upper_index], &tracks[lower_index]);
                self.check_parallel_fifths(&tracks[upper_index], &tracks[lower_index]);
                self.check_contrapuntal_motion(&tracks[upper_index], &tracks[lower_index]);
            }
        }
    }

    fn check_voice_crossing(&mut self, upper: &TrackIr, lower: &TrackIr) {
        if self.has_override("voice_crossing") || self.has_score_override("voice_crossing") {
            return;
        }
        for upper_event in &upper.events {
            for lower_event in &lower.events {
                if upper_event.start_tick == lower_event.start_tick
                    && upper_event.pitch.midi_number().ok() < lower_event.pitch.midi_number().ok()
                {
                    self.push_style_diagnostic(
                        "voice_crossing",
                        "ML_STYLE_VOICE_CROSSING",
                        format!(
                            "voice `{}` crosses below voice `{}`",
                            upper.name, lower.name
                        ),
                        upper_event.source_span.map_or(1, |span| span.line),
                        upper_event.source_span.map_or(1, |span| span.column),
                    );
                    return;
                }
            }
        }
    }

    fn check_parallel_fifths(&mut self, upper: &TrackIr, lower: &TrackIr) {
        if self.has_override("parallel_fifths") || self.has_score_override("parallel_fifths") {
            return;
        }
        let mut pairs = Vec::new();
        for upper_event in &upper.events {
            for lower_event in &lower.events {
                if upper_event.start_tick == lower_event.start_tick {
                    pairs.push((upper_event, lower_event));
                }
            }
        }
        pairs.sort_by_key(|(upper_event, _)| upper_event.start_tick);
        for window in pairs.windows(2) {
            let [(upper_a, lower_a), (upper_b, lower_b)] = window else {
                continue;
            };
            let first = stylecheck::interval_mod_12(upper_a.pitch, lower_a.pitch);
            let second = stylecheck::interval_mod_12(upper_b.pitch, lower_b.pitch);
            let upper_motion = upper_a.pitch.midi_number().ok() != upper_b.pitch.midi_number().ok();
            let lower_motion = lower_a.pitch.midi_number().ok() != lower_b.pitch.midi_number().ok();
            if first == Some(7) && second == Some(7) && upper_motion && lower_motion {
                self.push_style_diagnostic(
                    "parallel_fifths",
                    "ML_STYLE_PARALLEL_FIFTHS",
                    format!(
                        "voices `{}` and `{}` move in parallel fifths",
                        upper.name, lower.name
                    ),
                    upper_b.source_span.map_or(1, |span| span.line),
                    upper_b.source_span.map_or(1, |span| span.column),
                );
                return;
            }
        }
    }

    fn check_contrapuntal_motion(&mut self, upper: &TrackIr, lower: &TrackIr) {
        if self.style.contrapuntal_motion.is_empty()
            || self.has_override("contrapuntal_motion")
            || self.has_score_override("contrapuntal_motion")
        {
            return;
        }
        let mut pairs = Vec::new();
        for upper_event in &upper.events {
            for lower_event in &lower.events {
                if upper_event.start_tick == lower_event.start_tick {
                    pairs.push((upper_event, lower_event));
                }
            }
        }
        pairs.sort_by_key(|(upper_event, _)| upper_event.start_tick);
        for window in pairs.windows(2) {
            let [(upper_a, lower_a), (upper_b, lower_b)] = window else {
                continue;
            };
            let Some(motion) = motion_type(upper_a, lower_a, upper_b, lower_b) else {
                continue;
            };
            if !self
                .style
                .contrapuntal_motion
                .iter()
                .any(|allowed| allowed == motion)
            {
                self.push_style_diagnostic(
                    "contrapuntal_motion",
                    "ML_STYLE_CONTRAPUNTAL_MOTION",
                    format!(
                        "voices `{}` and `{}` use disallowed {motion} motion",
                        upper.name, lower.name
                    ),
                    upper_b.source_span.map_or(1, |span| span.line),
                    upper_b.source_span.map_or(1, |span| span.column),
                );
                return;
            }
        }
    }

    fn check_texture(&mut self, tracks: &[TrackIr]) {
        let Some(texture) = self.style.texture.clone() else {
            return;
        };
        if self.has_override("texture") || self.has_score_override("texture") {
            return;
        }
        let valid = match texture.as_str() {
            "monophony" | "monophonic" => {
                tracks
                    .iter()
                    .filter(|track| !track.events.is_empty())
                    .count()
                    <= 1
            }
            "polyphony" | "polyphonic" => {
                tracks
                    .iter()
                    .filter(|track| !track.events.is_empty())
                    .count()
                    >= 2
            }
            "homophony" | "homophonic" => tracks_share_attack_grid(tracks),
            "heterophony" | "heterophonic" => tracks_are_heterophonic(tracks),
            _ => true,
        };
        if !valid {
            self.push_style_diagnostic(
                "texture",
                "ML_STYLE_TEXTURE",
                format!("score texture does not satisfy `{texture}`"),
                self.program.score.line,
                self.program.score.column,
            );
        }
    }

    fn check_rhythm_concepts(&mut self, tracks: &[TrackIr]) {
        if self.style.rhythm_concepts.is_empty()
            || self.has_override("rhythm_concept")
            || self.has_score_override("rhythm_concept")
        {
            return;
        }
        for concept in self.style.rhythm_concepts.clone() {
            let valid = match concept.as_str() {
                "ostinato" => tracks.iter().any(track_has_repeated_duration_cell),
                "syncopation" => tracks.iter().any(track_has_syncopation),
                "hemiola" => tracks.iter().any(track_has_hemiola),
                "swing" => tracks.iter().any(track_has_swing),
                _ => true,
            };
            if !valid {
                self.push_style_diagnostic(
                    "rhythm_concept",
                    "ML_STYLE_RHYTHM_CONCEPT",
                    format!("score rhythm does not satisfy required {concept} concept"),
                    self.program.score.line,
                    self.program.score.column,
                );
                return;
            }
        }
    }

    fn check_form(&mut self) {
        let Some(form) = self.style.form.clone() else {
            return;
        };
        if self.has_override("form") || self.has_score_override("form") {
            return;
        }
        if !form_labels_match_catalog(&self.section_labels, &form) {
            self.push_style_diagnostic(
                "form",
                "ML_STYLE_FORM",
                format!("score sections do not satisfy `{form}` form"),
                self.program.score.line,
                self.program.score.column,
            );
        }
    }

    fn check_harmonic_progression(&mut self, tracks: &[TrackIr]) {
        if self.style.harmonic_progression.is_empty()
            || self.has_override("harmonic_progression")
            || self.has_score_override("harmonic_progression")
        {
            return;
        }
        let actual = harmonic_functions(tracks);
        if actual.len() < self.style.harmonic_progression.len() {
            return;
        }
        let expected = self.style.harmonic_progression.as_slice();
        if !actual
            .windows(expected.len())
            .any(|window| window == expected)
        {
            self.push_style_diagnostic(
                "harmonic_progression",
                "ML_STYLE_HARMONIC_PROGRESSION",
                format!(
                    "score does not contain required harmonic progression `{}`",
                    expected.join(" ")
                ),
                self.program.score.line,
                self.program.score.column,
            );
        }
    }

    fn check_cadence(&mut self, tracks: &[TrackIr]) {
        if self.style.cadences.is_empty()
            || self.has_override("cadence")
            || self.has_score_override("cadence")
        {
            return;
        }
        let sonorities = final_sonorities(tracks);
        let Some((penultimate, final_sonority)) = sonorities else {
            return;
        };
        if !self
            .style
            .cadences
            .iter()
            .any(|cadence| cadence_matches(cadence, &penultimate, &final_sonority))
        {
            self.push_style_diagnostic(
                "cadence",
                "ML_STYLE_CADENCE",
                format!(
                    "score ending does not satisfy cadence candidates `{}`",
                    self.style.cadences.join(" ")
                ),
                self.program.score.line,
                self.program.score.column,
            );
        }
    }

    fn check_score_style(&mut self, tempo_bpm: u16, meter: Option<Meter>) {
        if let Some((min, max)) = self.style.tempo_range {
            if (tempo_bpm < min || tempo_bpm > max) && !self.has_override("tempo_range") {
                self.push_style_diagnostic(
                    "tempo_range",
                    "ML_STYLE_TEMPO_RANGE",
                    format!("tempo {tempo_bpm} is outside active style tempo range {min}..={max}"),
                    self.program.score.line,
                    self.program.score.column,
                );
            }
        }
        if let (Some(expected), Some(actual)) = (self.style.meter, meter) {
            if expected != actual && !self.has_override("meter") {
                self.push_style_diagnostic(
                    "meter",
                    "ML_STYLE_METER",
                    format!(
                        "meter {}/{} does not match active style meter {}/{}",
                        actual.numerator,
                        actual.denominator,
                        expected.numerator,
                        expected.denominator
                    ),
                    self.program.score.line,
                    self.program.score.column,
                );
            }
        }
        if let Some(actual) = meter {
            if !self.style.meter_catalog.is_empty()
                && !self.has_override("meter_catalog")
                && !self
                    .style
                    .meter_catalog
                    .iter()
                    .any(|entry_id| meter_matches_catalog(actual, entry_id))
            {
                self.push_style_diagnostic(
                    "meter_catalog",
                    "ML_STYLE_METER_CATALOG",
                    format!(
                        "meter {}/{} is outside active style meter catalog",
                        actual.numerator, actual.denominator
                    ),
                    self.program.score.line,
                    self.program.score.column,
                );
            }
        }
    }

    fn compile_override_tracks(&mut self, override_stmt: &OverrideStmt, tracks: &mut Vec<TrackIr>) {
        if !self.is_known_rule(&override_stmt.rule) {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_STYLE_UNKNOWN_RULE",
                    format!("unknown style rule `{}`", override_stmt.rule),
                    override_stmt.line,
                    override_stmt.column,
                )
                .with_span(override_stmt.span)
                .with_rule(override_stmt.rule.clone())
                .with_style(self.style.name.clone()),
            );
            return;
        }
        self.override_rules.push(override_stmt.rule.clone());
        self.score_override_rules.insert(override_stmt.rule.clone());
        self.override_traces.push(OverrideTrace {
            rule: override_stmt.rule.clone(),
            reason: override_stmt.reason.clone(),
            line: override_stmt.line,
            column: override_stmt.column,
        });
        for statement in &override_stmt.statements {
            match statement {
                Stmt::Voice(voice) => {
                    let mut track = TrackBuilder::new(&voice.name, voice.program);
                    self.compile_voice(voice, &mut track);
                    tracks.push(track.finish());
                }
                other => {
                    let mut track = TrackBuilder::new("main", None);
                    self.compile_statement(other, &mut track);
                    if !track.events.is_empty() {
                        tracks.push(track.finish());
                    }
                }
            }
        }
        self.override_rules.pop();
    }

    fn compile_override(&mut self, override_stmt: &OverrideStmt, track: &mut TrackBuilder) {
        if !self.is_known_rule(&override_stmt.rule) {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_STYLE_UNKNOWN_RULE",
                    format!("unknown style rule `{}`", override_stmt.rule),
                    override_stmt.line,
                    override_stmt.column,
                )
                .with_span(override_stmt.span)
                .with_rule(override_stmt.rule.clone())
                .with_style(self.style.name.clone()),
            );
            return;
        }

        self.override_rules.push(override_stmt.rule.clone());
        self.override_traces.push(OverrideTrace {
            rule: override_stmt.rule.clone(),
            reason: override_stmt.reason.clone(),
            line: override_stmt.line,
            column: override_stmt.column,
        });
        let start_event = track.event_count();
        self.compile_statements(&override_stmt.statements, track);
        track.mark_rule_override(start_event, &override_stmt.rule);
        self.override_rules.pop();
    }

    fn eval_expr(&mut self, expr: &Expr, line: usize, column: usize) -> Option<Value> {
        match expr {
            Expr::Ident(name) => self.resolve_token(name).or_else(|| {
                self.diagnostics.push(Diagnostic::error(
                    "ML_RESOLVE_UNKNOWN_NAME",
                    format!("unknown name `{name}`"),
                    line,
                    column,
                ));
                None
            }),
            Expr::Int(value) => Some(Value::Int(*value)),
            Expr::Bool(value) => Some(Value::Bool(*value)),
            Expr::PitchLiteral(value) => match value.parse() {
                Ok(pitch) => Some(Value::Pitch(pitch)),
                Err(error) => {
                    self.diagnostics.push(Diagnostic::error(
                        "ML_CORE_PITCH",
                        error.to_string(),
                        line,
                        column,
                    ));
                    None
                }
            },
            Expr::IntervalLiteral(value) => match value.parse() {
                Ok(interval) => Some(Value::Interval(interval)),
                Err(error) => {
                    self.diagnostics.push(Diagnostic::error(
                        "ML_CORE_INTERVAL",
                        error.to_string(),
                        line,
                        column,
                    ));
                    None
                }
            },
            Expr::DurationLiteral(value) => match value.parse() {
                Ok(duration) => Some(Value::Duration(duration)),
                Err(error) => {
                    self.diagnostics.push(Diagnostic::error(
                        "ML_CORE_DURATION",
                        error.to_string(),
                        line,
                        column,
                    ));
                    None
                }
            },
            Expr::StringLiteral(value) => Some(Value::String(value.clone())),
            Expr::List(values) => values
                .iter()
                .map(|value| self.eval_expr(value, line, column))
                .collect::<Option<Vec<_>>>()
                .map(Value::List),
            Expr::Call { callee, args } => {
                let args = args
                    .iter()
                    .map(|arg| self.eval_expr(arg, line, column))
                    .collect::<Option<Vec<_>>>()?;
                self.eval_call(callee, args, line, column)
            }
            Expr::Binary { op, left, right } => {
                let left = self.eval_expr(left, line, column)?;
                let right = self.eval_expr(right, line, column)?;
                self.eval_binary(*op, left, right, line, column)
            }
        }
    }

    fn eval_call(
        &mut self,
        callee: &str,
        args: Vec<Value>,
        line: usize,
        column: usize,
    ) -> Option<Value> {
        match (callee, args.as_slice()) {
            ("transpose", [Value::Pitch(pitch), Value::Interval(interval)]) => {
                match pitch.transpose(*interval) {
                    Ok(pitch) => Some(Value::Pitch(pitch)),
                    Err(error) => {
                        self.diagnostics.push(Diagnostic::error(
                            "ML_EVAL_UNSUPPORTED_OP",
                            error.to_string(),
                            line,
                            column,
                        ));
                        None
                    }
                }
            }
            ("duration", [Value::String(value)]) => match value.parse() {
                Ok(duration) => Some(Value::Duration(duration)),
                Err(error) => {
                    self.diagnostics.push(Diagnostic::error(
                        "ML_CORE_DURATION",
                        error.to_string(),
                        line,
                        column,
                    ));
                    None
                }
            },
            ("pitch", [Value::String(value)]) => match value.parse() {
                Ok(pitch) => Some(Value::Pitch(pitch)),
                Err(error) => {
                    self.diagnostics.push(Diagnostic::error(
                        "ML_CORE_PITCH",
                        error.to_string(),
                        line,
                        column,
                    ));
                    None
                }
            },
            ("first", [Value::List(values)]) => values.first().cloned().or_else(|| {
                self.diagnostics.push(Diagnostic::error(
                    "ML_TYPE_MISMATCH",
                    "expected non-empty list",
                    line,
                    column,
                ));
                None
            }),
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    "ML_EVAL_UNSUPPORTED_OP",
                    format!("unsupported call `{callee}`"),
                    line,
                    column,
                ));
                None
            }
        }
    }

    fn eval_binary(
        &mut self,
        op: BinaryOp,
        left: Value,
        right: Value,
        line: usize,
        column: usize,
    ) -> Option<Value> {
        match (op, left, right) {
            (BinaryOp::Add, Value::Pitch(pitch), Value::Interval(interval)) => {
                match pitch + interval {
                    Ok(pitch) => Some(Value::Pitch(pitch)),
                    Err(error) => {
                        self.diagnostics.push(Diagnostic::error(
                            "ML_EVAL_UNSUPPORTED_OP",
                            error.to_string(),
                            line,
                            column,
                        ));
                        None
                    }
                }
            }
            (BinaryOp::Sub, Value::Pitch(pitch), Value::Interval(interval)) => {
                match pitch - interval {
                    Ok(pitch) => Some(Value::Pitch(pitch)),
                    Err(error) => {
                        self.diagnostics.push(Diagnostic::error(
                            "ML_EVAL_UNSUPPORTED_OP",
                            error.to_string(),
                            line,
                            column,
                        ));
                        None
                    }
                }
            }
            (BinaryOp::Eq, Value::Int(left), Value::Int(right)) => Some(Value::Bool(left == right)),
            (BinaryOp::Eq, Value::Bool(left), Value::Bool(right)) => {
                Some(Value::Bool(left == right))
            }
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    "ML_TYPE_MISMATCH",
                    "unsupported expression operand types",
                    line,
                    column,
                ));
                None
            }
        }
    }

    fn eval_pitch(&mut self, expr: &Expr, line: usize, column: usize) -> Option<Pitch> {
        match self.eval_expr(expr, line, column)? {
            Value::Pitch(pitch) => Some(pitch),
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    "ML_TYPE_MISMATCH",
                    "expected pitch expression",
                    line,
                    column,
                ));
                None
            }
        }
    }

    fn eval_duration(&mut self, expr: &Expr, line: usize, column: usize) -> Option<Duration> {
        match self.eval_expr(expr, line, column)? {
            Value::Duration(duration) => Some(duration),
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    "ML_TYPE_MISMATCH",
                    "expected duration expression",
                    line,
                    column,
                ));
                None
            }
        }
    }

    fn eval_bool(&mut self, expr: &Expr, line: usize, column: usize) -> Option<bool> {
        match self.eval_expr(expr, line, column)? {
            Value::Bool(value) => Some(value),
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    "ML_TYPE_MISMATCH",
                    "expected bool expression",
                    line,
                    column,
                ));
                None
            }
        }
    }

    fn check_pitch_style(&mut self, pitch: Pitch, line: usize, column: usize, span: Option<Span>) {
        if self.style.allows_pitch(pitch) || self.has_override("scale") {
            return;
        }

        self.push_style_diagnostic_with_span(
            "scale",
            "ML_STYLE_SCALE",
            format!("pitch {pitch} is outside active style scale"),
            line,
            column,
            span,
        );
    }

    fn check_chord_vocab(
        &mut self,
        pitches: &[Pitch],
        line: usize,
        column: usize,
        span: Option<Span>,
    ) {
        if self.style.chord_vocab.is_empty() || self.has_override("chord_vocab") {
            return;
        }
        let classes = pitches
            .iter()
            .map(|pitch| pitch.class())
            .collect::<Vec<_>>();
        let allowed = self.style.chord_vocab.iter().any(|vocab| {
            vocab.len() == classes.len() && vocab.iter().all(|class| classes.contains(class))
        });
        if !allowed {
            self.push_style_diagnostic_with_span(
                "chord_vocab",
                "ML_STYLE_CHORD_VOCAB",
                "chord is outside active style vocabulary".to_string(),
                line,
                column,
                span,
            );
        }
    }

    fn check_chord_quality_vocab(
        &mut self,
        pitches: &[Pitch],
        line: usize,
        column: usize,
        span: Option<Span>,
    ) {
        if self.style.chord_quality_vocab.is_empty() || self.has_override("chord_quality_vocab") {
            return;
        }
        let allowed = self
            .style
            .chord_quality_vocab
            .iter()
            .any(|quality| chord_matches_quality(pitches, quality));
        if !allowed {
            self.push_style_diagnostic_with_span(
                "chord_quality_vocab",
                "ML_STYLE_CHORD_QUALITY_VOCAB",
                "chord quality is outside active style vocabulary".to_string(),
                line,
                column,
                span,
            );
        }
    }

    fn check_set_class_vocab(
        &mut self,
        pitches: &[Pitch],
        line: usize,
        column: usize,
        span: Option<Span>,
    ) {
        if self.style.set_class_vocab.is_empty() || self.has_override("set_class_vocab") {
            return;
        }
        let allowed = self
            .style
            .set_class_vocab
            .iter()
            .any(|set_class| chord_matches_set_class(pitches, set_class));
        if !allowed {
            self.push_style_diagnostic_with_span(
                "set_class_vocab",
                "ML_STYLE_SET_CLASS_VOCAB",
                "chord set class is outside active style vocabulary".to_string(),
                line,
                column,
                span,
            );
        }
    }

    fn check_rhythm_vocab(
        &mut self,
        duration: Duration,
        line: usize,
        column: usize,
        span: Option<Span>,
    ) {
        if self.style.rhythm_vocab.is_empty() || self.has_override("rhythm_vocab") {
            return;
        }
        if !self.style.rhythm_vocab.contains(&duration) {
            self.push_style_diagnostic_with_span(
                "rhythm_vocab",
                "ML_STYLE_RHYTHM_VOCAB",
                format!(
                    "duration {}/{} is outside active style rhythm vocabulary",
                    duration.numerator(),
                    duration.denominator()
                ),
                line,
                column,
                span,
            );
        }
    }

    fn check_instrument_range(
        &mut self,
        program: Option<u8>,
        pitch: Pitch,
        line: usize,
        column: usize,
        span: Option<Span>,
    ) {
        let Some(program) = program else {
            return;
        };
        if self.has_override("instrument_range") {
            return;
        }
        let Some(range) = self
            .style
            .instrument_ranges
            .iter()
            .find(|range| range.program == program)
        else {
            return;
        };
        let pitch_midi = pitch.midi_number();
        let low = range.low.midi_number();
        let high = range.high.midi_number();
        if let (Ok(pitch_midi), Ok(low), Ok(high)) = (pitch_midi, low, high) {
            if pitch_midi < low || pitch_midi > high {
                self.push_style_diagnostic_with_span(
                    "instrument_range",
                    "ML_STYLE_INSTRUMENT_RANGE",
                    format!("pitch {pitch} is outside program {program} range"),
                    line,
                    column,
                    span,
                );
            }
        }
    }

    fn style_context_by_name(
        &mut self,
        name: &str,
        line: usize,
        column: usize,
        span: Option<Span>,
    ) -> Option<StyleContext> {
        let Some(style) = self.program.styles.iter().find(|style| style.name == name) else {
            let mut diagnostic = Diagnostic::error(
                "ML_STYLE_UNKNOWN_NAME",
                format!("unknown style `{name}`"),
                line,
                column,
            );
            if let Some(span) = span {
                diagnostic = diagnostic.with_span(span);
            }
            self.diagnostics.push(diagnostic);
            return None;
        };
        let (style, diagnostics) = style_from_program(&self.program, style);
        self.diagnostics.extend(diagnostics);
        Some(style)
    }

    fn is_known_rule(&self, rule: &str) -> bool {
        stylecheck::known_rule(rule)
            || self
                .style
                .custom_rules
                .iter()
                .any(|custom| custom.id == rule)
    }

    fn has_override(&self, rule: &str) -> bool {
        let rules = self.override_rules.iter().collect::<HashSet<_>>();
        rules.contains(&rule.to_string())
    }

    fn has_score_override(&self, rule: &str) -> bool {
        self.score_override_rules.contains(rule)
    }

    fn push_scope(&mut self) {
        self.variables.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.variables.pop();
    }

    fn set_var(&mut self, name: &str, value: Value) {
        if let Some(scope) = self.variables.last_mut() {
            scope.insert(name.to_string(), value);
        }
    }

    fn resolve_token(&self, token: &str) -> Option<Value> {
        self.variables
            .iter()
            .rev()
            .find_map(|scope| scope.get(token).cloned())
    }
}

fn style_from_program(program: &Program, style: &StyleDecl) -> (StyleContext, Vec<Diagnostic>) {
    style_from_program_inner(program, style, &mut Vec::new())
}

fn style_from_program_inner(
    program: &Program,
    style: &StyleDecl,
    visiting: &mut Vec<String>,
) -> (StyleContext, Vec<Diagnostic>) {
    if visiting.iter().any(|name| name == &style.name) {
        return (
            StyleContext::named(&style.name),
            vec![Diagnostic::error(
                "ML_STYLE_INHERITANCE_CYCLE",
                format!("style inheritance cycle at `{}`", style.name),
                style.line,
                style.column,
            )
            .with_span(style.span)],
        );
    }
    visiting.push(style.name.clone());
    let (mut context, mut diagnostics) = if let Some(parent_name) = &style.parent {
        if let Some(parent) = program
            .styles
            .iter()
            .find(|candidate| &candidate.name == parent_name)
        {
            style_from_program_inner(program, parent, visiting)
        } else {
            (
                StyleContext::named(&style.name),
                vec![Diagnostic::error(
                    "ML_STYLE_UNKNOWN_NAME",
                    format!("unknown parent style `{parent_name}`"),
                    style.line,
                    style.column,
                )
                .with_span(style.span)],
            )
        }
    } else {
        (StyleContext::named(&style.name), Vec::new())
    };
    visiting.pop();
    for entry in &style.entries {
        if let Some(domain_name) = entry.key.strip_prefix("theory_") {
            context.custom_theory.push(CustomTheoryDomain {
                name: domain_name.to_string(),
                entries: entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect(),
            });
        } else if let Some(rule_id) = entry.key.strip_prefix("rule_") {
            context.custom_rules.push(CustomStyleRule {
                id: rule_id.to_string(),
                description: entry.value.clone(),
            });
        } else if let Some(rule_id) = entry.key.strip_prefix("severity_") {
            if let Some(severity) = parse_rule_severity(&entry.value) {
                context.rule_severity.insert(rule_id.to_string(), severity);
            }
        }
    }
    for entry in &style.entries {
        if entry.key.starts_with("theory_")
            || entry.key.starts_with("rule_")
            || entry.key.starts_with("severity_")
        {
            continue;
        }
        match entry.key.as_str() {
            "scale" => {
                let classes = entry
                    .value
                    .split_whitespace()
                    .filter_map(|value| value.parse::<PitchClass>().ok())
                    .collect::<BTreeSet<_>>();
                if !classes.is_empty() {
                    context.allowed_pitch_classes = Some(classes);
                }
            }
            "scale_pattern" => {
                if let Some(classes) = parse_scale_pattern(&entry.value) {
                    context.allowed_pitch_classes = Some(classes);
                }
            }
            "mode_pattern" => {
                if let Some(classes) = parse_mode_pattern(&entry.value) {
                    context.allowed_pitch_classes = Some(classes);
                }
            }
            "chord_vocab" => {
                context.chord_vocab = entry
                    .value
                    .split(';')
                    .filter_map(parse_chord_classes)
                    .collect();
            }
            "chord_quality_vocab" => {
                context.chord_quality_vocab = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
            }
            "set_class_vocab" => {
                context.set_class_vocab = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(style, entry, TheoryDomain::SetClasses, &mut diagnostics);
            }
            "rhythm_vocab" => {
                context.rhythm_vocab = entry
                    .value
                    .split_whitespace()
                    .filter_map(|value| value.parse::<Duration>().ok())
                    .collect();
            }
            "rhythm_concept" => {
                context.rhythm_concepts = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(style, entry, TheoryDomain::Rhythms, &mut diagnostics);
            }
            "dynamic_vocab" => {
                context.dynamic_vocab = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(style, entry, TheoryDomain::Dynamics, &mut diagnostics);
            }
            "articulation_vocab" => {
                context.articulation_vocab = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(style, entry, TheoryDomain::Ornaments, &mut diagnostics);
            }
            "ornament" => {
                context.ornaments = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(style, entry, TheoryDomain::Ornaments, &mut diagnostics);
            }
            "non_chord_tone" => {
                context.non_chord_tones = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(style, entry, TheoryDomain::NonChordTones, &mut diagnostics);
            }
            "tuning_system" => {
                context.tuning_systems = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(style, entry, TheoryDomain::TuningSystems, &mut diagnostics);
            }
            "world_tradition" => {
                context.world_traditions = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(
                    style,
                    entry,
                    TheoryDomain::WorldTraditions,
                    &mut diagnostics,
                );
            }
            "historical_era" => {
                context.historical_eras = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(style, entry, TheoryDomain::StyleEras, &mut diagnostics);
            }
            "harmonic_function" => {
                context.harmonic_functions = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(
                    style,
                    entry,
                    TheoryDomain::HarmonicFunctions,
                    &mut diagnostics,
                );
            }
            "max_melodic_leap" => {
                context.max_melodic_leap = entry.value.trim().parse::<Interval>().ok();
            }
            "contrapuntal_motion" => {
                context.contrapuntal_motion = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(
                    style,
                    entry,
                    TheoryDomain::ContrapuntalMotions,
                    &mut diagnostics,
                );
            }
            "cadence" => {
                context.cadences = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(style, entry, TheoryDomain::Cadences, &mut diagnostics);
            }
            "harmonic_progression" => {
                context.harmonic_progression = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(
                    style,
                    entry,
                    TheoryDomain::HarmonicFunctions,
                    &mut diagnostics,
                );
            }
            "texture" => {
                context.texture = Some(entry.value.trim().to_string());
                validate_vocab_entries(style, entry, TheoryDomain::Textures, &mut diagnostics);
            }
            "form" => {
                context.form = Some(entry.value.trim().to_string());
                validate_vocab_entries(style, entry, TheoryDomain::Forms, &mut diagnostics);
            }
            "meter" => {
                if let Some((numerator, denominator)) = parse_meter(&entry.value) {
                    context.meter = Some(Meter {
                        numerator,
                        denominator,
                    });
                }
            }
            "meter_catalog" => {
                context.meter_catalog = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_vocab_entries(style, entry, TheoryDomain::Meters, &mut diagnostics);
            }
            "tempo_range" => {
                if let Some((min, max)) = entry.value.split_once("..") {
                    if let (Ok(min), Ok(max)) = (min.trim().parse(), max.trim().parse()) {
                        context.tempo_range = Some((min, max));
                    }
                }
            }
            "instrument_range" => {
                if let Some(range) = parse_instrument_range(&entry.value) {
                    context.instrument_ranges.push(range);
                }
            }
            _ => {
                if let Some(domain) = parse_theory_domain(&entry.key) {
                    validate_builtin_theory_references(
                        style,
                        entry,
                        domain,
                        &mut context,
                        &mut diagnostics,
                    );
                } else if custom_theory_domain_exists(&context, &entry.key) {
                    validate_custom_theory_references(style, entry, &mut context, &mut diagnostics);
                } else {
                    diagnostics.push(
                        Diagnostic::error(
                            "ML_STYLE_UNKNOWN_KEY",
                            format!("unknown style key `{}`", entry.key),
                            style.line,
                            style.column,
                        )
                        .with_span(style.span)
                        .with_style(style.name.clone()),
                    );
                }
            }
        }
    }
    context.name = style.name.clone();
    (context, diagnostics)
}

fn validate_vocab_entries(
    style: &StyleDecl,
    entry: &musiclang_parser::StyleEntry,
    domain: TheoryDomain,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry_id in entry.value.split_whitespace() {
        if !theory_entry_exists(domain, entry_id) {
            diagnostics.push(
                Diagnostic::error(
                    "ML_STYLE_UNKNOWN_THEORY_ENTRY",
                    format!("unknown theory entry `{entry_id}` in domain `{domain}`"),
                    style.line,
                    style.column,
                )
                .with_span(style.span)
                .with_rule(entry.key.clone())
                .with_style(style.name.clone()),
            );
        }
    }
}

fn validate_builtin_theory_references(
    style: &StyleDecl,
    entry: &musiclang_parser::StyleEntry,
    domain: TheoryDomain,
    context: &mut StyleContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry_id in entry.value.split_whitespace() {
        if theory_entry_exists(domain, entry_id) {
            context.theory.push(TheoryReference {
                domain: domain.to_string(),
                entry_id: entry_id.to_string(),
            });
        } else {
            diagnostics.push(
                Diagnostic::error(
                    "ML_STYLE_UNKNOWN_THEORY_ENTRY",
                    format!("unknown theory entry `{entry_id}` in domain `{domain}`"),
                    style.line,
                    style.column,
                )
                .with_span(style.span)
                .with_rule(entry.key.clone())
                .with_style(style.name.clone()),
            );
        }
    }
}

fn validate_custom_theory_references(
    style: &StyleDecl,
    entry: &musiclang_parser::StyleEntry,
    context: &mut StyleContext,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(domain) = context
        .custom_theory
        .iter()
        .find(|domain| domain.name == entry.key)
    else {
        return;
    };
    let known_entries = domain.entries.clone();
    for entry_id in entry.value.split_whitespace() {
        if known_entries.iter().any(|known| known == entry_id) {
            context.theory.push(TheoryReference {
                domain: entry.key.clone(),
                entry_id: entry_id.to_string(),
            });
        } else {
            diagnostics.push(
                Diagnostic::error(
                    "ML_STYLE_UNKNOWN_THEORY_ENTRY",
                    format!(
                        "unknown theory entry `{entry_id}` in custom domain `{}`",
                        entry.key
                    ),
                    style.line,
                    style.column,
                )
                .with_span(style.span)
                .with_rule(entry.key.clone())
                .with_style(style.name.clone()),
            );
        }
    }
}

fn custom_theory_domain_exists(context: &StyleContext, domain_name: &str) -> bool {
    context
        .custom_theory
        .iter()
        .any(|domain| domain.name == domain_name)
}

fn parse_theory_domain(value: &str) -> Option<TheoryDomain> {
    value.parse().ok()
}

fn tracks_share_attack_grid(tracks: &[TrackIr]) -> bool {
    let mut non_empty = tracks.iter().filter(|track| !track.events.is_empty());
    let Some(first) = non_empty.next() else {
        return true;
    };
    let first_grid = attack_grid(first);
    non_empty.all(|track| attack_grid(track) == first_grid)
}

fn tracks_are_heterophonic(tracks: &[TrackIr]) -> bool {
    let non_empty = tracks
        .iter()
        .filter(|track| !track.events.is_empty())
        .collect::<Vec<_>>();
    let Some(first) = non_empty.first() else {
        return false;
    };
    non_empty.len() >= 2
        && non_empty.iter().all(|track| {
            attack_grid(track) == attack_grid(first) && track.events.len() == first.events.len()
        })
}

fn attack_grid(track: &TrackIr) -> Vec<u32> {
    let mut ticks = track
        .events
        .iter()
        .map(|event| event.start_tick)
        .collect::<Vec<_>>();
    ticks.sort_unstable();
    ticks.dedup();
    ticks
}

fn track_has_repeated_duration_cell(track: &TrackIr) -> bool {
    let durations = track
        .events
        .iter()
        .map(|event| event.duration_ticks)
        .collect::<Vec<_>>();
    durations.len() >= 4
        && durations.windows(2).any(|cell| {
            durations
                .windows(2)
                .filter(|candidate| *candidate == cell)
                .count()
                >= 2
        })
}

fn track_has_syncopation(track: &TrackIr) -> bool {
    track
        .events
        .iter()
        .any(|event| event.start_tick % DEFAULT_TICKS_PER_QUARTER != 0)
}

fn track_has_hemiola(track: &TrackIr) -> bool {
    track.events.windows(3).any(|events| {
        events[0].duration_ticks == events[1].duration_ticks
            && events[1].duration_ticks == events[2].duration_ticks
            && events.iter().map(|event| event.duration_ticks).sum::<u32>()
                == DEFAULT_TICKS_PER_QUARTER * 2
    })
}

fn track_has_swing(track: &TrackIr) -> bool {
    track
        .events
        .windows(2)
        .any(|events| events[0].duration_ticks == events[1].duration_ticks * 2)
}

fn events_form_trill(events: &[NoteEventIr]) -> bool {
    if events.len() < 3 {
        return false;
    }
    let first = events[0].pitch.class();
    let second = events[1].pitch.class();
    first != second
        && events.iter().enumerate().all(|(index, event)| {
            event.pitch.class() == if index % 2 == 0 { first } else { second }
        })
}

fn events_form_mordent(events: &[NoteEventIr]) -> bool {
    if events.len() != 3 {
        return false;
    }
    let main = events[0].pitch.class();
    let neighbor = events[1].pitch.class();
    events[2].pitch.class() == main && pitch_classes_are_neighbors(main, neighbor)
}

fn events_form_turn(events: &[NoteEventIr]) -> bool {
    if events.len() != 4 {
        return false;
    }
    let main = events[1].pitch.class();
    let upper = events[0].pitch.class();
    let lower = events[2].pitch.class();
    events[3].pitch.class() == main
        && upper != lower
        && pitch_classes_are_neighbors(main, upper)
        && pitch_classes_are_neighbors(main, lower)
}

fn pitch_classes_are_neighbors(first: PitchClass, second: PitchClass) -> bool {
    let distance = (first.semitone() - second.semitone()).abs();
    distance == 1 || distance == 2 || distance == 10 || distance == 11
}

fn events_form_passing_tone(
    previous: Option<&NoteEventIr>,
    tones: &[NoteEventIr],
    next: Option<&NoteEventIr>,
) -> bool {
    let (Some(previous), [tone], Some(next)) = (previous, tones, next) else {
        return false;
    };
    let Ok(previous_pitch) = previous.pitch.midi_number().map(i16::from) else {
        return false;
    };
    let Ok(tone_pitch) = tone.pitch.midi_number().map(i16::from) else {
        return false;
    };
    let Ok(next_pitch) = next.pitch.midi_number().map(i16::from) else {
        return false;
    };
    let first_step = tone_pitch - previous_pitch;
    let second_step = next_pitch - tone_pitch;
    first_step.signum() == second_step.signum()
        && pitch_distance_is_step(first_step)
        && pitch_distance_is_step(second_step)
}

fn events_form_neighbor_tone(
    previous: Option<&NoteEventIr>,
    tones: &[NoteEventIr],
    next: Option<&NoteEventIr>,
) -> bool {
    let (Some(previous), [tone], Some(next)) = (previous, tones, next) else {
        return false;
    };
    previous.pitch == next.pitch
        && pitch_classes_are_neighbors(previous.pitch.class(), tone.pitch.class())
}

fn pitch_distance_is_step(distance: i16) -> bool {
    matches!(distance.abs(), 1 | 2)
}

fn cadence_matches(
    cadence: &str,
    penultimate: &[PitchClass],
    final_sonority: &[PitchClass],
) -> bool {
    match cadence {
        "authentic" => {
            contains_classes(penultimate, &[PitchClass::G, PitchClass::B, PitchClass::D])
                && contains_classes(
                    final_sonority,
                    &[PitchClass::C, PitchClass::E, PitchClass::G],
                )
        }
        "plagal" => {
            contains_classes(penultimate, &[PitchClass::F, PitchClass::A, PitchClass::C])
                && contains_classes(
                    final_sonority,
                    &[PitchClass::C, PitchClass::E, PitchClass::G],
                )
        }
        "deceptive" => {
            contains_classes(penultimate, &[PitchClass::G, PitchClass::B, PitchClass::D])
                && contains_classes(
                    final_sonority,
                    &[PitchClass::A, PitchClass::C, PitchClass::E],
                )
        }
        "half" => contains_classes(
            final_sonority,
            &[PitchClass::G, PitchClass::B, PitchClass::D],
        ),
        _ => true,
    }
}

fn harmonic_functions(tracks: &[TrackIr]) -> Vec<String> {
    sonority_sequence(tracks)
        .into_iter()
        .filter_map(|classes| harmonic_function(&classes).map(ToString::to_string))
        .collect()
}

fn harmonic_function(classes: &[PitchClass]) -> Option<&'static str> {
    if contains_classes(classes, &[PitchClass::C, PitchClass::E, PitchClass::G]) {
        Some("tonic")
    } else if contains_classes(classes, &[PitchClass::F, PitchClass::A, PitchClass::C])
        || contains_classes(classes, &[PitchClass::D, PitchClass::F, PitchClass::A])
    {
        Some("predominant")
    } else if contains_classes(classes, &[PitchClass::G, PitchClass::B, PitchClass::D])
        || contains_classes(classes, &[PitchClass::B, PitchClass::D, PitchClass::F])
    {
        Some("dominant")
    } else if contains_classes(classes, &[PitchClass::D, PitchClass::Fs, PitchClass::A]) {
        Some("secondary_dominant")
    } else if contains_classes(classes, &[PitchClass::A, PitchClass::C, PitchClass::E]) {
        Some("submediant")
    } else {
        None
    }
}

fn sonority_sequence(tracks: &[TrackIr]) -> Vec<Vec<PitchClass>> {
    let mut ticks = tracks
        .iter()
        .flat_map(|track| track.events.iter().map(|event| event.start_tick))
        .collect::<Vec<_>>();
    ticks.sort_unstable();
    ticks.dedup();
    ticks
        .into_iter()
        .map(|tick| sonority_at(tracks, tick))
        .collect()
}

fn final_sonorities(tracks: &[TrackIr]) -> Option<(Vec<PitchClass>, Vec<PitchClass>)> {
    let mut ticks = tracks
        .iter()
        .flat_map(|track| track.events.iter().map(|event| event.start_tick))
        .collect::<Vec<_>>();
    ticks.sort_unstable();
    ticks.dedup();
    let [.., penultimate_tick, final_tick] = ticks.as_slice() else {
        return None;
    };
    Some((
        sonority_at(tracks, *penultimate_tick),
        sonority_at(tracks, *final_tick),
    ))
}

fn sonority_at(tracks: &[TrackIr], tick: u32) -> Vec<PitchClass> {
    tracks
        .iter()
        .flat_map(|track| &track.events)
        .filter(|event| event.start_tick == tick)
        .map(|event| event.pitch.class())
        .collect()
}

fn contains_classes(actual: &[PitchClass], expected: &[PitchClass]) -> bool {
    expected.iter().all(|class| actual.contains(class))
}

fn motion_type(
    upper_a: &NoteEventIr,
    lower_a: &NoteEventIr,
    upper_b: &NoteEventIr,
    lower_b: &NoteEventIr,
) -> Option<&'static str> {
    let upper_motion =
        i16::from(upper_b.pitch.midi_number().ok()?) - i16::from(upper_a.pitch.midi_number().ok()?);
    let lower_motion =
        i16::from(lower_b.pitch.midi_number().ok()?) - i16::from(lower_a.pitch.midi_number().ok()?);
    if upper_motion == 0 || lower_motion == 0 {
        return Some("oblique");
    }
    if upper_motion.signum() != lower_motion.signum() {
        return Some("contrary");
    }
    if upper_motion == lower_motion {
        return Some("parallel");
    }
    Some("similar")
}

fn parse_rule_severity(value: &str) -> Option<RuleSeverity> {
    match value.trim() {
        "error" => Some(RuleSeverity::Error),
        "warning" | "warn" => Some(RuleSeverity::Warning),
        "off" => Some(RuleSeverity::Off),
        _ => None,
    }
}

fn theory_entry_exists(domain: TheoryDomain, entry_id: &str) -> bool {
    musiclang_core::theory_catalog()
        .entries(domain)
        .iter()
        .any(|entry| entry.id == entry_id)
}

fn parse_chord_classes(value: &str) -> Option<Vec<PitchClass>> {
    let classes = value
        .split_whitespace()
        .filter_map(|value| value.parse::<PitchClass>().ok())
        .collect::<Vec<_>>();
    (!classes.is_empty()).then_some(classes)
}

fn chord_matches_quality(pitches: &[Pitch], quality: &str) -> bool {
    let Some(root) = pitches.first().map(|pitch| pitch.class().semitone()) else {
        return false;
    };
    let intervals = pitches
        .iter()
        .map(|pitch| (pitch.class().semitone() - root).rem_euclid(12).to_string())
        .collect::<BTreeSet<_>>();
    let catalog = musiclang_core::theory_catalog();
    catalog
        .entries(TheoryDomain::ChordQualities)
        .iter()
        .find(|entry| entry.id == quality)
        .is_some_and(|entry| {
            entry.pattern.len() == intervals.len()
                && entry.pattern.iter().all(|step| intervals.contains(*step))
        })
}

fn chord_matches_set_class(pitches: &[Pitch], set_class: &str) -> bool {
    let Some(root) = pitches.first().map(|pitch| pitch.class().semitone()) else {
        return false;
    };
    let normalized = pitches
        .iter()
        .map(|pitch| (pitch.class().semitone() - root).rem_euclid(12).to_string())
        .collect::<BTreeSet<_>>();
    let catalog = musiclang_core::theory_catalog();
    catalog
        .entries(TheoryDomain::SetClasses)
        .iter()
        .find(|entry| entry.id == set_class)
        .is_some_and(|entry| {
            entry.pattern.len() == normalized.len()
                && entry.pattern.iter().all(|step| normalized.contains(*step))
        })
}

fn dynamic_velocity(mark: &str) -> Option<u8> {
    let catalog = musiclang_core::theory_catalog();
    catalog
        .entries(TheoryDomain::Dynamics)
        .iter()
        .find(|entry| entry.id == mark)
        .and_then(|entry| entry.pattern.first())
        .and_then(|value| value.parse::<u8>().ok())
}

fn parse_scale_pattern(value: &str) -> Option<BTreeSet<PitchClass>> {
    parse_catalog_pitch_class_pattern(value, TheoryDomain::Scales)
}

fn parse_mode_pattern(value: &str) -> Option<BTreeSet<PitchClass>> {
    parse_catalog_pitch_class_pattern(value, TheoryDomain::Modes)
}

fn parse_catalog_pitch_class_pattern(
    value: &str,
    domain: TheoryDomain,
) -> Option<BTreeSet<PitchClass>> {
    let mut parts = value.split_whitespace();
    let tonic = parts.next()?.parse::<PitchClass>().ok()?;
    let entry_id = parts.next()?;
    let catalog = musiclang_core::theory_catalog();
    let entry = catalog
        .entries(domain)
        .iter()
        .find(|entry| entry.id == entry_id)?;
    let mut semitone = tonic.semitone();
    let mut classes = BTreeSet::from([tonic]);
    for step in entry.pattern.iter().filter_map(pattern_step_semitones) {
        semitone += step;
        classes.insert(PitchClass::from_semitone(semitone));
    }
    Some(classes)
}

fn pattern_step_semitones(step: &&'static str) -> Option<i16> {
    match *step {
        "W" => Some(2),
        "H" => Some(1),
        value => value.parse::<i16>().ok(),
    }
}

fn parse_meter(value: &str) -> Option<(u8, u8)> {
    let (numerator, denominator) = value.trim().split_once('/')?;
    Some((numerator.parse().ok()?, denominator.parse().ok()?))
}

fn meter_matches_catalog(meter: Meter, entry_id: &str) -> bool {
    let id = format!("{}/{}", meter.numerator, meter.denominator);
    let catalog = musiclang_core::theory_catalog();
    catalog
        .entries(TheoryDomain::Meters)
        .iter()
        .any(|entry| entry.id == entry_id && entry.id == id)
}

fn form_labels_match_catalog(labels: &[String], entry_id: &str) -> bool {
    let catalog = musiclang_core::theory_catalog();
    catalog
        .entries(TheoryDomain::Forms)
        .iter()
        .find(|entry| entry.id == entry_id)
        .is_some_and(|entry| {
            labels
                .iter()
                .map(String::as_str)
                .eq(entry.pattern.iter().copied())
        })
}

fn key_signature(tonic: &str, mode: &str) -> Option<KeySignature> {
    let tonic = tonic.trim();
    let is_minor = matches!(mode.trim(), "minor" | "min" | "aeolian");
    let fifths = match (tonic, is_minor) {
        ("Cb", false) => -7,
        ("Gb", false) => -6,
        ("Db", false) => -5,
        ("Ab", false) => -4,
        ("Eb", false) => -3,
        ("Bb", false) => -2,
        ("F", false) => -1,
        ("C", false) => 0,
        ("G", false) => 1,
        ("D", false) => 2,
        ("A", false) => 3,
        ("E", false) => 4,
        ("B", false) => 5,
        ("F#", false) => 6,
        ("C#", false) => 7,
        ("Ab", true) => -7,
        ("Eb", true) => -6,
        ("Bb", true) => -5,
        ("F", true) => -4,
        ("C", true) => -3,
        ("G", true) => -2,
        ("D", true) => -1,
        ("A", true) => 0,
        ("E", true) => 1,
        ("B", true) => 2,
        ("F#", true) => 3,
        ("C#", true) => 4,
        ("G#", true) => 5,
        ("D#", true) => 6,
        ("A#", true) => 7,
        _ => return None,
    };
    Some(KeySignature { fifths, is_minor })
}

fn parse_instrument_range(value: &str) -> Option<InstrumentRange> {
    let mut parts = value.split_whitespace();
    Some(InstrumentRange {
        program: parts.next()?.parse().ok()?,
        low: parts.next()?.parse().ok()?,
        high: parts.next()?.parse().ok()?,
    })
}

struct TrackBuilder {
    name: String,
    program: Option<u8>,
    cursor_tick: u32,
    velocity: u8,
    articulation: Option<String>,
    events: Vec<NoteEventIr>,
    overridden_event_rules: HashMap<String, HashSet<u32>>,
}

impl TrackBuilder {
    fn new(name: &str, program: Option<u8>) -> Self {
        Self {
            name: name.to_string(),
            program,
            cursor_tick: 0,
            velocity: 80,
            articulation: None,
            events: Vec::new(),
            overridden_event_rules: HashMap::new(),
        }
    }

    fn push_note(&mut self, note: Note, source_span: Option<Span>) {
        let duration_ticks = note.duration().ticks(DEFAULT_TICKS_PER_QUARTER);
        self.events.push(NoteEventIr {
            pitch: note.pitch(),
            start_tick: self.cursor_tick,
            duration_ticks,
            velocity: self.velocity,
            articulation: self.articulation.clone(),
            source_span,
        });
        self.cursor_tick += duration_ticks;
    }

    fn set_velocity(&mut self, velocity: u8) {
        self.velocity = velocity.min(127);
    }

    fn set_articulation(&mut self, articulation: &str) {
        self.articulation = Some(articulation.to_string());
    }

    fn event_count(&self) -> usize {
        self.events.len()
    }

    fn events(&self) -> &[NoteEventIr] {
        &self.events
    }

    fn cursor_tick(&self) -> u32 {
        self.cursor_tick
    }

    fn is_event_overridden(&self, rule: &str, start_tick: u32) -> bool {
        self.overridden_event_rules
            .get(rule)
            .is_some_and(|ticks| ticks.contains(&start_tick))
    }

    fn mark_rule_override(&mut self, start_event: usize, rule: &str) {
        let ticks = self
            .overridden_event_rules
            .entry(rule.to_string())
            .or_default();
        for event in &self.events[start_event..] {
            ticks.insert(event.start_tick);
        }
    }

    fn push_chord(&mut self, chord: Chord, source_span: Option<Span>) {
        let duration_ticks = chord.duration().ticks(DEFAULT_TICKS_PER_QUARTER);
        for pitch in chord.pitches() {
            self.events.push(NoteEventIr {
                pitch: *pitch,
                start_tick: self.cursor_tick,
                duration_ticks,
                velocity: self.velocity,
                articulation: self.articulation.clone(),
                source_span,
            });
        }
        self.cursor_tick += duration_ticks;
    }

    fn finish(self) -> TrackIr {
        TrackIr {
            name: self.name,
            channel: 0,
            program: self.program,
            events: self.events,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_minimal_score_to_ir() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    note C4, 1/4
    chord [C4, E4, G4], 1/2
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.title, "demo");
        assert_eq!(ir.tracks[0].events.len(), 4);
    }

    #[test]
    fn style_violation_fails_without_override() {
        let diagnostics = compile_source(
            r#"
style Classical
score demo {
  voice lead {
    note F#4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_SCALE");
    }

    #[test]
    fn style_diagnostics_include_statement_span() {
        let source = r#"
style Classical
score demo {
  voice lead {
    note F#4, 1/4
  }
}
"#;
        let diagnostics = compile_source(source).unwrap_err();
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "ML_STYLE_SCALE")
            .unwrap();
        let span = diagnostic.span.unwrap();
        let expected_start = source.find("note F#4").unwrap();

        assert_eq!(span.start, expected_start);
        assert_eq!(span.end, expected_start + "note".len());
    }

    #[test]
    fn override_allows_style_violation() {
        let ir = compile_source(
            r#"
style Classical
score demo {
  voice lead {
    override scale allow reason "intentional chromatic color" {
      note F#4, 1/4
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.overrides.len(), 1);
        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn supports_let_for_if_and_function_call() {
        let ir = compile_source(
            r#"
fn motif {
  note C4, 1/8
}
score demo {
  voice lead {
    let d = duration 1/4
    for i in 0..3 {
      if i == 1 {
        call motif
      }
      note E4, d
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 4);
    }

    #[test]
    fn supports_configured_style_scale() {
        let diagnostics = compile_source(
            r#"
style Sparse {
  scale: C E G
}
score demo {
  voice lead {
    note D4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_SCALE");
        assert_eq!(diagnostics[0].style.as_deref(), Some("Sparse"));
    }

    #[test]
    fn scale_pattern_derives_enforced_pitch_classes() {
        let ir = compile_source(
            r#"
style BluesInC {
  scale_pattern: C blues
}
score demo {
  voice lead {
    note F#4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn scale_pattern_rejects_notes_outside_derived_scale() {
        let diagnostics = compile_source(
            r#"
style BluesInC {
  scale_pattern: C blues
}
score demo {
  voice lead {
    note B4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_SCALE");
    }

    #[test]
    fn mode_pattern_derives_enforced_pitch_classes() {
        let ir = compile_source(
            r#"
style DorianOnD {
  mode_pattern: D dorian
}
score demo {
  voice lead {
    note B4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn mode_pattern_rejects_notes_outside_derived_mode() {
        let diagnostics = compile_source(
            r#"
style DorianOnD {
  mode_pattern: D dorian
}
score demo {
  voice lead {
    note Bb4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_SCALE");
    }

    #[test]
    fn supports_pitch_interval_expression() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    note C4 + M3, 1/4
    note E4 - m3, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events[0].pitch, "E4".parse().unwrap());
        assert_eq!(ir.tracks[0].events[1].pitch, "C#4".parse().unwrap());
    }

    #[test]
    fn supports_builtin_expression_calls_and_lists() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    let pitches = [C4, E4, G4]
    note transpose(C4, M3), 1/4
    note first(pitches), 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events[0].pitch, "E4".parse().unwrap());
        assert_eq!(ir.tracks[0].events[1].pitch, "C4".parse().unwrap());
    }

    #[test]
    fn supports_chord_from_pitch_list_expression() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    let pitches = [C4, E4, G4]
    chord [pitches], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 3);
    }

    #[test]
    fn validates_theory_domain_references_in_style() {
        let ir = compile_source(
            r#"
style TheoryRich {
  scales: blues major_pentatonic
  harmonic_functions: tonic dominant secondary_dominant
  world_traditions: maqam hindustani_raga
  set_classes: 016 all_interval_tetrachord
}
score demo {
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn validates_custom_theory_domain_references_in_style() {
        let ir = compile_source(
            r#"
style MicrotonalPractice {
  theory_microgestures: bend flutter split_tone
  microgestures: bend split_tone
}
score demo {
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn invalid_custom_theory_reference_fails() {
        let diagnostics = compile_source(
            r#"
style MicrotonalPractice {
  theory_microgestures: bend flutter
  microgestures: unknown_gesture
}
score demo {
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_UNKNOWN_THEORY_ENTRY");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("microgestures"));
    }

    #[test]
    fn invalid_theory_reference_fails() {
        let diagnostics = compile_source(
            r#"
style TheoryRich {
  harmonic_functions: imaginary_function
}
score demo {
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_UNKNOWN_THEORY_ENTRY");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("harmonic_functions"));
    }

    #[test]
    fn executed_style_rule_inputs_validate_theory_entries() {
        for key in [
            "rhythm_concept",
            "contrapuntal_motion",
            "cadence",
            "harmonic_progression",
            "texture",
            "form",
            "meter_catalog",
        ] {
            let source = format!(
                r#"
style TheoryRich {{
  {key}: imaginary_entry
}}
score demo {{
  voice lead {{
    note C4, 1/4
  }}
}}
"#
            );
            let diagnostics = compile_source(&source).unwrap_err();

            assert_eq!(diagnostics[0].code, "ML_STYLE_UNKNOWN_THEORY_ENTRY");
            assert_eq!(diagnostics[0].rule.as_deref(), Some(key));
        }
    }

    #[test]
    fn unknown_style_key_fails() {
        let diagnostics = compile_source(
            r#"
style TheoryRich {
  imaginary_domain: anything
}
score demo {
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_UNKNOWN_KEY");
    }

    #[test]
    fn unknown_name_uses_stable_diagnostic_code() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice lead {
    note missing, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_RESOLVE_UNKNOWN_NAME");
    }

    #[test]
    fn duplicate_function_uses_stable_diagnostic_code() {
        let source = r#"
fn motif {
  note C4, 1/4
}
fn motif {
  note D4, 1/4
}
score demo {
  voice lead {
    call motif
  }
}
"#;
        let diagnostics = compile_source(source).unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_RESOLVE_DUPLICATE_NAME");
        let span = diagnostics[0].span.unwrap();
        let expected_start = source.rfind("fn motif").unwrap();
        assert_eq!(span.start, expected_start);
        assert_eq!(span.end, expected_start + "fn".len());
    }

    #[test]
    fn recursive_call_uses_stable_diagnostic_code() {
        let diagnostics = compile_source(
            r#"
fn motif {
  call motif
}
score demo {
  voice lead {
    call motif
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_RESOLVE_RECURSIVE_CALL");
    }

    #[test]
    fn indirect_recursive_call_uses_stable_diagnostic_code() {
        let diagnostics = compile_source(
            r#"
fn first {
  call second
}
fn second {
  call first
}
score demo {
  voice lead {
    call first
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_RESOLVE_RECURSIVE_CALL");
    }

    #[test]
    fn type_mismatch_uses_stable_diagnostic_code() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice lead {
    note 1, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_TYPE_MISMATCH");
    }

    #[test]
    fn custom_style_rule_can_be_overridden() {
        let ir = compile_source(
            r#"
style Experimental {
  rule_microtonal_collision: locally defined microtonal voice interaction
}
score demo {
  voice lead {
    override microtonal_collision allow reason "intentional beating" {
      note C4, 1/4
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.overrides[0].rule, "microtonal_collision");
        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn unknown_override_rule_fails() {
        let source = r#"
score demo {
  voice lead {
    override imaginary allow {
      note C4, 1/4
    }
  }
}
"#;
        let diagnostics = compile_source(source).unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_UNKNOWN_RULE");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("imaginary"));
        let span = diagnostics[0].span.unwrap();
        let expected_start = source.find("override imaginary").unwrap();
        assert_eq!(span.start, expected_start);
        assert_eq!(span.end, expected_start + "override".len());
    }

    #[test]
    fn lowers_score_and_voice_metadata() {
        let ir = compile_source(
            r#"
score demo {
  tempo 96
  meter 3/4
  key F major
  voice lead {
    program 40
    note C4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tempo_bpm, 96);
        assert_eq!(ir.meter.unwrap().numerator, 3);
        assert_eq!(ir.key.unwrap().fifths, -1);
        assert!(!ir.key.unwrap().is_minor);
        assert_eq!(ir.tracks[0].program, Some(40));
    }

    #[test]
    fn chord_vocab_rule_fails() {
        let diagnostics = compile_source(
            r#"
style Triads {
  chord_vocab: C E G
}
score demo {
  voice lead {
    chord [C4, D4, G4], 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_CHORD_VOCAB");
    }

    #[test]
    fn chord_quality_vocab_accepts_catalog_quality() {
        let ir = compile_source(
            r#"
style Triads {
  chord_quality_vocab: major minor
}
score demo {
  voice lead {
    chord [C4, E4, G4], 1/4
    chord [D4, F4, A4], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 6);
    }

    #[test]
    fn chord_quality_vocab_rejects_unlisted_quality() {
        let diagnostics = compile_source(
            r#"
style MajorOnly {
  chord_quality_vocab: major
}
score demo {
  voice lead {
    chord [C4, Eb4, G4], 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_CHORD_QUALITY_VOCAB");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("chord_quality_vocab"));
    }

    #[test]
    fn meter_rule_fails() {
        let diagnostics = compile_source(
            r#"
style Three {
  meter: 3/4
}
score demo {
  meter 4/4
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_METER");
    }

    #[test]
    fn meter_catalog_accepts_listed_catalog_meter() {
        let ir = compile_source(
            r#"
style CompoundOrTriple {
  meter_catalog: 3/4 6/8
}
score demo style CompoundOrTriple {
  meter 6/8
  voice lead {
    note C4, 1/8
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.meter.unwrap().numerator, 6);
    }

    #[test]
    fn meter_catalog_rejects_unlisted_meter() {
        let diagnostics = compile_source(
            r#"
style CompoundOnly {
  meter_catalog: 6/8
}
score demo style CompoundOnly {
  meter 4/4
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_METER_CATALOG");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("meter_catalog"));
    }

    #[test]
    fn max_melodic_leap_rule_fails() {
        let diagnostics = compile_source(
            r#"
style Smooth {
  max_melodic_leap: M3
}
score demo style Smooth {
  voice lead {
    note C4, 1/4
    note G4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_MAX_MELODIC_LEAP");
    }

    #[test]
    fn override_allows_max_melodic_leap_violation() {
        let ir = compile_source(
            r#"
style Smooth {
  max_melodic_leap: M3
}
score demo style Smooth {
  voice lead {
    override max_melodic_leap allow reason "registral contrast" {
      note C4, 1/4
      note G4, 1/4
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
    }

    #[test]
    fn max_melodic_leap_rule_can_be_disabled() {
        let ir = compile_source(
            r#"
style Smooth {
  max_melodic_leap: M3
  severity_max_melodic_leap: off
}
score demo style Smooth {
  voice lead {
    note C4, 1/4
    note G4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
    }

    #[test]
    fn rhythm_vocab_rule_fails() {
        let diagnostics = compile_source(
            r#"
style Pulse {
  rhythm_vocab: 1/4 1/8
}
score demo style Pulse {
  voice lead {
    note C4, 1/16
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_RHYTHM_VOCAB");
    }

    #[test]
    fn override_allows_rhythm_vocab_violation() {
        let ir = compile_source(
            r#"
style Pulse {
  rhythm_vocab: 1/4 1/8
}
score demo style Pulse {
  voice lead {
    override rhythm_vocab allow reason "ornamental turn" {
      note C4, 1/16
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn rhythm_vocab_rule_can_be_disabled() {
        let ir = compile_source(
            r#"
style Pulse {
  rhythm_vocab: 1/4 1/8
  severity_rhythm_vocab: off
}
score demo style Pulse {
  voice lead {
    note C4, 1/16
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn set_class_vocab_accepts_catalog_set_class() {
        let ir = compile_source(
            r#"
style PostTonal {
  set_class_vocab: 016
}
score demo style PostTonal {
  voice lead {
    chord [C4, Db4, Gb4], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 3);
    }

    #[test]
    fn set_class_vocab_rejects_unlisted_set_class() {
        let diagnostics = compile_source(
            r#"
style PostTonal {
  set_class_vocab: 016
}
score demo style PostTonal {
  voice lead {
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_SET_CLASS_VOCAB");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("set_class_vocab"));
    }

    #[test]
    fn tuning_system_accepts_catalog_entry() {
        let ir = compile_source(
            r#"
style IntonationPractice {
  tuning_system: just_intonation
}
score demo style IntonationPractice {
  voice lead {
    tuning_system just_intonation {
      note D4, 1/4
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn tuning_system_rejects_unlisted_entry() {
        let diagnostics = compile_source(
            r#"
style IntonationPractice {
  tuning_system: just_intonation
}
score demo style IntonationPractice {
  voice lead {
    tuning_system equal_temperament_12 {
      note D4, 1/4
    }
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_TUNING_SYSTEM");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("tuning_system"));
    }

    #[test]
    fn world_tradition_accepts_catalog_entry() {
        let ir = compile_source(
            r#"
style GlobalPractice {
  world_tradition: maqam
}
score demo style GlobalPractice {
  voice lead {
    world_tradition maqam {
      note D4, 1/4
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn historical_era_accepts_catalog_entry() {
        let ir = compile_source(
            r#"
style PeriodPractice {
  historical_era: baroque
}
score demo style PeriodPractice {
  voice lead {
    historical_era baroque {
      note D4, 1/4
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn historical_era_rejects_unlisted_entry() {
        let diagnostics = compile_source(
            r#"
style PeriodPractice {
  historical_era: baroque
}
score demo style PeriodPractice {
  voice lead {
    historical_era jazz {
      note D4, 1/4
    }
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_HISTORICAL_ERA");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("historical_era"));
    }

    #[test]
    fn harmonic_function_accepts_catalog_entry() {
        let ir = compile_source(
            r#"
style FunctionalPractice {
  harmonic_function: tonic
}
score demo style FunctionalPractice {
  voice lead {
    harmonic_function tonic {
      note D4, 1/4
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn harmonic_function_rejects_unlisted_entry() {
        let diagnostics = compile_source(
            r#"
style FunctionalPractice {
  harmonic_function: tonic
}
score demo style FunctionalPractice {
  voice lead {
    harmonic_function dominant {
      note D4, 1/4
    }
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_HARMONIC_FUNCTION");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("harmonic_function"));
    }

    #[test]
    fn world_tradition_rejects_unlisted_entry() {
        let diagnostics = compile_source(
            r#"
style GlobalPractice {
  world_tradition: maqam
}
score demo style GlobalPractice {
  voice lead {
    world_tradition hindustani_raga {
      note D4, 1/4
    }
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_WORLD_TRADITION");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("world_tradition"));
    }

    #[test]
    fn rhythm_concept_ostinato_accepts_repeated_duration_cell() {
        let ir = compile_source(
            r#"
style Patterned {
  rhythm_concept: ostinato
}
score demo style Patterned {
  voice lead {
    note C4, 1/4
    note D4, 1/8
    note E4, 1/4
    note F4, 1/8
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 4);
    }

    #[test]
    fn rhythm_concept_ostinato_rejects_non_repeating_rhythm() {
        let diagnostics = compile_source(
            r#"
style Patterned {
  rhythm_concept: ostinato
}
score demo style Patterned {
  voice lead {
    note C4, 1/4
    note D4, 1/8
    note E4, 1/16
    note F4, 1/2
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_RHYTHM_CONCEPT");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("rhythm_concept"));
    }

    #[test]
    fn rhythm_concept_syncopation_accepts_offbeat_attack() {
        let ir = compile_source(
            r#"
style Syncopated {
  rhythm_concept: syncopation
}
score demo style Syncopated {
  voice lead {
    note C4, 1/8
    note D4, 1/8
    note E4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 3);
    }

    #[test]
    fn rhythm_concept_syncopation_rejects_onbeat_attacks() {
        let diagnostics = compile_source(
            r#"
style Syncopated {
  rhythm_concept: syncopation
}
score demo style Syncopated {
  voice lead {
    note C4, 1/4
    note D4, 1/4
    note E4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_RHYTHM_CONCEPT");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("rhythm_concept"));
    }

    #[test]
    fn rhythm_concept_hemiola_accepts_three_in_two_pattern() {
        let ir = compile_source(
            r#"
style CrossRhythm {
  rhythm_concept: hemiola
}
score demo style CrossRhythm {
  voice lead {
    note C4, 1/6
    note D4, 1/6
    note E4, 1/6
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 3);
    }

    #[test]
    fn rhythm_concept_hemiola_rejects_plain_quarters() {
        let diagnostics = compile_source(
            r#"
style CrossRhythm {
  rhythm_concept: hemiola
}
score demo style CrossRhythm {
  voice lead {
    note C4, 1/4
    note D4, 1/4
    note E4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_RHYTHM_CONCEPT");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("rhythm_concept"));
    }

    #[test]
    fn rhythm_concept_swing_accepts_long_short_pair() {
        let ir = compile_source(
            r#"
style SwingFeel {
  rhythm_concept: swing
}
score demo style SwingFeel {
  voice lead {
    note C4, 1/6
    note D4, 1/12
    note E4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 3);
    }

    #[test]
    fn rhythm_concept_swing_rejects_even_durations() {
        let diagnostics = compile_source(
            r#"
style SwingFeel {
  rhythm_concept: swing
}
score demo style SwingFeel {
  voice lead {
    note C4, 1/8
    note D4, 1/8
    note E4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_RHYTHM_CONCEPT");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("rhythm_concept"));
    }

    #[test]
    fn dynamic_vocab_accepts_catalog_mark() {
        let ir = compile_source(
            r#"
style Quiet {
  dynamic_vocab: p mp
}
score demo style Quiet {
  voice lead {
    dynamic p
    note C4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events[0].velocity, 48);
    }

    #[test]
    fn dynamic_vocab_rejects_unlisted_mark() {
        let diagnostics = compile_source(
            r#"
style Quiet {
  dynamic_vocab: p mp
}
score demo style Quiet {
  voice lead {
    dynamic ff
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_DYNAMIC_VOCAB");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("dynamic_vocab"));
    }

    #[test]
    fn articulation_vocab_accepts_catalog_mark() {
        let ir = compile_source(
            r#"
style ShortArticulations {
  articulation_vocab: staccato accent
}
score demo style ShortArticulations {
  voice lead {
    articulation staccato
    note C4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(
            ir.tracks[0].events[0].articulation.as_deref(),
            Some("staccato")
        );
    }

    #[test]
    fn articulation_vocab_rejects_unlisted_mark() {
        let diagnostics = compile_source(
            r#"
style ShortArticulations {
  articulation_vocab: staccato accent
}
score demo style ShortArticulations {
  voice lead {
    articulation legato
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_ARTICULATION_VOCAB");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("articulation_vocab"));
    }

    #[test]
    fn ornament_accepts_catalog_entry() {
        let ir = compile_source(
            r#"
style Ornamented {
  ornament: trill mordent
}
score demo style Ornamented {
  voice lead {
    ornament mordent {
      note C4, 1/16
      note B3, 1/16
      note C4, 1/16
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 3);
    }

    #[test]
    fn ornament_trill_accepts_alternating_pattern() {
        let ir = compile_source(
            r#"
style Ornamented {
  ornament: trill
}
score demo style Ornamented {
  voice lead {
    ornament trill {
      note C4, 1/16
      note D4, 1/16
      note C4, 1/16
      note D4, 1/16
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 4);
    }

    #[test]
    fn ornament_trill_rejects_non_alternating_pattern() {
        let diagnostics = compile_source(
            r#"
style Ornamented {
  ornament: trill
}
score demo style Ornamented {
  voice lead {
    ornament trill {
      note C4, 1/16
      note D4, 1/16
      note E4, 1/16
    }
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_ORNAMENT");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("ornament"));
    }

    #[test]
    fn ornament_mordent_rejects_non_returning_pattern() {
        let diagnostics = compile_source(
            r#"
style Ornamented {
  ornament: mordent
}
score demo style Ornamented {
  voice lead {
    ornament mordent {
      note C4, 1/16
      note B3, 1/16
      note D4, 1/16
    }
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_ORNAMENT");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("ornament"));
    }

    #[test]
    fn ornament_turn_accepts_neighbor_enclosure() {
        let ir = compile_source(
            r#"
style Ornamented {
  ornament: turn
}
score demo style Ornamented {
  voice lead {
    ornament turn {
      note D4, 1/16
      note C4, 1/16
      note B3, 1/16
      note C4, 1/16
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 4);
    }

    #[test]
    fn ornament_turn_rejects_missing_lower_neighbor() {
        let diagnostics = compile_source(
            r#"
style Ornamented {
  ornament: turn
}
score demo style Ornamented {
  voice lead {
    ornament turn {
      note D4, 1/16
      note C4, 1/16
      note E4, 1/16
      note C4, 1/16
    }
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_ORNAMENT");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("ornament"));
    }

    #[test]
    fn ornament_rejects_unlisted_entry() {
        let diagnostics = compile_source(
            r#"
style Ornamented {
  ornament: trill
}
score demo style Ornamented {
  voice lead {
    ornament mordent {
      note C4, 1/4
    }
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_ORNAMENT");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("ornament"));
    }

    #[test]
    fn non_chord_tone_rule_accepts_listed_tone() {
        let ir = compile_source(
            r#"
style Ornamented {
  non_chord_tone: passing_tone neighbor_tone
}
score demo style Ornamented {
  voice lead {
    note C4, 1/8
    non_chord_tone passing_tone {
      note D4, 1/8
    }
    note E4, 1/8
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 3);
    }

    #[test]
    fn non_chord_tone_passing_tone_rejects_leap() {
        let diagnostics = compile_source(
            r#"
style Ornamented {
  non_chord_tone: passing_tone
}
score demo style Ornamented {
  voice lead {
    note C4, 1/8
    non_chord_tone passing_tone {
      note E4, 1/8
    }
    note F4, 1/8
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_NON_CHORD_TONE");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("non_chord_tone"));
    }

    #[test]
    fn non_chord_tone_neighbor_tone_accepts_return() {
        let ir = compile_source(
            r#"
style Ornamented {
  non_chord_tone: neighbor_tone
}
score demo style Ornamented {
  voice lead {
    note C4, 1/8
    non_chord_tone neighbor_tone {
      note D4, 1/8
    }
    note C4, 1/8
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 3);
    }

    #[test]
    fn non_chord_tone_neighbor_tone_rejects_non_return() {
        let diagnostics = compile_source(
            r#"
style Ornamented {
  non_chord_tone: neighbor_tone
}
score demo style Ornamented {
  voice lead {
    note C4, 1/8
    non_chord_tone neighbor_tone {
      note D4, 1/8
    }
    note E4, 1/8
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_NON_CHORD_TONE");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("non_chord_tone"));
    }

    #[test]
    fn non_chord_tone_rule_rejects_unlisted_tone() {
        let diagnostics = compile_source(
            r#"
style Ornamented {
  non_chord_tone: passing_tone
}
score demo style Ornamented {
  voice lead {
    non_chord_tone neighbor_tone {
      note D4, 1/8
    }
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_NON_CHORD_TONE");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("non_chord_tone"));
    }

    #[test]
    fn form_rule_accepts_matching_sections() {
        let ir = compile_source(
            r#"
style BinarySong {
  form: binary
}
score demo style BinarySong {
  voice lead {
    section A {
      note C4, 1/4
    }
    section B {
      note D4, 1/4
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.markers.len(), 2);
        assert_eq!(ir.markers[0].label, "A");
        assert_eq!(ir.markers[1].label, "B");
    }

    #[test]
    fn form_rule_rejects_wrong_section_pattern() {
        let diagnostics = compile_source(
            r#"
style TernarySong {
  form: ternary
}
score demo style TernarySong {
  voice lead {
    section A {
      note C4, 1/4
    }
    section B {
      note D4, 1/4
    }
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_FORM");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("form"));
    }

    #[test]
    fn tempo_range_rule_fails() {
        let diagnostics = compile_source(
            r#"
style Slow {
  tempo_range: 40..80
}
score demo {
  tempo 120
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_TEMPO_RANGE");
    }

    #[test]
    fn instrument_range_rule_fails() {
        let diagnostics = compile_source(
            r#"
style Tiny {
  instrument_range: 40 C4 C5
}
score demo {
  voice lead {
    program 40
    note C6, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_INSTRUMENT_RANGE");
    }

    #[test]
    fn voice_crossing_rule_fails() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice upper {
    note C4, 1/4
  }
  voice lower {
    note E4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_VOICE_CROSSING");
    }

    #[test]
    fn texture_monophony_rule_fails_for_multiple_tracks() {
        let diagnostics = compile_source(
            r#"
style Solo {
  texture: monophony
  severity_voice_crossing: off
}
score demo style Solo {
  voice a { note C4, 1/4 }
  voice b { note E4, 1/4 }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_TEXTURE");
    }

    #[test]
    fn texture_polyphony_rule_accepts_multiple_tracks() {
        let ir = compile_source(
            r#"
style Poly {
  texture: polyphony
  severity_voice_crossing: off
}
score demo style Poly {
  voice a { note C4, 1/4 }
  voice b { note E4, 1/4 }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks.len(), 2);
    }

    #[test]
    fn texture_homophony_rule_fails_for_different_attack_grid() {
        let diagnostics = compile_source(
            r#"
style Hymn {
  texture: homophony
  severity_voice_crossing: off
}
score demo style Hymn {
  voice a {
    note C4, 1/4
    note D4, 1/4
  }
  voice b {
    note E4, 1/2
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_TEXTURE");
    }

    #[test]
    fn texture_heterophony_accepts_shared_melodic_grid() {
        let ir = compile_source(
            r#"
style SharedMelody {
  texture: heterophony
  severity_voice_crossing: off
}
score demo style SharedMelody {
  voice a {
    note C4, 1/4
    note D4, 1/4
  }
  voice b {
    note C4, 1/4
    note E4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks.len(), 2);
    }

    #[test]
    fn texture_heterophony_rejects_single_voice() {
        let diagnostics = compile_source(
            r#"
style SharedMelody {
  texture: heterophony
}
score demo style SharedMelody {
  voice a {
    note C4, 1/4
    note D4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_TEXTURE");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("texture"));
    }

    #[test]
    fn texture_heterophony_rejects_different_event_counts() {
        let diagnostics = compile_source(
            r#"
style SharedMelody {
  texture: heterophony
  severity_voice_crossing: off
}
score demo style SharedMelody {
  voice a {
    note C4, 1/4
    note D4, 1/4
  }
  voice b {
    note C4, 1/2
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_TEXTURE");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("texture"));
    }

    #[test]
    fn override_allows_texture_violation() {
        let ir = compile_source(
            r#"
style Solo {
  texture: monophony
  severity_voice_crossing: off
}
score demo style Solo {
  override texture allow reason "brief divisi" {
    voice a { note C4, 1/4 }
    voice b { note E4, 1/4 }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks.len(), 2);
    }

    #[test]
    fn texture_rule_can_be_disabled() {
        let ir = compile_source(
            r#"
style Solo {
  texture: monophony
  severity_texture: off
  severity_voice_crossing: off
}
score demo style Solo {
  voice a { note C4, 1/4 }
  voice b { note E4, 1/4 }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks.len(), 2);
    }

    #[test]
    fn harmonic_progression_rule_accepts_sequence() {
        let ir = compile_source(
            r#"
style Functional {
  harmonic_progression: tonic predominant dominant tonic
}
score demo style Functional {
  voice chordal {
    chord [C4, E4, G4], 1/4
    chord [F4, A4, C5], 1/4
    chord [G4, B4, D5], 1/4
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 12);
    }

    #[test]
    fn harmonic_progression_rule_accepts_secondary_dominant() {
        let ir = compile_source(
            r#"
style Functional {
  harmonic_progression: tonic secondary_dominant dominant tonic
}
score demo style Functional {
  voice chordal {
    chord [C4, E4, G4], 1/4
    chord [D4, F#4, A4], 1/4
    chord [G4, B4, D5], 1/4
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 12);
    }

    #[test]
    fn harmonic_progression_rule_accepts_submediant() {
        let ir = compile_source(
            r#"
style Functional {
  harmonic_progression: tonic dominant submediant
}
score demo style Functional {
  voice chordal {
    chord [C4, E4, G4], 1/4
    chord [G4, B4, D5], 1/4
    chord [A4, C5, E5], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 9);
    }

    #[test]
    fn harmonic_progression_rule_fails() {
        let diagnostics = compile_source(
            r#"
style Functional {
  harmonic_progression: tonic predominant dominant tonic
}
score demo style Functional {
  voice chordal {
    chord [C4, E4, G4], 1/4
    chord [G4, B4, D5], 1/4
    chord [F4, A4, C5], 1/4
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_HARMONIC_PROGRESSION");
    }

    #[test]
    fn override_allows_harmonic_progression_violation() {
        let ir = compile_source(
            r#"
style Functional {
  harmonic_progression: tonic predominant dominant tonic
}
score demo style Functional {
  override harmonic_progression allow reason "nonfunctional progression" {
    voice chordal {
      chord [C4, E4, G4], 1/4
      chord [G4, B4, D5], 1/4
      chord [F4, A4, C5], 1/4
      chord [C4, E4, G4], 1/4
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 12);
    }

    #[test]
    fn harmonic_progression_rule_can_be_disabled() {
        let ir = compile_source(
            r#"
style Functional {
  harmonic_progression: tonic predominant dominant tonic
  severity_harmonic_progression: off
}
score demo style Functional {
  voice chordal {
    chord [C4, E4, G4], 1/4
    chord [G4, B4, D5], 1/4
    chord [F4, A4, C5], 1/4
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 12);
    }

    #[test]
    fn cadence_rule_accepts_authentic_ending() {
        let ir = compile_source(
            r#"
style ClassicalCadence {
  cadence: authentic
}
score demo style ClassicalCadence {
  voice chordal {
    chord [G4, B4, D5], 1/4
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 6);
    }

    #[test]
    fn cadence_rule_fails() {
        let diagnostics = compile_source(
            r#"
style ClassicalCadence {
  cadence: authentic
}
score demo style ClassicalCadence {
  voice chordal {
    chord [F4, A4, C5], 1/4
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_CADENCE");
    }

    #[test]
    fn cadence_rule_accepts_plagal_cadence() {
        let ir = compile_source(
            r#"
style ClassicalCadence {
  cadence: plagal
}
score demo style ClassicalCadence {
  voice chordal {
    chord [F4, A4, C5], 1/4
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 6);
    }

    #[test]
    fn cadence_rule_rejects_plagal_without_subdominant() {
        let diagnostics = compile_source(
            r#"
style ClassicalCadence {
  cadence: plagal
}
score demo style ClassicalCadence {
  voice chordal {
    chord [G4, B4, D5], 1/4
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_CADENCE");
    }

    #[test]
    fn cadence_rule_accepts_deceptive_cadence() {
        let ir = compile_source(
            r#"
style ClassicalCadence {
  cadence: deceptive
}
score demo style ClassicalCadence {
  voice chordal {
    chord [G4, B4, D5], 1/4
    chord [A4, C5, E5], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 6);
    }

    #[test]
    fn cadence_rule_rejects_deceptive_without_submediant() {
        let diagnostics = compile_source(
            r#"
style ClassicalCadence {
  cadence: deceptive
}
score demo style ClassicalCadence {
  voice chordal {
    chord [G4, B4, D5], 1/4
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_CADENCE");
    }

    #[test]
    fn cadence_rule_accepts_half_cadence() {
        let ir = compile_source(
            r#"
style ClassicalCadence {
  cadence: half
}
score demo style ClassicalCadence {
  voice chordal {
    chord [C4, E4, G4], 1/4
    chord [G4, B4, D5], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 6);
    }

    #[test]
    fn cadence_rule_accepts_any_configured_candidate() {
        let ir = compile_source(
            r#"
style FlexibleCadence {
  cadence: authentic plagal deceptive
}
score demo style FlexibleCadence {
  voice chordal {
    chord [F4, A4, C5], 1/4
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 6);
    }

    #[test]
    fn cadence_rule_rejects_when_no_candidate_matches() {
        let diagnostics = compile_source(
            r#"
style FlexibleCadence {
  cadence: authentic deceptive
}
score demo style FlexibleCadence {
  voice chordal {
    chord [F4, A4, C5], 1/4
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_CADENCE");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("cadence"));
    }

    #[test]
    fn cadence_rule_rejects_half_cadence_without_final_dominant() {
        let diagnostics = compile_source(
            r#"
style ClassicalCadence {
  cadence: half
}
score demo style ClassicalCadence {
  voice chordal {
    chord [G4, B4, D5], 1/4
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_CADENCE");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("cadence"));
    }

    #[test]
    fn override_allows_cadence_violation() {
        let ir = compile_source(
            r#"
style ClassicalCadence {
  cadence: authentic
}
score demo style ClassicalCadence {
  override cadence allow reason "open ending" {
    voice chordal {
      chord [F4, A4, C5], 1/4
      chord [C4, E4, G4], 1/4
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 6);
    }

    #[test]
    fn cadence_rule_can_be_disabled() {
        let ir = compile_source(
            r#"
style ClassicalCadence {
  cadence: authentic
  severity_cadence: off
}
score demo style ClassicalCadence {
  voice chordal {
    chord [F4, A4, C5], 1/4
    chord [C4, E4, G4], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 6);
    }

    #[test]
    fn contrapuntal_motion_rule_fails() {
        let diagnostics = compile_source(
            r#"
style Counterpoint {
  contrapuntal_motion: contrary oblique
}
score demo style Counterpoint {
  voice upper {
    note C5, 1/4
    note D5, 1/4
  }
  voice lower {
    note C4, 1/4
    note D4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_CONTRAPUNTAL_MOTION");
    }

    #[test]
    fn override_allows_contrapuntal_motion_violation() {
        let ir = compile_source(
            r#"
style Counterpoint {
  contrapuntal_motion: contrary oblique
}
score demo style Counterpoint {
  override contrapuntal_motion allow reason "sequence" {
    voice upper {
      note C5, 1/4
      note D5, 1/4
    }
    voice lower {
      note C4, 1/4
      note D4, 1/4
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks.len(), 2);
    }

    #[test]
    fn contrapuntal_motion_rule_can_be_disabled() {
        let ir = compile_source(
            r#"
style Counterpoint {
  contrapuntal_motion: contrary oblique
  severity_contrapuntal_motion: off
}
score demo style Counterpoint {
  voice upper {
    note C5, 1/4
    note D5, 1/4
  }
  voice lower {
    note C4, 1/4
    note D4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks.len(), 2);
    }

    #[test]
    fn parallel_fifths_rule_fails() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice upper {
    note G4, 1/4
    note A4, 1/4
  }
  voice lower {
    note C4, 1/4
    note D4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_PARALLEL_FIFTHS");
    }

    #[test]
    fn score_selects_named_style() {
        let ir = compile_source(
            r#"
style Classical {
  scale: C D E F G A B
}
style Sparse {
  scale: C E G
}
score demo style Sparse {
  voice lead {
    note E4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn score_can_select_builtin_style_without_local_declaration() {
        let ir = compile_source(
            r#"
score demo style Jazz {
  voice lead {
    note F#4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn unknown_score_style_fails() {
        let diagnostics = compile_source(
            r#"
style Classical
score demo style Missing {
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_UNKNOWN_NAME");
    }

    #[test]
    fn diagnose_source_reports_non_blocking_warning() {
        let diagnostics = diagnose_source(
            r#"
style Soft {
  scale: C E G
  severity_scale: warning
}
score demo style Soft {
  voice lead {
    note D4, 1/4
  }
}
"#,
        );

        assert_eq!(diagnostics[0].code, "ML_STYLE_SCALE");
        assert_eq!(diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn style_rule_warning_does_not_block_compile() {
        let compilation = compile_source_with_diagnostics(
            r#"
style Soft {
  scale: C E G
  severity_scale: warning
}
score demo style Soft {
  voice lead {
    note D4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(compilation.ir.tracks[0].events.len(), 1);
        assert_eq!(compilation.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn style_rule_off_suppresses_rule() {
        let ir = compile_source(
            r#"
style Open {
  scale: C E G
  severity_scale: off
}
score demo style Open {
  voice lead {
    note D4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn style_inherits_parent_rules() {
        let diagnostics = compile_source(
            r#"
style Parent {
  scale: C E G
}
style Child extends Parent {
  tempo_range: 60..120
}
score demo style Child {
  tempo 90
  voice lead {
    note D4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_SCALE");
        assert_eq!(diagnostics[0].style.as_deref(), Some("Child"));
    }

    #[test]
    fn style_child_overrides_parent_rule_inputs() {
        let ir = compile_source(
            r#"
style Parent {
  scale: C E G
}
style Child extends Parent {
  scale: C D E F G A B
}
score demo style Child {
  voice lead {
    note D4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn style_inheritance_cycle_fails() {
        let diagnostics = compile_source(
            r#"
style A extends B
style B extends A
score demo style A {
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_INHERITANCE_CYCLE");
    }

    #[test]
    fn local_style_scope_switches_rules() {
        let ir = compile_source(
            r#"
style Classical {
  scale: C D E F G A B
}
style Sparse {
  scale: C E G
}
score demo style Classical {
  voice lead {
    with style Sparse {
      note E4, 1/4
    }
    note D4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
    }

    #[test]
    fn local_style_scope_enforces_selected_style() {
        let diagnostics = compile_source(
            r#"
style Classical {
  scale: C D E F G A B
}
style Sparse {
  scale: C E G
}
score demo style Classical {
  voice lead {
    with style Sparse {
      note D4, 1/4
    }
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_SCALE");
        assert_eq!(diagnostics[0].style.as_deref(), Some("Sparse"));
    }

    #[test]
    fn dynamic_and_velocity_set_following_event_velocity() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    dynamic f
    note C4, 1/4
    velocity 32
    note D4, 1/4
    chord [E4, G4], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events[0].velocity, 96);
        assert_eq!(ir.tracks[0].events[1].velocity, 32);
        assert_eq!(ir.tracks[0].events[2].velocity, 32);
        assert_eq!(ir.tracks[0].events[3].velocity, 32);
    }

    #[test]
    fn articulation_sets_following_event_metadata() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    articulation staccato
    note C4, 1/4
    chord [E4, G4], 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(
            ir.tracks[0].events[0].articulation.as_deref(),
            Some("staccato")
        );
        assert_eq!(
            ir.tracks[0].events[1].articulation.as_deref(),
            Some("staccato")
        );
        assert_eq!(
            ir.tracks[0].events[2].articulation.as_deref(),
            Some("staccato")
        );
    }
}
