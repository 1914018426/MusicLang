use std::collections::{BTreeSet, HashMap, HashSet};

use musiclang_core::{
    Chord, CustomStyleRule, CustomTheoryDomain, Diagnostic, Duration, FormEventIr, HarmonicEventIr,
    InstrumentRange, Interval, KeyChangeIr, KeySignature, MarkerIr, MelodicEventIr, Meter,
    MeterChangeIr, MotifEventIr, Note, NoteEventIr, OverrideTrace, PhraseEventIr, Pitch,
    PitchClass, RuleSeverity, ScoreIr, Severity, SourceFile, SourceId, Span, StyleContext,
    TempoChangeIr, TheoryDomain, TheoryReference, TrackIr, DEFAULT_TICKS_PER_QUARTER,
};
use musiclang_parser::{
    parse_source_file, parse_source_with_source_id, ArpeggioStmt, ArticulationStmt, BinaryOp,
    CadenceStmt, ChordStmt, DegreeStmt, DrumStmt, DynamicStmt, Expr, ExprKind, FunctionDecl,
    GlissandoStmt, ModulateStmt, NoteStmt, OstinatoStmt, OverrideStmt, PedalStmt, PlayStmt,
    Program, ProgressionStmt, RestStmt, RomanStmt, ScaleStmt, ScoreMeta, SequenceStmt, Stmt,
    StrumStmt, StyleDecl, TransposeStmt, TremoloStmt, TupletStmt, UnaryOp, VoiceDecl,
    WithStyleStmt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compilation {
    pub ir: ScoreIr,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn compile_source(source: &str) -> Result<ScoreIr, Vec<Diagnostic>> {
    compile_source_with_diagnostics(source).map(|compilation| compilation.ir)
}

pub fn compile_source_with_source_id(
    source_id: SourceId,
    source: &str,
) -> Result<ScoreIr, Vec<Diagnostic>> {
    compile_source_with_diagnostics_and_source_id(source_id, source)
        .map(|compilation| compilation.ir)
}

pub fn compile_source_file(source_file: &SourceFile) -> Result<ScoreIr, Vec<Diagnostic>> {
    compile_source_file_with_diagnostics(source_file).map(|compilation| compilation.ir)
}

pub fn compile_source_with_diagnostics(source: &str) -> Result<Compilation, Vec<Diagnostic>> {
    compile_source_with_diagnostics_and_source_id(SourceId(0), source)
}

pub fn compile_source_with_diagnostics_and_source_id(
    source_id: SourceId,
    source: &str,
) -> Result<Compilation, Vec<Diagnostic>> {
    let program = parse_source_with_source_id(source_id, source)?;
    Compiler::new(program).compile()
}

pub fn compile_source_file_with_diagnostics(
    source_file: &SourceFile,
) -> Result<Compilation, Vec<Diagnostic>> {
    let program = parse_source_file(source_file)?;
    Compiler::new(program).compile()
}

pub fn diagnose_source(source: &str) -> Vec<Diagnostic> {
    diagnose_source_with_source_id(SourceId(0), source)
}

pub fn diagnose_source_with_source_id(source_id: SourceId, source: &str) -> Vec<Diagnostic> {
    match compile_source_with_diagnostics_and_source_id(source_id, source) {
        Ok(compilation) => compilation.diagnostics,
        Err(diagnostics) => diagnostics,
    }
}

pub fn diagnose_source_file(source_file: &SourceFile) -> Vec<Diagnostic> {
    match compile_source_file_with_diagnostics(source_file) {
        Ok(compilation) => compilation.diagnostics,
        Err(diagnostics) => diagnostics,
    }
}

mod context;
mod eval;
mod lower;
mod stylecheck;
mod track;

use eval::Value;
use track::TrackBuilder;

fn is_phrase_function_transform(callee: &str) -> bool {
    matches!(callee, "map" | "filter" | "mapi")
}

fn motif_transform(args: &[Value]) -> Option<String> {
    match args {
        [Value::Pitch(_)] => Some("transposition".to_string()),
        [Value::Duration(_)] => Some("rhythmic_variation".to_string()),
        [Value::Pitch(_), Value::Duration(_)] | [Value::Duration(_), Value::Pitch(_)] => {
            Some("transposition+rhythmic_variation".to_string())
        }
        _ => None,
    }
}

struct BuiltinSignature {
    arg_count: usize,
    type_message: &'static str,
}

fn is_note_tuple(values: &[Value]) -> bool {
    matches!(values, [Value::Pitch(_), Value::Duration(_)])
}

fn is_transform_collection(value: &Value) -> bool {
    match value {
        Value::List(_) => true,
        Value::Tuple(values) => !is_note_tuple(values),
        _ => false,
    }
}

fn builtin_signature(name: &str) -> Option<BuiltinSignature> {
    match name {
        "map" => Some(BuiltinSignature {
            arg_count: 2,
            type_message: "expects collection and function name",
        }),
        "filter" => Some(BuiltinSignature {
            arg_count: 2,
            type_message: "expects collection and function name",
        }),
        "mapi" => Some(BuiltinSignature {
            arg_count: 2,
            type_message: "expects collection and function name",
        }),
        "transpose" => Some(BuiltinSignature {
            arg_count: 2,
            type_message: "expects value and interval",
        }),
        "repeat" => Some(BuiltinSignature {
            arg_count: 2,
            type_message: "expects value and integer count",
        }),
        "stretch" => Some(BuiltinSignature {
            arg_count: 2,
            type_message: "expects value and integer factor",
        }),
        "duration" => Some(BuiltinSignature {
            arg_count: 1,
            type_message: "expects duration string",
        }),
        "pitch" => Some(BuiltinSignature {
            arg_count: 1,
            type_message: "expects pitch string",
        }),
        "first" => Some(BuiltinSignature {
            arg_count: 1,
            type_message: "expects non-empty collection",
        }),
        "len" => Some(BuiltinSignature {
            arg_count: 1,
            type_message: "expects list or tuple",
        }),
        "at" => Some(BuiltinSignature {
            arg_count: 2,
            type_message: "expects collection and integer index",
        }),
        "with" | "merge" => Some(BuiltinSignature {
            arg_count: 2,
            type_message: "expects dict arguments",
        }),
        _ => None,
    }
}

struct Compiler {
    program: Program,
    style: StyleContext,
    functions: HashMap<String, FunctionDecl>,
    function_call_stack: Vec<String>,
    variables: Vec<HashMap<String, Value>>,
    diagnostics: Vec<Diagnostic>,
    override_rules: Vec<String>,
    score_override_rules: HashSet<String>,
    phrase_concept_override_phrases: HashSet<usize>,
    phrase_concept_override_motifs: HashSet<usize>,
    override_traces: Vec<OverrideTrace>,
    section_labels: Vec<String>,
    markers: Vec<MarkerIr>,
    tempo_changes: Vec<TempoChangeIr>,
    meter_changes: Vec<MeterChangeIr>,
    key_changes: Vec<KeyChangeIr>,
    harmonic_events: Vec<HarmonicEventIr>,
    melodic_events: Vec<MelodicEventIr>,
    form_events: Vec<FormEventIr>,
    motif_events: Vec<MotifEventIr>,
    phrase_events: Vec<PhraseEventIr>,
    pending_non_chord_tones: Vec<PendingNonChordTone>,
    score_key: Option<KeySignature>,
    pitch_transpose_semitones: i16,
}

struct PendingNonChordTone {
    kind: String,
    previous_event_index: Option<usize>,
    event_start: usize,
    event_end: usize,
    line: usize,
    column: usize,
}

#[derive(Clone, Copy)]
struct ChordPitchContext {
    line: usize,
    column: usize,
    span: Span,
    program: Option<u8>,
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
            phrase_concept_override_phrases: HashSet::new(),
            phrase_concept_override_motifs: HashSet::new(),
            override_traces: Vec::new(),
            section_labels: Vec::new(),
            markers: Vec::new(),
            tempo_changes: Vec::new(),
            meter_changes: Vec::new(),
            key_changes: Vec::new(),
            harmonic_events: Vec::new(),
            melodic_events: Vec::new(),
            form_events: Vec::new(),
            motif_events: Vec::new(),
            phrase_events: Vec::new(),
            pending_non_chord_tones: Vec::new(),
            score_key: None,
            pitch_transpose_semitones: 0,
        }
    }

    fn compile(mut self) -> Result<Compilation, Vec<Diagnostic>> {
        let mut tracks = Vec::new();
        let (title, composer, tempo_bpm, meter, key) = lower::score_metadata(&self.program);
        self.check_score_key_metadata();
        self.check_function_calls();
        self.check_function_arity();
        self.check_style_references();
        self.check_override_rules();
        self.check_expression_names();
        self.score_key = key;
        self.check_score_style(tempo_bpm, meter);
        let statements = self.program.score.statements.clone();

        for statement in statements {
            match statement {
                Stmt::Voice(voice) => {
                    let mut track = TrackBuilder::new(
                        &voice.name,
                        voice.program,
                        voice.channel,
                        voice.volume,
                        voice.pan,
                    );
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
                    let mut track = TrackBuilder::new("main", None, None, None, None);
                    self.compile_statement(&other, &mut track);
                    if !track.is_empty() {
                        tracks.push(track.finish());
                    }
                }
            }
        }

        self.check_counterpoint_rules(&tracks);
        self.check_texture(&tracks);
        self.check_rhythm_concepts(&tracks);
        let melodic_events = self.melodic_events.clone();
        self.check_melodic_concepts(&tracks, &melodic_events);
        let phrase_events = self.phrase_events.clone();
        let motif_events = self.motif_events.clone();
        self.check_phrase_concepts(&phrase_events, &motif_events);
        self.check_ensemble_concepts(&tracks);
        self.check_bass_concepts(&tracks);
        let form_events = self.form_events.clone();
        self.check_form(&form_events);
        let harmonic_events = self.harmonic_events.clone();
        self.check_harmonic_progression(&tracks, &harmonic_events);
        self.check_cadence(&tracks, &harmonic_events);

        if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
        {
            return Err(self.diagnostics);
        }

        Ok(Compilation {
            ir: lower::score_ir(
                lower::ScoreMetadata {
                    title,
                    composer,
                    tempo_bpm,
                    meter,
                    key,
                },
                lower::ScoreLoweringParts {
                    tracks,
                    markers: self.markers,
                    tempo_changes: self.tempo_changes,
                    meter_changes: self.meter_changes,
                    key_changes: self.key_changes,
                    harmonic_events: self.harmonic_events,
                    melodic_events: self.melodic_events,
                    form_events: self.form_events,
                    motif_events: self.motif_events,
                    phrase_events: self.phrase_events,
                    overrides: self.override_traces,
                },
            ),
            diagnostics: self.diagnostics,
        })
    }

    fn check_score_key_metadata(&mut self) {
        let metadata = self.program.score.metadata.clone();
        for meta in metadata {
            if let ScoreMeta::Key(key) = meta {
                self.key_signature_or_diagnostic(
                    &key.tonic, &key.mode, key.line, key.column, key.span,
                );
            }
        }
    }

    fn check_function_calls(&mut self) {
        let mut score_calls = Vec::new();
        self.collect_function_calls(&self.program.score.statements, &mut score_calls);
        self.check_unknown_function_calls(&score_calls);

        let mut graph = HashMap::new();
        let functions = self.program.functions.clone();
        for function in functions {
            let mut calls = Vec::new();
            self.collect_function_calls(function.statements(), &mut calls);
            if let Some(expr) = function.body_expr() {
                self.collect_expression_calls(expr, function.line, function.column, &mut calls);
            }
            self.check_unknown_function_calls(&calls);
            graph.insert(function.name.clone(), calls);
        }

        let mut reported = HashSet::new();
        for function in self.functions.keys().cloned().collect::<Vec<_>>() {
            let mut visiting = Vec::new();
            self.check_recursive_function_calls(&function, &graph, &mut visiting, &mut reported);
        }
    }

    fn check_unknown_function_calls(&mut self, calls: &[(String, usize, usize, Span)]) {
        for (name, line, column, span) in calls {
            if !self.functions.contains_key(name) {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_RESOLVE_UNKNOWN_NAME",
                        format!("unknown function `{name}`"),
                        *line,
                        *column,
                    )
                    .with_span(*span)
                    .with_help(
                        "define the function before calling it or correct the function name",
                    ),
                );
            }
        }
    }

    fn check_function_arity(&mut self) {
        let statements = self.program.score.statements.clone();
        self.check_function_arity_in_statements(&statements);
        let functions = self.program.functions.clone();
        for function in functions {
            self.check_function_arity_in_statements(function.statements());
            if let Some(expr) = function.body_expr() {
                self.check_function_arity_in_expr(expr, function.line, function.column);
            }
        }
    }

    fn check_function_arity_in_expr(&mut self, expr: &Expr, line: usize, column: usize) {
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                if let Some(function) = self.functions.get(callee) {
                    if function.body_expr().is_none() {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "ML_TYPE_MISMATCH",
                                format!("function `{callee}` is not expression-bodied"),
                                line,
                                column,
                            )
                            .with_span(expr.span),
                        );
                    } else if function.params.len() != args.len() {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "ML_TYPE_MISMATCH",
                                format!(
                                    "function `{}` expects {} arguments, got {}",
                                    callee,
                                    function.params.len(),
                                    args.len()
                                ),
                                line,
                                column,
                            )
                            .with_span(expr.span),
                        );
                    }
                }
                for arg in args {
                    self.check_function_arity_in_expr(arg, line, column);
                }
            }
            ExprKind::MethodCall {
                target,
                method,
                args,
            } => {
                self.check_function_arity_in_expr(target, line, column);
                if is_phrase_function_transform(method) && args.len() == 1 {
                    return;
                }
                for arg in args {
                    self.check_function_arity_in_expr(arg, line, column);
                }
            }
            ExprKind::Pipe { value, call } => {
                self.check_function_arity_in_expr(value, line, column);
                self.check_function_arity_in_expr(call, line, column);
            }
            ExprKind::List(values) | ExprKind::Tuple(values) => {
                for value in values {
                    self.check_function_arity_in_expr(value, line, column);
                }
            }
            ExprKind::ListComprehension {
                item,
                source,
                condition,
                ..
            } => {
                self.check_function_arity_in_expr(item, line, column);
                self.check_function_arity_in_expr(source, line, column);
                if let Some(condition) = condition {
                    self.check_function_arity_in_expr(condition, line, column);
                }
            }
            ExprKind::Dict(entries) => {
                for (_, value) in entries {
                    self.check_function_arity_in_expr(value, line, column);
                }
            }
            ExprKind::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                self.check_function_arity_in_expr(condition, line, column);
                self.check_function_arity_in_expr(then_branch, line, column);
                self.check_function_arity_in_expr(else_branch, line, column);
            }
            ExprKind::Access { target, .. } => {
                self.check_function_arity_in_expr(target, line, column)
            }
            ExprKind::Unary { expr, .. } => self.check_function_arity_in_expr(expr, line, column),
            ExprKind::Range { start, end } => {
                self.check_function_arity_in_expr(start, line, column);
                self.check_function_arity_in_expr(end, line, column);
            }
            ExprKind::Binary { left, right, .. } => {
                self.check_function_arity_in_expr(left, line, column);
                self.check_function_arity_in_expr(right, line, column);
            }
            ExprKind::Ident(_)
            | ExprKind::Int(_)
            | ExprKind::Bool(_)
            | ExprKind::PitchLiteral(_)
            | ExprKind::IntervalLiteral(_)
            | ExprKind::DurationLiteral(_)
            | ExprKind::StringLiteral(_) => {}
        }
    }

    fn check_function_arity_in_statements(&mut self, statements: &[Stmt]) {
        for statement in statements {
            match statement {
                Stmt::Call(call) => {
                    if let Some(function) = self.functions.get(&call.name) {
                        if function.params.len() != call.args.len() {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "ML_TYPE_MISMATCH",
                                    format!(
                                        "function `{}` expects {} arguments, got {}",
                                        call.name,
                                        function.params.len(),
                                        call.args.len()
                                    ),
                                    call.line,
                                    call.column,
                                )
                                .with_span(call.span),
                            );
                        }
                    }
                }
                Stmt::Play(play) => {
                    self.check_function_arity_in_expr(&play.expr, play.line, play.column)
                }
                Stmt::Voice(voice) => self.check_function_arity_in_statements(&voice.statements),
                Stmt::Ostinato(ostinato) => {
                    self.check_function_arity_in_statements(&ostinato.statements)
                }
                Stmt::Sequence(sequence) => {
                    self.check_function_arity_in_statements(&sequence.statements)
                }
                Stmt::Tuplet(tuplet) => self.check_function_arity_in_statements(&tuplet.statements),
                Stmt::Transpose(transpose) => {
                    self.check_function_arity_in_statements(&transpose.statements)
                }
                Stmt::Section(section) => {
                    self.check_function_arity_in_statements(&section.statements)
                }
                Stmt::Ornament(ornament) => {
                    self.check_function_arity_in_statements(&ornament.statements)
                }
                Stmt::NonChordTone(non_chord_tone) => {
                    self.check_function_arity_in_statements(&non_chord_tone.statements)
                }
                Stmt::TuningSystem(tuning_system) => {
                    self.check_function_arity_in_statements(&tuning_system.statements)
                }
                Stmt::WorldTradition(world_tradition) => {
                    self.check_function_arity_in_statements(&world_tradition.statements)
                }
                Stmt::HistoricalEra(historical_era) => {
                    self.check_function_arity_in_statements(&historical_era.statements)
                }
                Stmt::HarmonicFunction(harmonic_function) => {
                    self.check_function_arity_in_statements(&harmonic_function.statements)
                }
                Stmt::For(for_stmt) => {
                    self.check_function_arity_in_statements(&for_stmt.statements)
                }
                Stmt::If(if_stmt) => self.check_function_arity_in_statements(&if_stmt.statements),
                Stmt::Override(override_stmt) => {
                    self.check_function_arity_in_statements(&override_stmt.statements)
                }
                Stmt::WithStyle(with_style) => {
                    self.check_function_arity_in_statements(&with_style.statements)
                }
                _ => {}
            }
        }
    }

    fn check_recursive_function_calls(
        &mut self,
        function: &str,
        graph: &HashMap<String, Vec<(String, usize, usize, Span)>>,
        visiting: &mut Vec<String>,
        reported: &mut HashSet<String>,
    ) {
        if let Some(index) = visiting.iter().position(|name| name == function) {
            let cycle = visiting[index..].join(" -> ");
            if reported.insert(cycle.clone()) {
                let line = self
                    .functions
                    .get(function)
                    .map(|function| function.line)
                    .unwrap_or(self.program.score.line);
                let column = self
                    .functions
                    .get(function)
                    .map(|function| function.column)
                    .unwrap_or(self.program.score.column);
                let message = if cycle == function {
                    format!("recursive function call `{function}`")
                } else {
                    format!("recursive function call `{cycle} -> {function}`")
                };
                self.diagnostics.push(Diagnostic::error(
                    "ML_RESOLVE_RECURSIVE_CALL",
                    message,
                    line,
                    column,
                ));
            }
            return;
        }
        visiting.push(function.to_string());
        if let Some(calls) = graph.get(function) {
            for (callee, _, _, _) in calls {
                if self.functions.contains_key(callee) {
                    self.check_recursive_function_calls(callee, graph, visiting, reported);
                }
            }
        }
        visiting.pop();
    }

    fn collect_expression_calls(
        &self,
        expr: &Expr,
        line: usize,
        column: usize,
        calls: &mut Vec<(String, usize, usize, Span)>,
    ) {
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                if self.functions.contains_key(callee) {
                    calls.push((callee.clone(), line, column, expr.span));
                }
                for arg in args {
                    self.collect_expression_calls(arg, line, column, calls);
                }
            }
            ExprKind::MethodCall {
                target,
                method,
                args,
            } => {
                self.collect_expression_calls(target, line, column, calls);
                if is_phrase_function_transform(method) && args.len() == 1 {
                    return;
                }
                for arg in args {
                    self.collect_expression_calls(arg, line, column, calls);
                }
            }
            ExprKind::Pipe { value, call } => {
                self.collect_expression_calls(value, line, column, calls);
                self.collect_expression_calls(call, line, column, calls);
            }
            ExprKind::List(values) | ExprKind::Tuple(values) => {
                for value in values {
                    self.collect_expression_calls(value, line, column, calls);
                }
            }
            ExprKind::ListComprehension {
                item,
                source,
                condition,
                ..
            } => {
                self.collect_expression_calls(item, line, column, calls);
                self.collect_expression_calls(source, line, column, calls);
                if let Some(condition) = condition {
                    self.collect_expression_calls(condition, line, column, calls);
                }
            }
            ExprKind::Dict(entries) => {
                for (_, value) in entries {
                    self.collect_expression_calls(value, line, column, calls);
                }
            }
            ExprKind::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_expression_calls(condition, line, column, calls);
                self.collect_expression_calls(then_branch, line, column, calls);
                self.collect_expression_calls(else_branch, line, column, calls);
            }
            ExprKind::Access { target, .. } => {
                self.collect_expression_calls(target, line, column, calls)
            }
            ExprKind::Unary { expr, .. } => {
                self.collect_expression_calls(expr, line, column, calls)
            }
            ExprKind::Range { start, end } => {
                self.collect_expression_calls(start, line, column, calls);
                self.collect_expression_calls(end, line, column, calls);
            }
            ExprKind::Binary { left, right, .. } => {
                self.collect_expression_calls(left, line, column, calls);
                self.collect_expression_calls(right, line, column, calls);
            }
            ExprKind::Ident(_)
            | ExprKind::Int(_)
            | ExprKind::Bool(_)
            | ExprKind::PitchLiteral(_)
            | ExprKind::IntervalLiteral(_)
            | ExprKind::DurationLiteral(_)
            | ExprKind::StringLiteral(_) => {}
        }
    }

    fn collect_function_calls(
        &self,
        statements: &[Stmt],
        calls: &mut Vec<(String, usize, usize, Span)>,
    ) {
        for statement in statements {
            match statement {
                Stmt::Call(call) => {
                    calls.push((call.name.clone(), call.line, call.column, call.span))
                }
                Stmt::Voice(voice) => self.collect_function_calls(&voice.statements, calls),
                Stmt::Ostinato(ostinato) => {
                    self.collect_function_calls(&ostinato.statements, calls)
                }
                Stmt::Sequence(sequence) => {
                    self.collect_function_calls(&sequence.statements, calls)
                }
                Stmt::Tuplet(tuplet) => self.collect_function_calls(&tuplet.statements, calls),
                Stmt::Transpose(transpose) => {
                    self.collect_function_calls(&transpose.statements, calls)
                }
                Stmt::Section(section) => self.collect_function_calls(&section.statements, calls),
                Stmt::Ornament(ornament) => {
                    self.collect_function_calls(&ornament.statements, calls)
                }
                Stmt::NonChordTone(non_chord_tone) => {
                    self.collect_function_calls(&non_chord_tone.statements, calls)
                }
                Stmt::TuningSystem(tuning_system) => {
                    self.collect_function_calls(&tuning_system.statements, calls)
                }
                Stmt::WorldTradition(world_tradition) => {
                    self.collect_function_calls(&world_tradition.statements, calls)
                }
                Stmt::HistoricalEra(historical_era) => {
                    self.collect_function_calls(&historical_era.statements, calls)
                }
                Stmt::HarmonicFunction(harmonic_function) => {
                    self.collect_function_calls(&harmonic_function.statements, calls)
                }
                Stmt::For(for_stmt) => self.collect_function_calls(&for_stmt.statements, calls),
                Stmt::If(if_stmt) => self.collect_function_calls(&if_stmt.statements, calls),
                Stmt::Override(override_stmt) => {
                    self.collect_function_calls(&override_stmt.statements, calls)
                }
                Stmt::WithStyle(with_style) => {
                    self.collect_function_calls(&with_style.statements, calls)
                }
                _ => {}
            }
        }
    }

    fn check_style_references(&mut self) {
        let score_statements = self.program.score.statements.clone();
        self.check_style_references_in_statements(&score_statements);
        let functions = self.program.functions.clone();
        for function in functions {
            self.check_style_references_in_statements(function.statements());
        }
    }

    fn check_style_references_in_statements(&mut self, statements: &[Stmt]) {
        for statement in statements {
            match statement {
                Stmt::WithStyle(with_style) => {
                    self.check_style_reference(
                        &with_style.style,
                        with_style.line,
                        with_style.column,
                        Some(with_style.span),
                    );
                    self.check_style_references_in_statements(&with_style.statements);
                }
                Stmt::Voice(voice) => self.check_style_references_in_statements(&voice.statements),
                Stmt::Ostinato(ostinato) => {
                    self.check_style_references_in_statements(&ostinato.statements)
                }
                Stmt::Sequence(sequence) => {
                    self.check_style_references_in_statements(&sequence.statements)
                }
                Stmt::Tuplet(tuplet) => {
                    self.check_style_references_in_statements(&tuplet.statements)
                }
                Stmt::Transpose(transpose) => {
                    self.check_style_references_in_statements(&transpose.statements)
                }
                Stmt::Section(section) => {
                    self.check_style_references_in_statements(&section.statements)
                }
                Stmt::Ornament(ornament) => {
                    self.check_style_references_in_statements(&ornament.statements)
                }
                Stmt::NonChordTone(non_chord_tone) => {
                    self.check_style_references_in_statements(&non_chord_tone.statements)
                }
                Stmt::TuningSystem(tuning_system) => {
                    self.check_style_references_in_statements(&tuning_system.statements)
                }
                Stmt::WorldTradition(world_tradition) => {
                    self.check_style_references_in_statements(&world_tradition.statements)
                }
                Stmt::HistoricalEra(historical_era) => {
                    self.check_style_references_in_statements(&historical_era.statements)
                }
                Stmt::HarmonicFunction(harmonic_function) => {
                    self.check_style_references_in_statements(&harmonic_function.statements)
                }
                Stmt::For(for_stmt) => {
                    self.check_style_references_in_statements(&for_stmt.statements)
                }
                Stmt::If(if_stmt) => self.check_style_references_in_statements(&if_stmt.statements),
                Stmt::Override(override_stmt) => {
                    self.check_style_references_in_statements(&override_stmt.statements)
                }
                _ => {}
            }
        }
    }

    fn check_style_reference(
        &mut self,
        name: &str,
        line: usize,
        column: usize,
        span: Option<Span>,
    ) {
        if self.program.styles.iter().any(|style| style.name == name) {
            return;
        }
        let mut diagnostic = Diagnostic::error(
            "ML_STYLE_UNKNOWN_NAME",
            format!("unknown style `{name}`"),
            line,
            column,
        )
        .with_style(name)
        .with_help("declare the style before selecting it or choose a built-in style name");
        if let Some(span) = span {
            diagnostic = diagnostic.with_span(span);
        }
        self.diagnostics.push(diagnostic);
    }

    fn check_override_rules(&mut self) {
        let score_statements = self.program.score.statements.clone();
        self.check_override_rules_in_statements(&score_statements);
        let functions = self.program.functions.clone();
        for function in functions {
            self.check_override_rules_in_statements(function.statements());
        }
    }

    fn check_override_rules_in_statements(&mut self, statements: &[Stmt]) {
        for statement in statements {
            match statement {
                Stmt::Override(override_stmt) => {
                    self.check_override_rule(override_stmt);
                    self.check_override_rules_in_statements(&override_stmt.statements);
                }
                Stmt::Voice(voice) => self.check_override_rules_in_statements(&voice.statements),
                Stmt::Ostinato(ostinato) => {
                    self.check_override_rules_in_statements(&ostinato.statements)
                }
                Stmt::Sequence(sequence) => {
                    self.check_override_rules_in_statements(&sequence.statements)
                }
                Stmt::Tuplet(tuplet) => self.check_override_rules_in_statements(&tuplet.statements),
                Stmt::Transpose(transpose) => {
                    self.check_override_rules_in_statements(&transpose.statements)
                }
                Stmt::Section(section) => {
                    self.check_override_rules_in_statements(&section.statements)
                }
                Stmt::Ornament(ornament) => {
                    self.check_override_rules_in_statements(&ornament.statements)
                }
                Stmt::NonChordTone(non_chord_tone) => {
                    self.check_override_rules_in_statements(&non_chord_tone.statements)
                }
                Stmt::TuningSystem(tuning_system) => {
                    self.check_override_rules_in_statements(&tuning_system.statements)
                }
                Stmt::WorldTradition(world_tradition) => {
                    self.check_override_rules_in_statements(&world_tradition.statements)
                }
                Stmt::HistoricalEra(historical_era) => {
                    self.check_override_rules_in_statements(&historical_era.statements)
                }
                Stmt::HarmonicFunction(harmonic_function) => {
                    self.check_override_rules_in_statements(&harmonic_function.statements)
                }
                Stmt::For(for_stmt) => {
                    self.check_override_rules_in_statements(&for_stmt.statements)
                }
                Stmt::If(if_stmt) => self.check_override_rules_in_statements(&if_stmt.statements),
                Stmt::WithStyle(with_style) => {
                    self.check_override_rules_in_statements(&with_style.statements)
                }
                _ => {}
            }
        }
    }

    fn check_override_rule(&mut self, override_stmt: &OverrideStmt) {
        if self.is_known_rule(&override_stmt.rule) {
            return;
        }
        self.diagnostics.push(
            Diagnostic::error(
                "ML_STYLE_UNKNOWN_RULE",
                format!("unknown style rule `{}`", override_stmt.rule),
                override_stmt.line,
                override_stmt.column,
            )
            .with_span(override_stmt.span)
            .with_rule(override_stmt.rule.clone())
            .with_help("use a built-in rule id or declare a custom rule with rule_<id> in the active style")
            .with_style(self.style.name.clone()),
        );
    }

    fn check_expression_names(&mut self) {
        let score_statements = self.program.score.statements.clone();
        self.check_expression_names_in_statements(&score_statements, &mut vec![HashMap::new()]);
        let functions = self.program.functions.clone();
        for function in functions {
            let mut scopes = vec![function
                .params
                .iter()
                .map(|param| (param.clone(), function.span))
                .collect()];
            self.check_expression_names_in_statements(function.statements(), &mut scopes);
            if let Some(expr) = function.body_expr() {
                self.check_expression_name(expr, function.line, function.column, &scopes);
            }
        }
    }

    fn check_expression_names_in_statements(
        &mut self,
        statements: &[Stmt],
        scopes: &mut Vec<HashMap<String, Span>>,
    ) {
        for statement in statements {
            match statement {
                Stmt::Voice(voice) => {
                    self.check_expression_names_in_statements(&voice.statements, scopes)
                }
                Stmt::Note(note) => {
                    self.check_expression_name(&note.pitch_expr, note.line, note.column, scopes);
                    self.check_expression_name(&note.duration_expr, note.line, note.column, scopes);
                }
                Stmt::Play(play) => {
                    self.check_expression_name(&play.expr, play.line, play.column, scopes);
                }
                Stmt::Drum(drum) => {
                    self.check_expression_name(&drum.duration_expr, drum.line, drum.column, scopes);
                }
                Stmt::Rest(rest) => {
                    self.check_expression_name(&rest.duration_expr, rest.line, rest.column, scopes);
                }
                Stmt::Glissando(glissando) => {
                    self.check_expression_name(
                        &glissando.start_expr,
                        glissando.line,
                        glissando.column,
                        scopes,
                    );
                    self.check_expression_name(
                        &glissando.end_expr,
                        glissando.line,
                        glissando.column,
                        scopes,
                    );
                    self.check_expression_name(
                        &glissando.steps_expr,
                        glissando.line,
                        glissando.column,
                        scopes,
                    );
                    self.check_expression_name(
                        &glissando.duration_expr,
                        glissando.line,
                        glissando.column,
                        scopes,
                    );
                }
                Stmt::Tremolo(tremolo) => {
                    self.check_expression_name(
                        &tremolo.first_expr,
                        tremolo.line,
                        tremolo.column,
                        scopes,
                    );
                    self.check_expression_name(
                        &tremolo.second_expr,
                        tremolo.line,
                        tremolo.column,
                        scopes,
                    );
                    self.check_expression_name(
                        &tremolo.repeats_expr,
                        tremolo.line,
                        tremolo.column,
                        scopes,
                    );
                    self.check_expression_name(
                        &tremolo.duration_expr,
                        tremolo.line,
                        tremolo.column,
                        scopes,
                    );
                }
                Stmt::Degree(degree) => {
                    self.check_expression_name(
                        &degree.duration_expr,
                        degree.line,
                        degree.column,
                        scopes,
                    );
                }
                Stmt::Scale(scale) => {
                    self.check_expression_name(
                        &scale.duration_expr,
                        scale.line,
                        scale.column,
                        scopes,
                    );
                }
                Stmt::Pedal(pedal) => {
                    self.check_expression_name(&pedal.pitch_expr, pedal.line, pedal.column, scopes);
                    self.check_expression_name(&pedal.count_expr, pedal.line, pedal.column, scopes);
                    self.check_expression_name(
                        &pedal.duration_expr,
                        pedal.line,
                        pedal.column,
                        scopes,
                    );
                }
                Stmt::Ostinato(ostinato) => {
                    self.check_expression_name(
                        &ostinato.count_expr,
                        ostinato.line,
                        ostinato.column,
                        scopes,
                    );
                    self.check_expression_names_in_statements(&ostinato.statements, scopes);
                }
                Stmt::Sequence(sequence) => {
                    self.check_expression_name(
                        &sequence.count_expr,
                        sequence.line,
                        sequence.column,
                        scopes,
                    );
                    self.check_expression_name(
                        &sequence.interval_expr,
                        sequence.line,
                        sequence.column,
                        scopes,
                    );
                    self.check_expression_names_in_statements(&sequence.statements, scopes);
                }
                Stmt::Tuplet(tuplet) => {
                    self.check_expression_name(
                        &tuplet.count_expr,
                        tuplet.line,
                        tuplet.column,
                        scopes,
                    );
                    self.check_expression_name(
                        &tuplet.space_expr,
                        tuplet.line,
                        tuplet.column,
                        scopes,
                    );
                    self.check_expression_names_in_statements(&tuplet.statements, scopes);
                }
                Stmt::Transpose(transpose) => {
                    self.check_expression_name(
                        &transpose.interval_expr,
                        transpose.line,
                        transpose.column,
                        scopes,
                    );
                    self.check_expression_names_in_statements(&transpose.statements, scopes);
                }
                Stmt::Chord(chord) => {
                    for expr in &chord.pitch_exprs {
                        self.check_expression_name(expr, chord.line, chord.column, scopes);
                    }
                    if let Some(expr) = &chord.root_expr {
                        self.check_expression_name(expr, chord.line, chord.column, scopes);
                    }
                    self.check_expression_name(
                        &chord.duration_expr,
                        chord.line,
                        chord.column,
                        scopes,
                    );
                }
                Stmt::Arpeggio(arpeggio) => {
                    for expr in &arpeggio.pitch_exprs {
                        self.check_expression_name(expr, arpeggio.line, arpeggio.column, scopes);
                    }
                    if let Some(expr) = &arpeggio.root_expr {
                        self.check_expression_name(expr, arpeggio.line, arpeggio.column, scopes);
                    }
                    self.check_expression_name(
                        &arpeggio.duration_expr,
                        arpeggio.line,
                        arpeggio.column,
                        scopes,
                    );
                }
                Stmt::Strum(strum) => {
                    for expr in &strum.pitch_exprs {
                        self.check_expression_name(expr, strum.line, strum.column, scopes);
                    }
                    if let Some(expr) = &strum.root_expr {
                        self.check_expression_name(expr, strum.line, strum.column, scopes);
                    }
                    self.check_expression_name(
                        &strum.duration_expr,
                        strum.line,
                        strum.column,
                        scopes,
                    );
                    self.check_expression_name(
                        &strum.offset_expr,
                        strum.line,
                        strum.column,
                        scopes,
                    );
                }
                Stmt::Roman(roman) => {
                    self.check_expression_name(
                        &roman.duration_expr,
                        roman.line,
                        roman.column,
                        scopes,
                    );
                }
                Stmt::Progression(progression) => {
                    self.check_expression_name(
                        &progression.duration_expr,
                        progression.line,
                        progression.column,
                        scopes,
                    );
                }
                Stmt::Cadence(cadence) => {
                    self.check_expression_name(
                        &cadence.duration_expr,
                        cadence.line,
                        cadence.column,
                        scopes,
                    );
                }
                Stmt::Section(section) => {
                    self.check_expression_names_in_statements(&section.statements, scopes);
                }
                Stmt::Ornament(ornament) => {
                    self.check_expression_names_in_statements(&ornament.statements, scopes);
                }
                Stmt::NonChordTone(non_chord_tone) => {
                    self.check_expression_names_in_statements(&non_chord_tone.statements, scopes);
                }
                Stmt::TuningSystem(tuning_system) => {
                    self.check_expression_names_in_statements(&tuning_system.statements, scopes);
                }
                Stmt::WorldTradition(world_tradition) => {
                    self.check_expression_names_in_statements(&world_tradition.statements, scopes);
                }
                Stmt::HistoricalEra(historical_era) => {
                    self.check_expression_names_in_statements(&historical_era.statements, scopes);
                }
                Stmt::HarmonicFunction(harmonic_function) => {
                    self.check_expression_names_in_statements(
                        &harmonic_function.statements,
                        scopes,
                    );
                }
                Stmt::For(for_stmt) => {
                    scopes.push(HashMap::new());
                    self.check_duplicate_binding(
                        &for_stmt.variable,
                        for_stmt.line,
                        for_stmt.column,
                        for_stmt.span,
                        scopes,
                    );
                    self.check_expression_names_in_statements(&for_stmt.statements, scopes);
                    scopes.pop();
                }
                Stmt::If(if_stmt) => {
                    self.check_expression_name(
                        &if_stmt.condition,
                        if_stmt.line,
                        if_stmt.column,
                        scopes,
                    );
                    self.check_expression_names_in_statements(&if_stmt.statements, scopes);
                }
                Stmt::Let(let_stmt) => {
                    self.check_expression_name(
                        &let_stmt.value_expr,
                        let_stmt.line,
                        let_stmt.column,
                        scopes,
                    );
                    self.check_duplicate_binding(
                        &let_stmt.name,
                        let_stmt.line,
                        let_stmt.column,
                        let_stmt.span,
                        scopes,
                    );
                }
                Stmt::Override(override_stmt) => {
                    self.check_expression_names_in_statements(&override_stmt.statements, scopes);
                }
                Stmt::WithStyle(with_style) => {
                    self.check_expression_names_in_statements(&with_style.statements, scopes);
                }
                Stmt::Call(call) => {
                    for arg in &call.args {
                        self.check_expression_name(arg, call.line, call.column, scopes);
                    }
                }
                Stmt::Tempo(_)
                | Stmt::Meter(_)
                | Stmt::Key(_)
                | Stmt::Modulate(_)
                | Stmt::Dynamic(_)
                | Stmt::Velocity(_)
                | Stmt::Articulation(_) => {}
            }
        }
    }

    fn check_duplicate_binding(
        &mut self,
        name: &str,
        line: usize,
        column: usize,
        span: Span,
        scopes: &mut [HashMap<String, Span>],
    ) {
        let Some(scope) = scopes.last_mut() else {
            return;
        };
        if let Some(original_span) = scope.insert(name.to_string(), span) {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_RESOLVE_DUPLICATE_NAME",
                    format!("duplicate binding `{name}`"),
                    line,
                    column,
                )
                .with_span(span)
                .with_related(original_span, "first binding")
                .with_help("rename one binding or move it into a nested scope"),
            );
        }
    }

    fn check_expression_name(
        &mut self,
        expr: &Expr,
        line: usize,
        column: usize,
        scopes: &[HashMap<String, Span>],
    ) {
        match &expr.kind {
            ExprKind::Ident(name) => {
                if !scopes.iter().rev().any(|scope| scope.contains_key(name)) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "ML_RESOLVE_UNKNOWN_NAME",
                            format!("unknown name `{name}`"),
                            line,
                            column,
                        )
                        .with_span(expr.span)
                        .with_help(
                            "define the referenced name before using it or correct the identifier",
                        ),
                    );
                }
            }
            ExprKind::List(values) | ExprKind::Tuple(values) => {
                for value in values {
                    self.check_expression_name(value, line, column, scopes);
                }
            }
            ExprKind::ListComprehension {
                item,
                binding,
                source,
                condition,
            } => {
                self.check_expression_name(source, line, column, scopes);
                let mut scopes = scopes.to_vec();
                scopes.push(HashMap::from([(binding.clone(), expr.span)]));
                self.check_expression_name(item, line, column, &scopes);
                if let Some(condition) = condition {
                    self.check_expression_name(condition, line, column, &scopes);
                }
            }
            ExprKind::Dict(entries) => {
                for (_, value) in entries {
                    self.check_expression_name(value, line, column, scopes);
                }
            }
            ExprKind::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                self.check_expression_name(condition, line, column, scopes);
                self.check_expression_name(then_branch, line, column, scopes);
                self.check_expression_name(else_branch, line, column, scopes);
            }
            ExprKind::Access { target, .. } => {
                self.check_expression_name(target, line, column, scopes);
            }
            ExprKind::MethodCall {
                target,
                method,
                args,
            } if is_phrase_function_transform(method) && args.len() == 1 => {
                self.check_expression_name(target, line, column, scopes);
                self.check_function_reference_name(&args[0], line, column);
            }
            ExprKind::MethodCall { target, args, .. } => {
                self.check_expression_name(target, line, column, scopes);
                for arg in args {
                    self.check_expression_name(arg, line, column, scopes);
                }
            }
            ExprKind::Pipe { value, call } => {
                self.check_expression_name(value, line, column, scopes);
                match &call.kind {
                    ExprKind::Call { callee, args }
                        if is_phrase_function_transform(callee) && args.len() == 1 =>
                    {
                        self.check_function_reference_name(&args[0], line, column);
                    }
                    _ => self.check_expression_name(call, line, column, scopes),
                }
            }
            ExprKind::Call { callee, args }
                if is_phrase_function_transform(callee) && args.len() == 2 =>
            {
                self.check_expression_name(&args[0], line, column, scopes);
                self.check_function_reference_name(&args[1], line, column);
            }
            ExprKind::Call { args, .. } => {
                for arg in args {
                    self.check_expression_name(arg, line, column, scopes);
                }
            }
            ExprKind::Unary { expr, .. } => self.check_expression_name(expr, line, column, scopes),
            ExprKind::Range { start, end } => {
                self.check_expression_name(start, line, column, scopes);
                self.check_expression_name(end, line, column, scopes);
            }
            ExprKind::Binary { left, right, .. } => {
                self.check_expression_name(left, line, column, scopes);
                self.check_expression_name(right, line, column, scopes);
            }
            ExprKind::Int(_)
            | ExprKind::Bool(_)
            | ExprKind::PitchLiteral(_)
            | ExprKind::IntervalLiteral(_)
            | ExprKind::DurationLiteral(_)
            | ExprKind::StringLiteral(_) => {}
        }
    }

    fn check_function_reference_name(&mut self, expr: &Expr, line: usize, column: usize) {
        match &expr.kind {
            ExprKind::Ident(name)
                if self
                    .functions
                    .get(name)
                    .and_then(|function| function.body_expr())
                    .is_some() => {}
            ExprKind::StringLiteral(name)
                if self
                    .functions
                    .get(name)
                    .and_then(|function| function.body_expr())
                    .is_some() => {}
            ExprKind::Ident(name) | ExprKind::StringLiteral(name) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_RESOLVE_UNKNOWN_NAME",
                        format!("unknown function `{name}`"),
                        line,
                        column,
                    )
                    .with_span(expr.span)
                    .with_help("define an expression-bodied function or correct the function name"),
                );
            }
            _ => self.diagnostics.push(
                Diagnostic::error("ML_TYPE_MISMATCH", "expected function name", line, column)
                    .with_span(expr.span),
            ),
        }
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
            Stmt::Tempo(tempo) => {
                self.check_tempo_style(tempo.bpm, tempo.line, tempo.column, Some(tempo.span));
                self.tempo_changes.push(TempoChangeIr {
                    bpm: tempo.bpm,
                    tick: track.cursor_tick(),
                });
            }
            Stmt::Meter(meter) => {
                let compiled_meter = Meter {
                    numerator: meter.numerator,
                    denominator: meter.denominator,
                };
                self.check_meter_style(compiled_meter, meter.line, meter.column, Some(meter.span));
                self.meter_changes.push(MeterChangeIr {
                    meter: compiled_meter,
                    tick: track.cursor_tick(),
                });
            }
            Stmt::Key(key) => {
                if let Some(signature) = self.key_signature_or_diagnostic(
                    &key.tonic, &key.mode, key.line, key.column, key.span,
                ) {
                    self.score_key = Some(signature);
                    self.key_changes.push(KeyChangeIr {
                        key: signature,
                        tick: track.cursor_tick(),
                    });
                }
            }
            Stmt::Note(note) => self.compile_note(note, track),
            Stmt::Play(play) => self.compile_play(play, track),
            Stmt::Drum(drum) => self.compile_drum(drum, track),
            Stmt::Rest(rest) => self.compile_rest(rest, track),
            Stmt::Glissando(glissando) => self.compile_glissando(glissando, track),
            Stmt::Tremolo(tremolo) => self.compile_tremolo(tremolo, track),
            Stmt::Degree(degree) => self.compile_degree(degree, track),
            Stmt::Scale(scale) => self.compile_scale(scale, track),
            Stmt::Pedal(pedal) => self.compile_pedal(pedal, track),
            Stmt::Ostinato(ostinato) => self.compile_ostinato(ostinato, track),
            Stmt::Sequence(sequence) => self.compile_sequence(sequence, track),
            Stmt::Tuplet(tuplet) => self.compile_tuplet(tuplet, track),
            Stmt::Transpose(transpose) => self.compile_transpose(transpose, track),
            Stmt::Chord(chord) => self.compile_chord(chord, track),
            Stmt::Arpeggio(arpeggio) => self.compile_arpeggio(arpeggio, track),
            Stmt::Strum(strum) => self.compile_strum(strum, track),
            Stmt::Roman(roman) => self.compile_roman(roman, track),
            Stmt::Progression(progression) => self.compile_progression(progression, track),
            Stmt::Cadence(cadence) => self.compile_cadence(cadence, track),
            Stmt::Modulate(modulate) => self.compile_modulate(modulate),
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
                let start_tick = track.cursor_tick();
                self.markers.push(MarkerIr {
                    label: section.label.clone(),
                    tick: start_tick,
                });
                self.compile_statements(&section.statements, track);
                let duration_ticks = track.cursor_tick() - start_tick;
                self.form_events.push(FormEventIr {
                    label: section.label.clone(),
                    kind: "section".to_string(),
                    start_tick,
                    duration_ticks,
                    source_span: Some(section.span),
                });
                self.phrase_events.push(PhraseEventIr {
                    label: Some(section.label.clone()),
                    kind: "section".to_string(),
                    start_tick,
                    duration_ticks,
                    source_span: Some(section.span),
                });
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
                    return;
                }

                if let Some(function) = self.functions.get(&call.name).cloned() {
                    if function.params.len() != call.args.len() {
                        return;
                    }
                    let Some(args) = call
                        .args
                        .iter()
                        .map(|arg| self.eval_expr(arg, call.line, call.column))
                        .collect::<Option<Vec<_>>>()
                    else {
                        return;
                    };
                    let transform = motif_transform(&args);
                    let start_tick = track.cursor_tick();
                    self.function_call_stack.push(call.name.clone());
                    self.push_scope();
                    for (param, value) in function.params.iter().zip(args) {
                        self.set_var(param, value);
                    }
                    self.compile_statements(function.statements(), track);
                    self.pop_scope();
                    self.function_call_stack.pop();
                    let duration_ticks = track.cursor_tick() - start_tick;
                    self.motif_events.push(MotifEventIr {
                        name: call.name.clone(),
                        transform: transform.clone(),
                        start_tick,
                        duration_ticks,
                        source_span: Some(call.span),
                    });
                    self.phrase_events.push(PhraseEventIr {
                        label: Some(call.name.clone()),
                        kind: "motif_call".to_string(),
                        start_tick,
                        duration_ticks,
                        source_span: Some(call.span),
                    });
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
            track.program(),
            pitch,
            note.line,
            note.column,
            Some(note.span),
        );
        track.push_note(Note::new(pitch, duration), Some(note.span));
    }

    fn compile_play(&mut self, play: &PlayStmt, track: &mut TrackBuilder) {
        if let Some(value) = self.eval_expr(&play.expr, play.line, play.column) {
            self.compile_play_value(value, play.line, play.column, play.span, track);
        }
    }

    fn compile_play_value(
        &mut self,
        value: Value,
        line: usize,
        column: usize,
        span: Span,
        track: &mut TrackBuilder,
    ) {
        match value {
            Value::List(values) => {
                for value in values {
                    self.compile_play_value(value, line, column, span, track);
                }
            }
            Value::Tuple(values) => {
                if is_note_tuple(&values) {
                    let [Value::Pitch(pitch), Value::Duration(duration)] = values.as_slice() else {
                        unreachable!();
                    };
                    self.push_play_note(*pitch, *duration, line, column, span, track);
                } else {
                    for value in values {
                        self.compile_play_value(value, line, column, span, track);
                    }
                }
            }
            Value::Dict(values) => {
                let pitch = values.get("p").or_else(|| values.get("pitch"));
                let duration = values.get("d").or_else(|| values.get("dur"));
                if let (Some(Value::Pitch(pitch)), Some(Value::Duration(duration))) =
                    (pitch, duration)
                {
                    self.push_play_note(*pitch, *duration, line, column, span, track);
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "ML_TYPE_MISMATCH",
                            "expected play dict `{p: pitch, d: duration}`",
                            line,
                            column,
                        )
                        .with_span(span),
                    );
                }
            }
            _ => self.diagnostics.push(
                Diagnostic::error(
                    "ML_TYPE_MISMATCH",
                    "expected playable phrase value",
                    line,
                    column,
                )
                .with_span(span),
            ),
        }
    }

    fn push_play_note(
        &mut self,
        pitch: Pitch,
        duration: Duration,
        line: usize,
        column: usize,
        span: Span,
        track: &mut TrackBuilder,
    ) {
        self.check_pitch_style(pitch, line, column, Some(span));
        self.check_rhythm_vocab(duration, line, column, Some(span));
        self.check_instrument_range(track.program(), pitch, line, column, Some(span));
        track.push_note(Note::new(pitch, duration), Some(span));
    }

    fn compile_drum(&mut self, drum: &DrumStmt, track: &mut TrackBuilder) {
        let Some(duration) = self.eval_duration(&drum.duration_expr, drum.line, drum.column) else {
            return;
        };
        let Some(midi) = drum_midi_number(&drum.name) else {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_THEORY_DRUM",
                    format!("unknown drum `{}`", drum.name),
                    drum.line,
                    drum.column,
                )
                .with_span(drum.span),
            );
            return;
        };
        self.check_rhythm_vocab(duration, drum.line, drum.column, Some(drum.span));
        track.push_midi_note(midi, duration, Some(drum.span));
    }

    fn compile_rest(&mut self, rest: &RestStmt, track: &mut TrackBuilder) {
        let Some(duration) = self.eval_duration(&rest.duration_expr, rest.line, rest.column) else {
            return;
        };
        self.check_rhythm_vocab(duration, rest.line, rest.column, Some(rest.span));
        track.advance(duration);
    }

    fn compile_glissando(&mut self, glissando: &GlissandoStmt, track: &mut TrackBuilder) {
        let Some(start) = self.eval_pitch(&glissando.start_expr, glissando.line, glissando.column)
        else {
            return;
        };
        let Some(end) = self.eval_pitch(&glissando.end_expr, glissando.line, glissando.column)
        else {
            return;
        };
        let Some(steps) = self.eval_glissando_steps(glissando) else {
            return;
        };
        let Some(duration) =
            self.eval_duration(&glissando.duration_expr, glissando.line, glissando.column)
        else {
            return;
        };
        self.check_rhythm_vocab(
            duration,
            glissando.line,
            glissando.column,
            Some(glissando.span),
        );
        let start_midi = i16::from(start.midi_number().expect("validated pitch"));
        let end_midi = i16::from(end.midi_number().expect("validated pitch"));
        for index in 0..steps {
            let midi = if steps == 1 {
                start_midi
            } else {
                start_midi + ((end_midi - start_midi) * index as i16) / (steps as i16 - 1)
            };
            let pitch = match Pitch::from_midi_number(midi) {
                Ok(pitch) => pitch,
                Err(error) => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "ML_CORE_PITCH",
                            error.to_string(),
                            glissando.line,
                            glissando.column,
                        )
                        .with_span(glissando.span),
                    );
                    return;
                }
            };
            self.check_pitch_style(
                pitch,
                glissando.line,
                glissando.column,
                Some(glissando.span),
            );
            self.check_instrument_range(
                track.program(),
                pitch,
                glissando.line,
                glissando.column,
                Some(glissando.span),
            );
            track.push_note(Note::new(pitch, duration), Some(glissando.span));
        }
    }

    fn eval_glissando_steps(&mut self, glissando: &GlissandoStmt) -> Option<usize> {
        match self.eval_expr(&glissando.steps_expr, glissando.line, glissando.column) {
            Some(Value::Int(value)) if value > 0 => Some(value as usize),
            Some(Value::Int(_)) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_THEORY_GLISSANDO",
                        "glissando steps must be positive",
                        glissando.line,
                        glissando.column,
                    )
                    .with_span(glissando.span),
                );
                None
            }
            Some(_) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "expected integer glissando steps",
                        glissando.line,
                        glissando.column,
                    )
                    .with_span(glissando.span),
                );
                None
            }
            None => None,
        }
    }

    fn compile_tremolo(&mut self, tremolo: &TremoloStmt, track: &mut TrackBuilder) {
        let Some(first) = self.eval_pitch(&tremolo.first_expr, tremolo.line, tremolo.column) else {
            return;
        };
        let Some(second) = self.eval_pitch(&tremolo.second_expr, tremolo.line, tremolo.column)
        else {
            return;
        };
        let Some(repeats) = self.eval_tremolo_repeats(tremolo) else {
            return;
        };
        let Some(duration) =
            self.eval_duration(&tremolo.duration_expr, tremolo.line, tremolo.column)
        else {
            return;
        };
        self.check_rhythm_vocab(duration, tremolo.line, tremolo.column, Some(tremolo.span));
        for index in 0..repeats {
            let pitch = if index % 2 == 0 { first } else { second };
            self.check_pitch_style(pitch, tremolo.line, tremolo.column, Some(tremolo.span));
            self.check_instrument_range(
                track.program(),
                pitch,
                tremolo.line,
                tremolo.column,
                Some(tremolo.span),
            );
            track.push_note(Note::new(pitch, duration), Some(tremolo.span));
        }
    }

    fn eval_tremolo_repeats(&mut self, tremolo: &TremoloStmt) -> Option<usize> {
        match self.eval_expr(&tremolo.repeats_expr, tremolo.line, tremolo.column) {
            Some(Value::Int(value)) if value > 0 => Some(value as usize),
            Some(Value::Int(_)) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_THEORY_TREMOLO",
                        "tremolo repeats must be positive",
                        tremolo.line,
                        tremolo.column,
                    )
                    .with_span(tremolo.span),
                );
                None
            }
            Some(_) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "expected integer tremolo repeats",
                        tremolo.line,
                        tremolo.column,
                    )
                    .with_span(tremolo.span),
                );
                None
            }
            None => None,
        }
    }

    fn compile_degree(&mut self, degree: &DegreeStmt, track: &mut TrackBuilder) {
        let Some(pitch) = self.scale_degree_pitch(degree) else {
            return;
        };
        let Some(duration) = self.eval_duration(&degree.duration_expr, degree.line, degree.column)
        else {
            return;
        };
        self.check_pitch_style(pitch, degree.line, degree.column, Some(degree.span));
        self.check_rhythm_vocab(duration, degree.line, degree.column, Some(degree.span));
        self.check_instrument_range(
            track.program(),
            pitch,
            degree.line,
            degree.column,
            Some(degree.span),
        );
        let start_tick = track.cursor_tick();
        let duration_ticks = track.scaled_ticks(duration);
        track.push_note(Note::new(pitch, duration), Some(degree.span));
        if let Some((degree_index, accidental)) = parse_scale_degree(&degree.degree) {
            self.melodic_events.push(MelodicEventIr {
                kind: "scale_degree".to_string(),
                degree: Some((degree_index + 1) as u8),
                accidental: accidental as i8,
                pitch,
                start_tick,
                duration_ticks,
                source_span: Some(degree.span),
            });
        }
    }

    fn scale_degree_pitch(&mut self, degree: &DegreeStmt) -> Option<Pitch> {
        let Some(key) = self.score_key else {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_THEORY_DEGREE",
                    "scale degree requires score key metadata",
                    degree.line,
                    degree.column,
                )
                .with_span(degree.span),
            );
            return None;
        };
        let Some((index, accidental)) = parse_scale_degree(&degree.degree) else {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_THEORY_DEGREE",
                    format!("unsupported scale degree `{}`", degree.degree),
                    degree.line,
                    degree.column,
                )
                .with_span(degree.span),
            );
            return None;
        };
        let scale = key_scale_pattern(key);
        let semitone = key_tonic_semitone(key) + scale[index] + accidental;
        let pitch_class = PitchClass::from_semitone(semitone);
        let octave = degree.octave.clamp(i8::MIN as i32, i8::MAX as i32) as i8;
        let pitch = match Pitch::new(pitch_class, octave) {
            Ok(pitch) => pitch,
            Err(error) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_CORE_PITCH",
                        error.to_string(),
                        degree.line,
                        degree.column,
                    )
                    .with_span(degree.span),
                );
                return None;
            }
        };
        self.apply_pitch_transpose(pitch, degree.line, degree.column, degree.span)
    }

    fn compile_scale(&mut self, scale: &ScaleStmt, track: &mut TrackBuilder) {
        let Some(duration) = self.eval_duration(&scale.duration_expr, scale.line, scale.column)
        else {
            return;
        };
        let Some(pitches) = self.scale_run_pitches(scale) else {
            return;
        };
        self.check_rhythm_vocab(duration, scale.line, scale.column, Some(scale.span));
        for (index, pitch) in pitches.into_iter().enumerate() {
            self.check_pitch_style(pitch, scale.line, scale.column, Some(scale.span));
            self.check_instrument_range(
                track.program(),
                pitch,
                scale.line,
                scale.column,
                Some(scale.span),
            );
            let start_tick = track.cursor_tick();
            let duration_ticks = track.scaled_ticks(duration);
            track.push_note(Note::new(pitch, duration), Some(scale.span));
            self.melodic_events.push(MelodicEventIr {
                kind: "scale_run".to_string(),
                degree: Some((index % 7 + 1) as u8),
                accidental: 0,
                pitch,
                start_tick,
                duration_ticks,
                source_span: Some(scale.span),
            });
        }
    }

    fn scale_run_pitches(&mut self, scale: &ScaleStmt) -> Option<Vec<Pitch>> {
        let tonic = scale.tonic.parse::<PitchClass>().ok().or_else(|| {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_THEORY_SCALE",
                    format!("unsupported scale tonic `{}`", scale.tonic),
                    scale.line,
                    scale.column,
                )
                .with_span(scale.span),
            );
            None
        })?;
        let Some(pattern) = scale_mode_pattern(&scale.mode) else {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_THEORY_SCALE",
                    format!("unsupported scale mode `{}`", scale.mode),
                    scale.line,
                    scale.column,
                )
                .with_span(scale.span),
            );
            return None;
        };
        let mut semitone = tonic.semitone();
        let mut pitches = Vec::with_capacity(pattern.len() + 1);
        pitches.push(self.scale_run_pitch(semitone, scale)?);
        for step in pattern {
            semitone += step;
            pitches.push(self.scale_run_pitch(semitone, scale)?);
        }
        Some(pitches)
    }

    fn scale_run_pitch(&mut self, semitone: i16, scale: &ScaleStmt) -> Option<Pitch> {
        let pitch_class = PitchClass::from_semitone(semitone);
        let octave = scale.octave + i32::from(semitone.div_euclid(12));
        let octave = octave.clamp(i8::MIN as i32, i8::MAX as i32) as i8;
        let pitch = match Pitch::new(pitch_class, octave) {
            Ok(pitch) => pitch,
            Err(error) => {
                self.diagnostics.push(
                    Diagnostic::error("ML_CORE_PITCH", error.to_string(), scale.line, scale.column)
                        .with_span(scale.span),
                );
                return None;
            }
        };
        self.apply_pitch_transpose(pitch, scale.line, scale.column, scale.span)
    }

    fn compile_pedal(&mut self, pedal: &PedalStmt, track: &mut TrackBuilder) {
        let Some(pitch) = self.eval_pitch(&pedal.pitch_expr, pedal.line, pedal.column) else {
            return;
        };
        let Some(duration) = self.eval_duration(&pedal.duration_expr, pedal.line, pedal.column)
        else {
            return;
        };
        let Some(count) = self.eval_pedal_count(pedal) else {
            return;
        };
        self.check_pitch_style(pitch, pedal.line, pedal.column, Some(pedal.span));
        self.check_rhythm_vocab(duration, pedal.line, pedal.column, Some(pedal.span));
        self.check_instrument_range(
            track.program(),
            pitch,
            pedal.line,
            pedal.column,
            Some(pedal.span),
        );
        for _ in 0..count {
            track.push_note(Note::new(pitch, duration), Some(pedal.span));
        }
    }

    fn eval_pedal_count(&mut self, pedal: &PedalStmt) -> Option<usize> {
        match self.eval_expr(&pedal.count_expr, pedal.line, pedal.column) {
            Some(Value::Int(value)) if value > 0 => Some(value as usize),
            Some(Value::Int(_)) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_THEORY_PEDAL",
                        "pedal count must be positive",
                        pedal.line,
                        pedal.column,
                    )
                    .with_span(pedal.span),
                );
                None
            }
            Some(_) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "expected integer pedal count",
                        pedal.line,
                        pedal.column,
                    )
                    .with_span(pedal.span),
                );
                None
            }
            None => None,
        }
    }

    fn compile_ostinato(&mut self, ostinato: &OstinatoStmt, track: &mut TrackBuilder) {
        let Some(count) = self.eval_ostinato_count(ostinato) else {
            return;
        };
        for _ in 0..count {
            self.compile_statements(&ostinato.statements, track);
        }
    }

    fn eval_ostinato_count(&mut self, ostinato: &OstinatoStmt) -> Option<usize> {
        match self.eval_expr(&ostinato.count_expr, ostinato.line, ostinato.column) {
            Some(Value::Int(value)) if value > 0 => Some(value as usize),
            Some(Value::Int(_)) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_THEORY_OSTINATO",
                        "ostinato count must be positive",
                        ostinato.line,
                        ostinato.column,
                    )
                    .with_span(ostinato.span),
                );
                None
            }
            Some(_) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "expected integer ostinato count",
                        ostinato.line,
                        ostinato.column,
                    )
                    .with_span(ostinato.span),
                );
                None
            }
            None => None,
        }
    }

    fn compile_sequence(&mut self, sequence: &SequenceStmt, track: &mut TrackBuilder) {
        let Some(count) = self.eval_sequence_count(sequence) else {
            return;
        };
        let Some(interval) = self.eval_interval(
            &sequence.interval_expr,
            sequence.line,
            sequence.column,
            "expected interval sequence step",
        ) else {
            return;
        };
        let base_transpose = self.pitch_transpose_semitones;
        for index in 0..count {
            self.pitch_transpose_semitones = base_transpose + interval.semitones() * index as i16;
            self.compile_statements(&sequence.statements, track);
        }
        self.pitch_transpose_semitones = base_transpose;
    }

    fn compile_transpose(&mut self, transpose: &TransposeStmt, track: &mut TrackBuilder) {
        let Some(interval) = self.eval_interval(
            &transpose.interval_expr,
            transpose.line,
            transpose.column,
            "expected interval transpose amount",
        ) else {
            return;
        };
        let base_transpose = self.pitch_transpose_semitones;
        self.pitch_transpose_semitones = base_transpose + interval.semitones();
        self.compile_statements(&transpose.statements, track);
        self.pitch_transpose_semitones = base_transpose;
    }

    fn compile_tuplet(&mut self, tuplet: &TupletStmt, track: &mut TrackBuilder) {
        let Some(count) = self.eval_tuplet_count(tuplet) else {
            return;
        };
        let Some(space) = self.eval_duration(&tuplet.space_expr, tuplet.line, tuplet.column) else {
            return;
        };
        self.check_rhythm_vocab(space, tuplet.line, tuplet.column, Some(tuplet.span));
        track.push_time_scale(count as u32, space.ticks(DEFAULT_TICKS_PER_QUARTER));
        self.compile_statements(&tuplet.statements, track);
        track.pop_time_scale();
    }

    fn eval_sequence_count(&mut self, sequence: &SequenceStmt) -> Option<usize> {
        match self.eval_expr(&sequence.count_expr, sequence.line, sequence.column) {
            Some(Value::Int(value)) if value > 0 => Some(value as usize),
            Some(Value::Int(_)) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_THEORY_SEQUENCE",
                        "sequence count must be positive",
                        sequence.line,
                        sequence.column,
                    )
                    .with_span(sequence.span),
                );
                None
            }
            Some(_) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "expected integer sequence count",
                        sequence.line,
                        sequence.column,
                    )
                    .with_span(sequence.span),
                );
                None
            }
            None => None,
        }
    }

    fn eval_tuplet_count(&mut self, tuplet: &TupletStmt) -> Option<usize> {
        match self.eval_expr(&tuplet.count_expr, tuplet.line, tuplet.column) {
            Some(Value::Int(value)) if value > 0 => Some(value as usize),
            Some(Value::Int(_)) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_THEORY_TUPLET",
                        "tuplet count must be positive",
                        tuplet.line,
                        tuplet.column,
                    )
                    .with_span(tuplet.span),
                );
                None
            }
            Some(_) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "expected integer tuplet count",
                        tuplet.line,
                        tuplet.column,
                    )
                    .with_span(tuplet.span),
                );
                None
            }
            None => None,
        }
    }

    fn compile_chord(&mut self, chord: &ChordStmt, track: &mut TrackBuilder) {
        let context = ChordPitchContext {
            line: chord.line,
            column: chord.column,
            span: chord.span,
            program: track.program(),
        };
        let pitches = self.collect_chord_pitches(
            chord.root_expr.as_ref(),
            chord.quality.as_deref(),
            chord.inversion,
            &chord.pitch_exprs,
            context,
        );
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

    fn compile_arpeggio(&mut self, arpeggio: &ArpeggioStmt, track: &mut TrackBuilder) {
        let context = ChordPitchContext {
            line: arpeggio.line,
            column: arpeggio.column,
            span: arpeggio.span,
            program: track.program(),
        };
        let pitches = self.collect_chord_pitches(
            arpeggio.root_expr.as_ref(),
            arpeggio.quality.as_deref(),
            arpeggio.inversion,
            &arpeggio.pitch_exprs,
            context,
        );
        let Some(duration) =
            self.eval_duration(&arpeggio.duration_expr, arpeggio.line, arpeggio.column)
        else {
            return;
        };
        self.check_chord_vocab(
            &pitches,
            arpeggio.line,
            arpeggio.column,
            Some(arpeggio.span),
        );
        self.check_chord_quality_vocab(
            &pitches,
            arpeggio.line,
            arpeggio.column,
            Some(arpeggio.span),
        );
        self.check_set_class_vocab(
            &pitches,
            arpeggio.line,
            arpeggio.column,
            Some(arpeggio.span),
        );
        self.check_rhythm_vocab(
            duration,
            arpeggio.line,
            arpeggio.column,
            Some(arpeggio.span),
        );
        for pitch in pitches {
            track.push_note(Note::new(pitch, duration), Some(arpeggio.span));
        }
    }

    fn compile_strum(&mut self, strum: &StrumStmt, track: &mut TrackBuilder) {
        let context = ChordPitchContext {
            line: strum.line,
            column: strum.column,
            span: strum.span,
            program: track.program(),
        };
        let pitches = self.collect_chord_pitches(
            strum.root_expr.as_ref(),
            strum.quality.as_deref(),
            strum.inversion,
            &strum.pitch_exprs,
            context,
        );
        let Some(duration) = self.eval_duration(&strum.duration_expr, strum.line, strum.column)
        else {
            return;
        };
        let Some(offset) = self.eval_duration(&strum.offset_expr, strum.line, strum.column) else {
            return;
        };
        self.check_chord_vocab(&pitches, strum.line, strum.column, Some(strum.span));
        self.check_chord_quality_vocab(&pitches, strum.line, strum.column, Some(strum.span));
        self.check_set_class_vocab(&pitches, strum.line, strum.column, Some(strum.span));
        self.check_rhythm_vocab(duration, strum.line, strum.column, Some(strum.span));
        self.check_rhythm_vocab(offset, strum.line, strum.column, Some(strum.span));
        track.push_strum(&pitches, duration, offset, Some(strum.span));
    }

    fn collect_chord_pitches(
        &mut self,
        root_expr: Option<&Expr>,
        quality: Option<&str>,
        inversion: Option<usize>,
        pitch_exprs: &[Expr],
        context: ChordPitchContext,
    ) -> Vec<Pitch> {
        let mut pitches = Vec::new();
        if let (Some(root_expr), Some(quality)) = (root_expr, quality) {
            if let Some(root) = self.eval_pitch(root_expr, context.line, context.column) {
                if let Some(mut expanded) = expand_chord_quality(root, quality) {
                    if let Some(inversion) = inversion {
                        if invert_chord(&mut expanded, inversion).is_none() {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "ML_THEORY_CHORD_INVERSION",
                                    format!("unsupported chord inversion `{inversion}`"),
                                    context.line,
                                    context.column,
                                )
                                .with_span(context.span),
                            );
                        }
                    }
                    pitches.extend(expanded);
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "ML_THEORY_CHORD_QUALITY",
                            format!("unknown chord quality `{quality}`"),
                            context.line,
                            context.column,
                        )
                        .with_span(context.span),
                    );
                }
            }
        }
        for pitch_expr in pitch_exprs {
            match self.eval_expr(pitch_expr, context.line, context.column) {
                Some(Value::Pitch(pitch)) => {
                    if let Some(pitch) = self.apply_pitch_transpose(
                        pitch,
                        context.line,
                        context.column,
                        pitch_expr.span,
                    ) {
                        pitches.push(pitch);
                    }
                }
                Some(Value::List(values)) => {
                    for value in values {
                        if let Value::Pitch(pitch) = value {
                            if let Some(pitch) = self.apply_pitch_transpose(
                                pitch,
                                context.line,
                                context.column,
                                pitch_expr.span,
                            ) {
                                pitches.push(pitch);
                            }
                        } else {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    "ML_TYPE_MISMATCH",
                                    "expected pitch expression",
                                    context.line,
                                    context.column,
                                )
                                .with_span(context.span),
                            );
                        }
                    }
                }
                Some(_) => self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "expected pitch expression",
                        context.line,
                        context.column,
                    )
                    .with_span(context.span),
                ),
                None => {}
            }
        }
        for pitch in &pitches {
            self.check_pitch_style(*pitch, context.line, context.column, Some(context.span));
            self.check_instrument_range(
                context.program,
                *pitch,
                context.line,
                context.column,
                Some(context.span),
            );
        }
        pitches
    }

    fn compile_roman(&mut self, roman: &RomanStmt, track: &mut TrackBuilder) {
        let Some(duration) = self.eval_duration(&roman.duration_expr, roman.line, roman.column)
        else {
            return;
        };
        self.compile_roman_symbol(
            &roman.symbol,
            duration,
            roman.line,
            roman.column,
            roman.span,
            track,
        );
    }

    fn compile_progression(&mut self, progression: &ProgressionStmt, track: &mut TrackBuilder) {
        let Some(duration) = self.eval_duration(
            &progression.duration_expr,
            progression.line,
            progression.column,
        ) else {
            return;
        };
        for symbol in &progression.symbols {
            self.compile_roman_symbol(
                symbol,
                duration,
                progression.line,
                progression.column,
                progression.span,
                track,
            );
        }
    }

    fn compile_cadence(&mut self, cadence: &CadenceStmt, track: &mut TrackBuilder) {
        let Some(duration) =
            self.eval_duration(&cadence.duration_expr, cadence.line, cadence.column)
        else {
            return;
        };
        let Some(symbols) = cadence_symbols(&cadence.kind) else {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_THEORY_CADENCE",
                    format!("unsupported cadence `{}`", cadence.kind),
                    cadence.line,
                    cadence.column,
                )
                .with_span(cadence.span),
            );
            return;
        };
        for symbol in symbols {
            self.compile_roman_symbol(
                symbol,
                duration,
                cadence.line,
                cadence.column,
                cadence.span,
                track,
            );
        }
    }

    fn key_signature_or_diagnostic(
        &mut self,
        tonic: &str,
        mode: &str,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<KeySignature> {
        let Some(key) = key_signature(tonic, mode) else {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_THEORY_KEY",
                    format!("unsupported key `{} {}`", tonic, mode),
                    line,
                    column,
                )
                .with_span(span),
            );
            return None;
        };
        Some(key)
    }

    fn compile_modulate(&mut self, modulate: &ModulateStmt) {
        if let Some(key) = self.key_signature_or_diagnostic(
            &modulate.tonic,
            &modulate.mode,
            modulate.line,
            modulate.column,
            modulate.span,
        ) {
            self.score_key = Some(key);
        }
    }

    fn compile_roman_symbol(
        &mut self,
        symbol: &str,
        duration: Duration,
        line: usize,
        column: usize,
        span: Span,
        track: &mut TrackBuilder,
    ) {
        let Some(key) = self.score_key else {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_THEORY_ROMAN_KEY",
                    "roman numeral chord requires score key metadata",
                    line,
                    column,
                )
                .with_span(span),
            );
            return;
        };
        let Some(pitches) = roman_chord_pitches(symbol, key) else {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_THEORY_ROMAN_NUMERAL",
                    format!("unsupported roman numeral chord `{symbol}`"),
                    line,
                    column,
                )
                .with_span(span),
            );
            return;
        };
        for pitch in &pitches {
            self.check_pitch_style(*pitch, line, column, Some(span));
            self.check_instrument_range(track.program(), *pitch, line, column, Some(span));
        }
        self.check_chord_vocab(&pitches, line, column, Some(span));
        self.check_chord_quality_vocab(&pitches, line, column, Some(span));
        self.check_set_class_vocab(&pitches, line, column, Some(span));
        self.check_rhythm_vocab(duration, line, column, Some(span));
        match Chord::new(pitches, duration) {
            Ok(compiled_chord) => {
                let start_tick = track.cursor_tick();
                let duration_ticks = track.scaled_ticks(duration);
                track.push_chord(compiled_chord, Some(span));
                let analysis = analyze_roman_symbol(symbol);
                self.harmonic_events.push(HarmonicEventIr {
                    symbol: symbol.to_string(),
                    normalized_symbol: analysis.normalized_symbol,
                    degree: analysis.degree,
                    applied_to: analysis.applied_to,
                    function: analysis.function.map(ToString::to_string),
                    cadence_role: analysis.cadence_role.map(ToString::to_string),
                    start_tick,
                    duration_ticks,
                    source_span: Some(span),
                });
            }
            Err(error) => self.diagnostics.push(
                Diagnostic::error("ML_CORE_CHORD", error.to_string(), line, column).with_span(span),
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
                    .with_style(self.style.name.clone())
                    .with_help(format!("adjust the active style rule `{rule}` or use an explicit audited override for intentional local exceptions"));
                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(span);
                }
                self.diagnostics.push(diagnostic);
            }
            RuleSeverity::Warning => {
                let mut diagnostic = Diagnostic::warning(code, message, line, column)
                    .with_rule(rule)
                    .with_style(self.style.name.clone())
                    .with_help(format!("adjust the active style rule `{rule}` or use an explicit audited override for intentional local exceptions"));
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
        let pitched_tracks = tracks
            .iter()
            .filter(|track| track.channel != 9)
            .collect::<Vec<_>>();
        if pitched_tracks.len() < 2 {
            return;
        }
        for upper_index in 0..pitched_tracks.len() {
            for lower_index in (upper_index + 1)..pitched_tracks.len() {
                self.check_voice_crossing(pitched_tracks[upper_index], pitched_tracks[lower_index]);
                self.check_voice_spacing(pitched_tracks[upper_index], pitched_tracks[lower_index]);
                self.check_parallel_fifths(
                    pitched_tracks[upper_index],
                    pitched_tracks[lower_index],
                );
                self.check_contrapuntal_motion(
                    pitched_tracks[upper_index],
                    pitched_tracks[lower_index],
                );
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

    fn check_voice_spacing(&mut self, upper: &TrackIr, lower: &TrackIr) {
        let Some(max_spacing) = self.style.max_voice_spacing else {
            return;
        };
        if self.has_override("voice_spacing") || self.has_score_override("voice_spacing") {
            return;
        }
        let max_spacing = max_spacing.semitones().abs();
        for upper_event in &upper.events {
            for lower_event in &lower.events {
                let Some(upper_pitch) = upper_event.pitch.midi_number().ok() else {
                    continue;
                };
                let Some(lower_pitch) = lower_event.pitch.midi_number().ok() else {
                    continue;
                };
                if upper_event.start_tick == lower_event.start_tick {
                    let spacing = (i16::from(upper_pitch) - i16::from(lower_pitch)).abs();
                    if spacing > max_spacing {
                        self.push_style_diagnostic(
                            "voice_spacing",
                            "ML_STYLE_VOICE_SPACING",
                            format!(
                                "voices `{}` and `{}` are spaced {spacing} semitones apart, exceeding maximum {max_spacing}",
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

    fn check_melodic_concepts(&mut self, tracks: &[TrackIr], melodic_events: &[MelodicEventIr]) {
        if self.style.melodic_concepts.is_empty()
            || self.has_override("melodic_concept")
            || self.has_score_override("melodic_concept")
        {
            return;
        }
        for concept in self.style.melodic_concepts.clone() {
            let valid = match concept.as_str() {
                "blues_inflection" => explicit_blues_inflection(melodic_events)
                    .unwrap_or_else(|| tracks.iter().any(track_has_blues_inflection)),
                _ => true,
            };
            if !valid {
                self.push_style_diagnostic(
                    "melodic_concept",
                    "ML_STYLE_MELODIC_CONCEPT",
                    format!("score melody does not satisfy required {concept} concept"),
                    self.program.score.line,
                    self.program.score.column,
                );
                return;
            }
        }
    }

    fn check_phrase_concepts(
        &mut self,
        phrase_events: &[PhraseEventIr],
        motif_events: &[MotifEventIr],
    ) {
        if self.style.phrase_concepts.is_empty() || self.has_score_override("phrase_concept") {
            return;
        }
        for concept in self.style.phrase_concepts.clone() {
            let valid = match concept.as_str() {
                "periodic_phrase" => {
                    let section_count = phrase_events
                        .iter()
                        .filter(|event| event.kind == "section")
                        .count();
                    let checked_section_count = phrase_events
                        .iter()
                        .enumerate()
                        .filter(|(index, event)| {
                            event.kind == "section"
                                && !self.phrase_concept_override_phrases.contains(index)
                        })
                        .count();
                    (section_count > 0 && checked_section_count == 0) || checked_section_count >= 2
                }
                "motivic_development" => {
                    let checked_motif_count = motif_events
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| !self.phrase_concept_override_motifs.contains(index))
                        .count();
                    (checked_motif_count == 0 && !motif_events.is_empty())
                        || motif_events
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| {
                                !self.phrase_concept_override_motifs.contains(index)
                            })
                            .filter_map(|(_, event)| event.transform.as_deref())
                            .any(|transform| transform != "literal")
                }
                _ => true,
            };
            if !valid {
                self.push_style_diagnostic(
                    "phrase_concept",
                    "ML_STYLE_PHRASE_CONCEPT",
                    format!("score phrase structure does not satisfy required {concept} concept"),
                    self.program.score.line,
                    self.program.score.column,
                );
                return;
            }
        }
    }

    fn check_ensemble_concepts(&mut self, tracks: &[TrackIr]) {
        if self.style.ensemble_concepts.is_empty()
            || self.has_override("ensemble_concept")
            || self.has_score_override("ensemble_concept")
        {
            return;
        }
        for concept in self.style.ensemble_concepts.clone() {
            let valid = match concept.as_str() {
                "call_response" => tracks_have_call_response(tracks),
                _ => true,
            };
            if !valid {
                self.push_style_diagnostic(
                    "ensemble_concept",
                    "ML_STYLE_ENSEMBLE_CONCEPT",
                    format!("score ensemble writing does not satisfy required {concept} concept"),
                    self.program.score.line,
                    self.program.score.column,
                );
                return;
            }
        }
    }

    fn check_bass_concepts(&mut self, tracks: &[TrackIr]) {
        if self.style.bass_concepts.is_empty()
            || self.has_override("bass_concept")
            || self.has_score_override("bass_concept")
        {
            return;
        }
        for concept in self.style.bass_concepts.clone() {
            let valid = match concept.as_str() {
                "walking_or_riff_bass" => tracks.iter().any(track_has_walking_or_riff_bass),
                _ => true,
            };
            if !valid {
                self.push_style_diagnostic(
                    "bass_concept",
                    "ML_STYLE_BASS_CONCEPT",
                    format!("score bass writing does not satisfy required {concept} concept"),
                    self.program.score.line,
                    self.program.score.column,
                );
                return;
            }
        }
    }

    fn check_form(&mut self, form_events: &[FormEventIr]) {
        let Some(form) = self.style.form.clone() else {
            return;
        };
        if self.has_override("form") || self.has_score_override("form") {
            return;
        }
        let labels = form_events
            .iter()
            .filter(|event| event.kind == "section")
            .map(|event| event.label.clone())
            .collect::<Vec<_>>();
        if !form_labels_match_catalog(&labels, &form) {
            self.push_style_diagnostic(
                "form",
                "ML_STYLE_FORM",
                format!("score sections do not satisfy `{form}` form"),
                self.program.score.line,
                self.program.score.column,
            );
        }
    }

    fn check_harmonic_progression(
        &mut self,
        tracks: &[TrackIr],
        harmonic_events: &[HarmonicEventIr],
    ) {
        if self.style.harmonic_progression.is_empty()
            || self.has_override("harmonic_progression")
            || self.has_score_override("harmonic_progression")
        {
            return;
        }
        let actual = explicit_harmonic_functions(harmonic_events)
            .filter(|functions| !functions.is_empty())
            .unwrap_or_else(|| harmonic_functions(tracks));
        let expected = self.style.harmonic_progression.as_slice();
        if actual.len() < expected.len() {
            self.push_style_diagnostic(
                "harmonic_progression",
                "ML_STYLE_HARMONIC_PROGRESSION",
                format!(
                    "score does not contain enough functional harmony for required progression `{}`",
                    expected.join(" ")
                ),
                self.program.score.line,
                self.program.score.column,
            );
            return;
        }
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

    fn check_cadence(&mut self, tracks: &[TrackIr], harmonic_events: &[HarmonicEventIr]) {
        if self.style.cadences.is_empty()
            || self.has_override("cadence")
            || self.has_score_override("cadence")
        {
            return;
        }
        let explicit = final_harmonic_symbols(harmonic_events);
        let sonorities = explicit
            .is_none()
            .then(|| final_sonorities(tracks))
            .flatten();
        if !self.style.cadences.iter().any(|cadence| {
            explicit
                .as_ref()
                .is_some_and(|(penultimate, final_symbol)| {
                    cadence_matches_symbols(cadence, penultimate, final_symbol)
                })
                || sonorities
                    .as_ref()
                    .is_some_and(|(penultimate, final_sonority)| {
                        cadence_matches(cadence, penultimate, final_sonority)
                    })
        }) {
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
        self.check_tempo_style(
            tempo_bpm,
            self.program.score.line,
            self.program.score.column,
            Some(self.program.score.span),
        );
        if let Some(meter) = meter {
            self.check_meter_style(
                meter,
                self.program.score.line,
                self.program.score.column,
                Some(self.program.score.span),
            );
        }
    }

    fn check_tempo_style(
        &mut self,
        tempo_bpm: u16,
        line: usize,
        column: usize,
        span: Option<Span>,
    ) {
        if let Some((min, max)) = self.style.tempo_range {
            if (tempo_bpm < min || tempo_bpm > max) && !self.has_override("tempo_range") {
                self.push_style_diagnostic_with_span(
                    "tempo_range",
                    "ML_STYLE_TEMPO_RANGE",
                    format!("tempo {tempo_bpm} is outside active style tempo range {min}..={max}"),
                    line,
                    column,
                    span,
                );
            }
        }
    }

    fn check_meter_style(&mut self, actual: Meter, line: usize, column: usize, span: Option<Span>) {
        if let Some(expected) = self.style.meter {
            if expected != actual && !self.has_override("meter") {
                self.push_style_diagnostic_with_span(
                    "meter",
                    "ML_STYLE_METER",
                    format!(
                        "meter {}/{} does not match active style meter {}/{}",
                        actual.numerator,
                        actual.denominator,
                        expected.numerator,
                        expected.denominator
                    ),
                    line,
                    column,
                    span,
                );
            }
        }
        if !self.style.meter_catalog.is_empty()
            && !self.has_override("meter_catalog")
            && !self
                .style
                .meter_catalog
                .iter()
                .any(|entry_id| meter_matches_catalog(actual, entry_id))
        {
            self.push_style_diagnostic_with_span(
                "meter_catalog",
                "ML_STYLE_METER_CATALOG",
                format!(
                    "meter {}/{} is outside active style meter catalog",
                    actual.numerator, actual.denominator
                ),
                line,
                column,
                span,
            );
        }
    }

    fn compile_override_tracks(&mut self, override_stmt: &OverrideStmt, tracks: &mut Vec<TrackIr>) {
        if !self.is_known_rule(&override_stmt.rule) {
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
                    let mut track = TrackBuilder::new(
                        &voice.name,
                        voice.program,
                        voice.channel,
                        voice.volume,
                        voice.pan,
                    );
                    self.compile_voice(voice, &mut track);
                    tracks.push(track.finish());
                }
                other => {
                    let mut track = TrackBuilder::new("main", None, None, None, None);
                    self.compile_statement(other, &mut track);
                    if !track.is_empty() {
                        tracks.push(track.finish());
                    }
                }
            }
        }
        self.override_rules.pop();
    }

    fn compile_override(&mut self, override_stmt: &OverrideStmt, track: &mut TrackBuilder) {
        if !self.is_known_rule(&override_stmt.rule) {
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
        let start_phrase_event = self.phrase_events.len();
        let start_motif_event = self.motif_events.len();
        self.compile_statements(&override_stmt.statements, track);
        track.mark_rule_override(start_event, &override_stmt.rule);
        if override_stmt.rule == "phrase_concept" {
            self.phrase_concept_override_phrases
                .extend(start_phrase_event..self.phrase_events.len());
            self.phrase_concept_override_motifs
                .extend(start_motif_event..self.motif_events.len());
        }
        self.override_rules.pop();
    }

    fn eval_expr(&mut self, expr: &Expr, line: usize, column: usize) -> Option<Value> {
        match &expr.kind {
            ExprKind::Ident(name) => self.resolve_token(name).or_else(|| {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_RESOLVE_UNKNOWN_NAME",
                        format!("unknown name `{name}`"),
                        line,
                        column,
                    )
                    .with_span(expr.span)
                    .with_help(
                        "define the referenced name before using it or correct the identifier",
                    ),
                );
                None
            }),
            ExprKind::Int(value) => Some(Value::Int(*value)),
            ExprKind::Bool(value) => Some(Value::Bool(*value)),
            ExprKind::PitchLiteral(value) => match value.parse() {
                Ok(pitch) => Some(Value::Pitch(pitch)),
                Err(error) => {
                    self.diagnostics.push(
                        Diagnostic::error("ML_CORE_PITCH", error.to_string(), line, column)
                            .with_span(expr.span),
                    );
                    None
                }
            },
            ExprKind::IntervalLiteral(value) => match value.parse() {
                Ok(interval) => Some(Value::Interval(interval)),
                Err(error) => {
                    self.diagnostics.push(
                        Diagnostic::error("ML_CORE_INTERVAL", error.to_string(), line, column)
                            .with_span(expr.span),
                    );
                    None
                }
            },
            ExprKind::DurationLiteral(value) => match value.parse() {
                Ok(duration) => Some(Value::Duration(duration)),
                Err(error) => {
                    self.diagnostics.push(
                        Diagnostic::error("ML_CORE_DURATION", error.to_string(), line, column)
                            .with_span(expr.span),
                    );
                    None
                }
            },
            ExprKind::StringLiteral(value) => Some(Value::String(value.clone())),
            ExprKind::List(values) => values
                .iter()
                .map(|value| self.eval_expr(value, line, column))
                .collect::<Option<Vec<_>>>()
                .map(Value::List),
            ExprKind::ListComprehension {
                item,
                binding,
                source,
                condition,
            } => {
                self.eval_list_comprehension(item, binding, source, condition.as_deref(), expr.span)
            }
            ExprKind::Tuple(values) => values
                .iter()
                .map(|value| self.eval_expr(value, line, column))
                .collect::<Option<Vec<_>>>()
                .map(Value::Tuple),
            ExprKind::Dict(entries) => entries
                .iter()
                .map(|(key, value)| {
                    self.eval_expr(value, line, column)
                        .map(|value| (key.clone(), value))
                })
                .collect::<Option<HashMap<_, _>>>()
                .map(Value::Dict),
            ExprKind::Conditional {
                condition,
                then_branch,
                else_branch,
            } => match self.eval_expr(condition, line, column)? {
                Value::Bool(true) => self.eval_expr(then_branch, line, column),
                Value::Bool(false) => self.eval_expr(else_branch, line, column),
                _ => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "ML_TYPE_MISMATCH",
                            "expected conditional expression to use bool condition",
                            line,
                            column,
                        )
                        .with_span(condition.span),
                    );
                    None
                }
            },
            ExprKind::Access { target, key } => {
                let target = self.eval_expr(target, line, column)?;
                self.eval_access(target, key, line, column, expr.span)
            }
            ExprKind::MethodCall {
                target,
                method,
                args,
            } => {
                let value = self.eval_expr(target, line, column)?;
                if is_phrase_function_transform(method) && args.len() == 1 {
                    let function_name = self.expr_function_name(&args[0], line, column)?;
                    return self.eval_phrase_function_transform(
                        method,
                        value,
                        &function_name,
                        line,
                        column,
                        expr.span,
                    );
                }
                let mut values = Vec::with_capacity(args.len() + 1);
                values.push(value);
                values.extend(
                    args.iter()
                        .map(|arg| self.eval_expr(arg, line, column))
                        .collect::<Option<Vec<_>>>()?,
                );
                self.eval_call(method, values, line, column, expr.span)
            }
            ExprKind::Pipe { value, call } => {
                let value = self.eval_expr(value, line, column)?;
                self.eval_pipe(value, call, line, column, expr.span)
            }
            ExprKind::Call { callee, args } => {
                if is_phrase_function_transform(callee) && args.len() == 2 {
                    let value = self.eval_expr(&args[0], line, column)?;
                    let function_name = self.expr_function_name(&args[1], line, column)?;
                    return self.eval_phrase_function_transform(
                        callee,
                        value,
                        &function_name,
                        line,
                        column,
                        expr.span,
                    );
                }
                let args = args
                    .iter()
                    .map(|arg| self.eval_expr(arg, line, column))
                    .collect::<Option<Vec<_>>>()?;
                self.eval_call(callee, args, line, column, expr.span)
            }
            ExprKind::Unary { op, expr } => {
                let value = self.eval_expr(expr, line, column)?;
                self.eval_unary(*op, value, line, column, expr.span)
            }
            ExprKind::Range { start, end } => self.eval_range(start, end, line, column, expr.span),
            ExprKind::Binary { op, left, right } => {
                let left = self.eval_expr(left, line, column)?;
                let right = self.eval_expr(right, line, column)?;
                self.eval_binary(*op, left, right, line, column, expr.span)
            }
        }
    }

    fn expr_function_name(&mut self, expr: &Expr, line: usize, column: usize) -> Option<String> {
        match &expr.kind {
            ExprKind::Ident(name)
                if self
                    .functions
                    .get(name)
                    .and_then(|function| function.body_expr())
                    .is_some() =>
            {
                Some(name.clone())
            }
            ExprKind::StringLiteral(name) => Some(name.clone()),
            ExprKind::Ident(name) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_RESOLVE_UNKNOWN_NAME",
                        format!("unknown function `{name}`"),
                        line,
                        column,
                    )
                    .with_span(expr.span)
                    .with_help("define an expression-bodied function or correct the function name"),
                );
                None
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error("ML_TYPE_MISMATCH", "expected function name", line, column)
                        .with_span(expr.span),
                );
                None
            }
        }
    }

    fn eval_range(
        &mut self,
        start: &Expr,
        end: &Expr,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        match (
            self.eval_expr(start, line, column)?,
            self.eval_expr(end, line, column)?,
        ) {
            (Value::Int(start), Value::Int(end)) => {
                let values = if start <= end {
                    (start..end).map(Value::Int).collect()
                } else {
                    (end + 1..=start).rev().map(Value::Int).collect()
                };
                Some(Value::List(values))
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "expected int range bounds",
                        line,
                        column,
                    )
                    .with_span(span),
                );
                None
            }
        }
    }

    fn eval_list_comprehension(
        &mut self,
        item: &Expr,
        binding: &str,
        source: &Expr,
        condition: Option<&Expr>,
        span: Span,
    ) -> Option<Value> {
        let line = span.line;
        let column = span.column;
        let values = match self.eval_expr(source, line, column)? {
            Value::List(values) => values,
            Value::Tuple(values) if !is_note_tuple(&values) => values,
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "expected collection source",
                        line,
                        column,
                    )
                    .with_span(span),
                );
                return None;
            }
        };
        let mut output = Vec::new();
        for value in values {
            self.push_scope();
            self.set_var(binding, value);
            let keep = match condition {
                Some(condition) => match self.eval_expr(condition, line, column) {
                    Some(Value::Bool(value)) => value,
                    Some(_) => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "ML_TYPE_MISMATCH",
                                "expected comprehension condition to be bool",
                                line,
                                column,
                            )
                            .with_span(condition.span),
                        );
                        self.pop_scope();
                        return None;
                    }
                    None => {
                        self.pop_scope();
                        return None;
                    }
                },
                None => true,
            };
            if keep {
                let Some(value) = self.eval_expr(item, line, column) else {
                    self.pop_scope();
                    return None;
                };
                output.push(value);
            }
            self.pop_scope();
        }
        Some(Value::List(output))
    }

    fn eval_access(
        &mut self,
        target: Value,
        key: &str,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        match target {
            Value::Dict(values) => values.get(key).cloned().or_else(|| {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_RESOLVE_UNKNOWN_NAME",
                        format!("unknown field `{key}`"),
                        line,
                        column,
                    )
                    .with_span(span),
                );
                None
            }),
            Value::Tuple(values) => key
                .parse::<usize>()
                .ok()
                .and_then(|index| values.get(index).cloned())
                .or_else(|| {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "ML_RESOLVE_UNKNOWN_NAME",
                            format!("unknown tuple index `{key}`"),
                            line,
                            column,
                        )
                        .with_span(span),
                    );
                    None
                }),
            _ => {
                self.diagnostics.push(
                    Diagnostic::error("ML_TYPE_MISMATCH", "expected tuple or dict", line, column)
                        .with_span(span),
                );
                None
            }
        }
    }

    fn eval_pipe(
        &mut self,
        value: Value,
        call: &Expr,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        match &call.kind {
            ExprKind::Call { callee, args } => {
                if is_phrase_function_transform(callee) && args.len() == 1 {
                    let function_name = self.expr_function_name(&args[0], line, column)?;
                    return self.eval_phrase_function_transform(
                        callee,
                        value,
                        &function_name,
                        line,
                        column,
                        span,
                    );
                }
                let mut values = Vec::with_capacity(args.len() + 1);
                values.push(value);
                values.extend(
                    args.iter()
                        .map(|arg| self.eval_expr(arg, line, column))
                        .collect::<Option<Vec<_>>>()?,
                );
                self.eval_call(callee, values, line, column, span)
            }
            ExprKind::MethodCall {
                target,
                method,
                args,
            } => {
                let mut values = Vec::with_capacity(args.len() + 2);
                values.push(value);
                values.push(self.eval_expr(target, line, column)?);
                values.extend(
                    args.iter()
                        .map(|arg| self.eval_expr(arg, line, column))
                        .collect::<Option<Vec<_>>>()?,
                );
                self.eval_call(method, values, line, column, span)
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "expected function call after pipe",
                        line,
                        column,
                    )
                    .with_span(span),
                );
                None
            }
        }
    }

    fn eval_user_function_call(
        &mut self,
        callee: &str,
        args: &[Value],
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Option<Value>> {
        let function = self.functions.get(callee)?.clone();
        let body_expr = function.body_expr().cloned()?;
        if function.params.len() != args.len() {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_TYPE_MISMATCH",
                    format!(
                        "function `{}` expects {} arguments, got {}",
                        callee,
                        function.params.len(),
                        args.len()
                    ),
                    line,
                    column,
                )
                .with_span(span),
            );
            return Some(None);
        }
        if self.function_call_stack.iter().any(|name| name == callee) {
            return Some(None);
        }
        self.function_call_stack.push(callee.to_string());
        self.push_scope();
        for (param, value) in function.params.iter().zip(args.iter().cloned()) {
            self.set_var(param, value);
        }
        let value = self.eval_expr(&body_expr, line, column);
        self.pop_scope();
        self.function_call_stack.pop();
        Some(value)
    }

    fn eval_transpose_value(
        &mut self,
        value: Value,
        interval: Interval,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        match value {
            Value::Pitch(pitch) => match pitch.transpose(interval) {
                Ok(pitch) => Some(Value::Pitch(pitch)),
                Err(error) => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "ML_EVAL_UNSUPPORTED_OP",
                            error.to_string(),
                            line,
                            column,
                        )
                        .with_span(span),
                    );
                    None
                }
            },
            Value::List(values) => values
                .into_iter()
                .map(|value| self.eval_transpose_value(value, interval, line, column, span))
                .collect::<Option<Vec<_>>>()
                .map(Value::List),
            Value::Tuple(values) => values
                .into_iter()
                .map(|value| self.eval_transpose_value(value, interval, line, column, span))
                .collect::<Option<Vec<_>>>()
                .map(Value::Tuple),
            Value::Dict(values) => values
                .into_iter()
                .map(|(key, value)| {
                    self.eval_transpose_value(value, interval, line, column, span)
                        .map(|value| (key, value))
                })
                .collect::<Option<HashMap<_, _>>>()
                .map(Value::Dict),
            value => Some(value),
        }
    }

    fn eval_stretch_value(
        &mut self,
        value: Value,
        factor: i32,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        if factor <= 0 {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_TYPE_MISMATCH",
                    "expected positive stretch factor",
                    line,
                    column,
                )
                .with_span(span),
            );
            return None;
        }
        match value {
            Value::Duration(duration) => match duration.numerator().checked_mul(factor as u32) {
                Some(numerator) => match Duration::new(numerator, duration.denominator()) {
                    Ok(duration) => Some(Value::Duration(duration)),
                    Err(error) => {
                        self.diagnostics.push(
                            Diagnostic::error("ML_CORE_DURATION", error.to_string(), line, column)
                                .with_span(span),
                        );
                        None
                    }
                },
                None => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "ML_TYPE_MISMATCH",
                            "duration stretch overflow",
                            line,
                            column,
                        )
                        .with_span(span),
                    );
                    None
                }
            },
            Value::List(values) => values
                .into_iter()
                .map(|value| self.eval_stretch_value(value, factor, line, column, span))
                .collect::<Option<Vec<_>>>()
                .map(Value::List),
            Value::Tuple(values) => values
                .into_iter()
                .map(|value| self.eval_stretch_value(value, factor, line, column, span))
                .collect::<Option<Vec<_>>>()
                .map(Value::Tuple),
            Value::Dict(values) => values
                .into_iter()
                .map(|(key, value)| {
                    self.eval_stretch_value(value, factor, line, column, span)
                        .map(|value| (key, value))
                })
                .collect::<Option<HashMap<_, _>>>()
                .map(Value::Dict),
            value => Some(value),
        }
    }

    fn eval_concat_values(&self, args: Vec<Value>) -> Value {
        let mut values = Vec::new();
        for arg in args {
            match arg {
                Value::List(items) => values.extend(items),
                Value::Tuple(items) if !is_note_tuple(&items) => values.extend(items),
                value => values.push(value),
            }
        }
        Value::List(values)
    }

    fn eval_phrase_function_transform(
        &mut self,
        transform: &str,
        value: Value,
        function_name: &str,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        match transform {
            "map" => self.eval_map_value(value, function_name, line, column, span),
            "filter" => self.eval_filter_value(value, function_name, line, column, span),
            "mapi" => self.eval_mapi_value(value, function_name, line, column, span),
            _ => None,
        }
    }

    fn eval_mapi_value(
        &mut self,
        value: Value,
        function_name: &str,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        let mut index = 0;
        self.eval_mapi_value_with_index(value, function_name, &mut index, line, column, span)
    }

    fn eval_mapi_value_with_index(
        &mut self,
        value: Value,
        function_name: &str,
        index: &mut i32,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        match value {
            Value::List(values) => values
                .into_iter()
                .map(|value| {
                    self.eval_mapi_value_with_index(value, function_name, index, line, column, span)
                })
                .collect::<Option<Vec<_>>>()
                .map(Value::List),
            Value::Tuple(values) if !is_note_tuple(&values) => values
                .into_iter()
                .map(|value| {
                    self.eval_mapi_value_with_index(value, function_name, index, line, column, span)
                })
                .collect::<Option<Vec<_>>>()
                .map(Value::Tuple),
            value => {
                let current_index = *index;
                *index += 1;
                self.eval_user_function_call_or_unknown(
                    function_name,
                    vec![Value::Int(current_index), value],
                    line,
                    column,
                    span,
                )
            }
        }
    }

    fn eval_map_value(
        &mut self,
        value: Value,
        function_name: &str,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        match value {
            Value::List(values) => values
                .into_iter()
                .map(|value| self.eval_map_value(value, function_name, line, column, span))
                .collect::<Option<Vec<_>>>()
                .map(Value::List),
            Value::Tuple(values) if !is_note_tuple(&values) => values
                .into_iter()
                .map(|value| self.eval_map_value(value, function_name, line, column, span))
                .collect::<Option<Vec<_>>>()
                .map(Value::Tuple),
            value => self.eval_user_function_call_or_unknown(
                function_name,
                vec![value],
                line,
                column,
                span,
            ),
        }
    }

    fn eval_filter_value(
        &mut self,
        value: Value,
        function_name: &str,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        match value {
            Value::List(values) => {
                let mut kept = Vec::new();
                for value in values {
                    match value {
                        value if is_transform_collection(&value) => kept.push(
                            self.eval_filter_value(value, function_name, line, column, span)?,
                        ),
                        value => match self.eval_user_function_call_or_unknown(
                            function_name,
                            vec![value.clone()],
                            line,
                            column,
                            span,
                        )? {
                            Value::Bool(true) => kept.push(value),
                            Value::Bool(false) => {}
                            _ => {
                                self.diagnostics.push(
                                    Diagnostic::error(
                                        "ML_TYPE_MISMATCH",
                                        "expected filter predicate to return bool",
                                        line,
                                        column,
                                    )
                                    .with_span(span),
                                );
                                return None;
                            }
                        },
                    }
                }
                Some(Value::List(kept))
            }
            Value::Tuple(values) => {
                let mut kept = Vec::new();
                for value in values {
                    match value {
                        value if is_transform_collection(&value) => kept.push(
                            self.eval_filter_value(value, function_name, line, column, span)?,
                        ),
                        value => match self.eval_user_function_call_or_unknown(
                            function_name,
                            vec![value.clone()],
                            line,
                            column,
                            span,
                        )? {
                            Value::Bool(true) => kept.push(value),
                            Value::Bool(false) => {}
                            _ => {
                                self.diagnostics.push(
                                    Diagnostic::error(
                                        "ML_TYPE_MISMATCH",
                                        "expected filter predicate to return bool",
                                        line,
                                        column,
                                    )
                                    .with_span(span),
                                );
                                return None;
                            }
                        },
                    }
                }
                Some(Value::Tuple(kept))
            }
            value => match self.eval_user_function_call_or_unknown(
                function_name,
                vec![value.clone()],
                line,
                column,
                span,
            )? {
                Value::Bool(true) => Some(value),
                Value::Bool(false) => Some(Value::List(Vec::new())),
                _ => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "ML_TYPE_MISMATCH",
                            "expected filter predicate to return bool",
                            line,
                            column,
                        )
                        .with_span(span),
                    );
                    None
                }
            },
        }
    }

    fn eval_user_function_call_or_unknown(
        &mut self,
        function_name: &str,
        args: Vec<Value>,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        match self.eval_user_function_call(function_name, &args, line, column, span) {
            Some(value) => value,
            None => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_RESOLVE_UNKNOWN_NAME",
                        format!("unknown function `{function_name}`"),
                        line,
                        column,
                    )
                    .with_span(span)
                    .with_help(
                        "define the function before calling it or correct the function name",
                    ),
                );
                None
            }
        }
    }

    fn eval_call(
        &mut self,
        callee: &str,
        args: Vec<Value>,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        if let Some(value) = self.eval_user_function_call(callee, &args, line, column, span) {
            return value;
        }
        match (callee, args.as_slice()) {
            ("cat" | "concat", _) => Some(self.eval_concat_values(args)),
            ("map", [value, Value::String(function_name)]) => {
                self.eval_map_value(value.clone(), function_name, line, column, span)
            }
            ("filter", [value, Value::String(function_name)]) => {
                self.eval_filter_value(value.clone(), function_name, line, column, span)
            }
            ("mapi", [value, Value::String(function_name)]) => {
                self.eval_mapi_value(value.clone(), function_name, line, column, span)
            }
            ("transpose", [value, Value::Interval(interval)]) => {
                self.eval_transpose_value(value.clone(), *interval, line, column, span)
            }
            ("repeat", [value, Value::Int(count)]) => {
                if *count <= 0 {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "ML_TYPE_MISMATCH",
                            "expected positive repeat count",
                            line,
                            column,
                        )
                        .with_span(span),
                    );
                    None
                } else {
                    let mut values = Vec::new();
                    for _ in 0..*count {
                        values.push(value.clone());
                    }
                    Some(Value::List(values))
                }
            }
            ("stretch", [value, Value::Int(factor)]) => {
                self.eval_stretch_value(value.clone(), *factor, line, column, span)
            }
            ("duration", [Value::String(value)]) => match value.parse() {
                Ok(duration) => Some(Value::Duration(duration)),
                Err(error) => {
                    self.diagnostics.push(
                        Diagnostic::error("ML_CORE_DURATION", error.to_string(), line, column)
                            .with_span(span),
                    );
                    None
                }
            },
            ("pitch", [Value::String(value)]) => match value.parse() {
                Ok(pitch) => Some(Value::Pitch(pitch)),
                Err(error) => {
                    self.diagnostics.push(
                        Diagnostic::error("ML_CORE_PITCH", error.to_string(), line, column)
                            .with_span(span),
                    );
                    None
                }
            },
            ("first", [Value::List(values)] | [Value::Tuple(values)]) => {
                values.first().cloned().or_else(|| {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "ML_TYPE_MISMATCH",
                            "expected non-empty collection",
                            line,
                            column,
                        )
                        .with_span(span),
                    );
                    None
                })
            }
            ("len", [Value::List(values)]) => Some(Value::Int(values.len() as i32)),
            ("len", [Value::Tuple(values)]) => Some(Value::Int(values.len() as i32)),
            ("at", [Value::List(values), Value::Int(index)]) => {
                self.eval_indexed_value(values, *index, line, column, span)
            }
            ("at", [Value::Tuple(values), Value::Int(index)]) => {
                self.eval_indexed_value(values, *index, line, column, span)
            }
            ("with" | "merge", [Value::Dict(values), Value::Dict(patch)]) => {
                let mut values = values.clone();
                values.extend(patch.clone());
                Some(Value::Dict(values))
            }
            ("with" | "merge", [_, _]) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        format!("builtin `{callee}` expects dict arguments"),
                        line,
                        column,
                    )
                    .with_span(span),
                );
                None
            }
            (name, _) if builtin_signature(name).is_some() => {
                let signature = builtin_signature(name).unwrap();
                let message = if args.len() == signature.arg_count {
                    format!("builtin `{name}` {}", signature.type_message)
                } else {
                    format!(
                        "builtin `{name}` expects {} arguments, got {}",
                        signature.arg_count,
                        args.len()
                    )
                };
                self.diagnostics.push(
                    Diagnostic::error("ML_TYPE_MISMATCH", message, line, column).with_span(span),
                );
                None
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_EVAL_UNSUPPORTED_OP",
                        format!("unsupported call `{callee}`"),
                        line,
                        column,
                    )
                    .with_span(span),
                );
                None
            }
        }
    }

    fn eval_indexed_value(
        &mut self,
        values: &[Value],
        index: i32,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        if index < 0 {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_TYPE_MISMATCH",
                    "collection index out of range",
                    line,
                    column,
                )
                .with_span(span),
            );
            return None;
        }
        values.get(index as usize).cloned().or_else(|| {
            self.diagnostics.push(
                Diagnostic::error(
                    "ML_TYPE_MISMATCH",
                    "collection index out of range",
                    line,
                    column,
                )
                .with_span(span),
            );
            None
        })
    }

    fn eval_unary(
        &mut self,
        op: UnaryOp,
        value: Value,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Value> {
        match (op, value) {
            (UnaryOp::Not, Value::Bool(value)) => Some(Value::Bool(!value)),
            _ => {
                self.diagnostics.push(
                    Diagnostic::error("ML_TYPE_MISMATCH", "expected bool operand", line, column)
                        .with_span(span),
                );
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
        span: Span,
    ) -> Option<Value> {
        match (op, left, right) {
            (BinaryOp::Add, Value::Int(left), Value::Int(right)) => Some(Value::Int(left + right)),
            (BinaryOp::Sub, Value::Int(left), Value::Int(right)) => Some(Value::Int(left - right)),
            (BinaryOp::Mul, Value::Int(left), Value::Int(right)) => Some(Value::Int(left * right)),
            (BinaryOp::Div, Value::Int(_), Value::Int(0)) => {
                self.diagnostics.push(
                    Diagnostic::error("ML_EVAL_UNSUPPORTED_OP", "division by zero", line, column)
                        .with_span(span),
                );
                None
            }
            (BinaryOp::Div, Value::Int(left), Value::Int(right)) => Some(Value::Int(left / right)),
            (BinaryOp::Add, Value::Pitch(pitch), Value::Interval(interval)) => {
                match pitch + interval {
                    Ok(pitch) => Some(Value::Pitch(pitch)),
                    Err(error) => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "ML_EVAL_UNSUPPORTED_OP",
                                error.to_string(),
                                line,
                                column,
                            )
                            .with_span(span),
                        );
                        None
                    }
                }
            }
            (BinaryOp::Sub, Value::Pitch(pitch), Value::Interval(interval)) => {
                match pitch - interval {
                    Ok(pitch) => Some(Value::Pitch(pitch)),
                    Err(error) => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                "ML_EVAL_UNSUPPORTED_OP",
                                error.to_string(),
                                line,
                                column,
                            )
                            .with_span(span),
                        );
                        None
                    }
                }
            }
            (BinaryOp::Eq, Value::Int(left), Value::Int(right)) => Some(Value::Bool(left == right)),
            (BinaryOp::Eq, Value::Bool(left), Value::Bool(right)) => {
                Some(Value::Bool(left == right))
            }
            (BinaryOp::Eq, Value::String(left), Value::String(right)) => {
                Some(Value::Bool(left == right))
            }
            (BinaryOp::Eq, Value::Pitch(left), Value::Pitch(right)) => {
                Some(Value::Bool(left == right))
            }
            (BinaryOp::Eq, Value::Duration(left), Value::Duration(right)) => {
                Some(Value::Bool(left == right))
            }
            (BinaryOp::Eq, Value::Interval(left), Value::Interval(right)) => {
                Some(Value::Bool(left == right))
            }
            (BinaryOp::NotEq, Value::Int(left), Value::Int(right)) => {
                Some(Value::Bool(left != right))
            }
            (BinaryOp::NotEq, Value::Bool(left), Value::Bool(right)) => {
                Some(Value::Bool(left != right))
            }
            (BinaryOp::NotEq, Value::String(left), Value::String(right)) => {
                Some(Value::Bool(left != right))
            }
            (BinaryOp::NotEq, Value::Pitch(left), Value::Pitch(right)) => {
                Some(Value::Bool(left != right))
            }
            (BinaryOp::NotEq, Value::Duration(left), Value::Duration(right)) => {
                Some(Value::Bool(left != right))
            }
            (BinaryOp::NotEq, Value::Interval(left), Value::Interval(right)) => {
                Some(Value::Bool(left != right))
            }
            (BinaryOp::Lt, Value::Int(left), Value::Int(right)) => Some(Value::Bool(left < right)),
            (BinaryOp::LtEq, Value::Int(left), Value::Int(right)) => {
                Some(Value::Bool(left <= right))
            }
            (BinaryOp::Gt, Value::Int(left), Value::Int(right)) => Some(Value::Bool(left > right)),
            (BinaryOp::GtEq, Value::Int(left), Value::Int(right)) => {
                Some(Value::Bool(left >= right))
            }
            (BinaryOp::And, Value::Bool(left), Value::Bool(right)) => {
                Some(Value::Bool(left && right))
            }
            (BinaryOp::Or, Value::Bool(left), Value::Bool(right)) => {
                Some(Value::Bool(left || right))
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "unsupported expression operand types",
                        line,
                        column,
                    )
                    .with_span(span),
                );
                None
            }
        }
    }

    fn eval_pitch(&mut self, expr: &Expr, line: usize, column: usize) -> Option<Pitch> {
        match self.eval_expr(expr, line, column)? {
            Value::Pitch(pitch) => self.apply_pitch_transpose(pitch, line, column, expr.span),
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "expected pitch expression",
                        line,
                        column,
                    )
                    .with_span(expr.span)
                    .with_help(
                        "define the referenced name before using it or correct the identifier",
                    ),
                );
                None
            }
        }
    }

    fn apply_pitch_transpose(
        &mut self,
        pitch: Pitch,
        line: usize,
        column: usize,
        span: Span,
    ) -> Option<Pitch> {
        if self.pitch_transpose_semitones == 0 {
            return Some(pitch);
        }
        match pitch.transpose(Interval::new(self.pitch_transpose_semitones)) {
            Ok(pitch) => Some(pitch),
            Err(error) => {
                self.diagnostics.push(
                    Diagnostic::error("ML_EVAL_UNSUPPORTED_OP", error.to_string(), line, column)
                        .with_span(span),
                );
                None
            }
        }
    }

    fn eval_interval(
        &mut self,
        expr: &Expr,
        line: usize,
        column: usize,
        message: &'static str,
    ) -> Option<Interval> {
        match self.eval_expr(expr, line, column)? {
            Value::Interval(interval) => Some(interval),
            _ => {
                self.diagnostics.push(
                    Diagnostic::error("ML_TYPE_MISMATCH", message, line, column)
                        .with_span(expr.span),
                );
                None
            }
        }
    }

    fn eval_duration(&mut self, expr: &Expr, line: usize, column: usize) -> Option<Duration> {
        match self.eval_expr(expr, line, column)? {
            Value::Duration(duration) => Some(duration),
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "ML_TYPE_MISMATCH",
                        "expected duration expression",
                        line,
                        column,
                    )
                    .with_span(expr.span)
                    .with_help(
                        "define the referenced name before using it or correct the identifier",
                    ),
                );
                None
            }
        }
    }

    fn eval_bool(&mut self, expr: &Expr, line: usize, column: usize) -> Option<bool> {
        match self.eval_expr(expr, line, column)? {
            Value::Bool(value) => Some(value),
            _ => {
                self.diagnostics.push(
                    Diagnostic::error("ML_TYPE_MISMATCH", "expected bool expression", line, column)
                        .with_span(expr.span),
                );
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
            let _ = (line, column, span);
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
            .with_span(style.span)
            .with_style(&style.name)
            .with_help("break the extends cycle by removing or changing one parent style")],
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
                .with_span(style.span)
                .with_style(parent_name)
                .with_help(
                    "declare the parent style before extending it or correct the parent style name",
                )],
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
            "melodic_concept" => {
                context.melodic_concepts = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_idiom_entries(style, entry, &["blues_inflection"], &mut diagnostics);
            }
            "phrase_concept" => {
                context.phrase_concepts = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_idiom_entries(
                    style,
                    entry,
                    &["periodic_phrase", "motivic_development"],
                    &mut diagnostics,
                );
            }
            "ensemble_concept" => {
                context.ensemble_concepts = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_idiom_entries(style, entry, &["call_response"], &mut diagnostics);
            }
            "bass_concept" => {
                context.bass_concepts = entry
                    .value
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect();
                validate_idiom_entries(style, entry, &["walking_or_riff_bass"], &mut diagnostics);
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
            "voice_spacing" => {
                context.max_voice_spacing = entry.value.trim().parse::<Interval>().ok();
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
                            entry.line,
                            entry.column,
                        )
                        .with_span(entry.span)
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
                    entry.line,
                    entry.column,
                )
                .with_span(entry.span)
                .with_rule(entry.key.clone())
                .with_style(style.name.clone()),
            );
        }
    }
}

fn validate_idiom_entries(
    style: &StyleDecl,
    entry: &musiclang_parser::StyleEntry,
    known_entries: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry_id in entry.value.split_whitespace() {
        if !known_entries.contains(&entry_id) {
            diagnostics.push(
                Diagnostic::error(
                    "ML_STYLE_UNKNOWN_IDIOM_ENTRY",
                    format!("unknown idiom entry `{entry_id}` for `{}`", entry.key),
                    entry.line,
                    entry.column,
                )
                .with_span(entry.span)
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
                    entry.line,
                    entry.column,
                )
                .with_span(entry.span)
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
                    entry.line,
                    entry.column,
                )
                .with_span(entry.span)
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

fn track_has_blues_inflection(track: &TrackIr) -> bool {
    track.events.iter().any(|event| {
        matches!(
            event.pitch.class(),
            PitchClass::Ds | PitchClass::Fs | PitchClass::As
        )
    })
}

fn explicit_blues_inflection(events: &[MelodicEventIr]) -> Option<bool> {
    (!events.is_empty()).then(|| {
        events
            .iter()
            .any(|event| matches!(event.degree, Some(3 | 5 | 7)) && event.accidental < 0)
    })
}

fn tracks_have_call_response(tracks: &[TrackIr]) -> bool {
    let pitched_tracks = tracks
        .iter()
        .filter(|track| track_is_call_response_voice(track))
        .collect::<Vec<_>>();
    for caller in &pitched_tracks {
        for responder in &pitched_tracks {
            if caller.name == responder.name {
                continue;
            }
            if tracks_form_call_response(caller, responder) {
                return true;
            }
        }
    }
    false
}

fn track_is_call_response_voice(track: &TrackIr) -> bool {
    if track.channel == 9 || track.events.is_empty() {
        return false;
    }
    let name = track.name.to_ascii_lowercase();
    if name.contains("piano") || name.contains("bass") || name.contains("comp") {
        return false;
    }
    if let Some(program) = track.program {
        if program <= 7 || (32..=39).contains(&program) {
            return false;
        }
    }
    true
}

fn tracks_form_call_response(caller: &TrackIr, responder: &TrackIr) -> bool {
    caller.events.iter().any(|call| {
        let call_end = call.start_tick + call.duration_ticks;
        responder.events.iter().any(|response| {
            response.start_tick >= call_end
                && response.start_tick <= call_end + DEFAULT_TICKS_PER_QUARTER * 2
                && response.duration_ticks <= DEFAULT_TICKS_PER_QUARTER
                && response.pitch.class() != call.pitch.class()
        })
    })
}

fn track_has_walking_or_riff_bass(track: &TrackIr) -> bool {
    if track.channel == 9 {
        return false;
    }
    let name = track.name.to_ascii_lowercase();
    if !name.contains("bass") {
        return false;
    }
    track_has_quarter_bass_motion(track) || track_has_repeated_pitch_riff(track)
}

fn track_has_quarter_bass_motion(track: &TrackIr) -> bool {
    track.events.windows(4).any(|events| {
        events
            .iter()
            .all(|event| event.duration_ticks == DEFAULT_TICKS_PER_QUARTER)
            && events.windows(2).all(|pair| {
                let Ok(first) = pair[0].pitch.midi_number().map(i16::from) else {
                    return false;
                };
                let Ok(second) = pair[1].pitch.midi_number().map(i16::from) else {
                    return false;
                };
                matches!((second - first).abs(), 1 | 2 | 3 | 4 | 5 | 7)
            })
    })
}

fn track_has_repeated_pitch_riff(track: &TrackIr) -> bool {
    let classes = track
        .events
        .iter()
        .map(|event| event.pitch.class())
        .collect::<Vec<_>>();
    classes.len() >= 6
        && classes.windows(2).any(|cell| {
            classes
                .windows(2)
                .filter(|candidate| *candidate == cell)
                .count()
                >= 2
        })
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

fn explicit_harmonic_functions(events: &[HarmonicEventIr]) -> Option<Vec<String>> {
    let functions = events
        .iter()
        .filter_map(|event| event.function.clone())
        .collect::<Vec<_>>();
    (!functions.is_empty()).then_some(functions)
}

fn final_harmonic_symbols(events: &[HarmonicEventIr]) -> Option<(String, String)> {
    let symbols = events
        .iter()
        .filter(|event| !event.normalized_symbol.is_empty())
        .map(|event| event.normalized_symbol.clone())
        .collect::<Vec<_>>();
    let [.., penultimate, final_symbol] = symbols.as_slice() else {
        return None;
    };
    Some((penultimate.clone(), final_symbol.clone()))
}

fn cadence_matches_symbols(cadence: &str, penultimate: &str, final_symbol: &str) -> bool {
    match cadence {
        "authentic" => is_dominant_symbol(penultimate) && is_tonic_symbol(final_symbol),
        "plagal" => penultimate == "IV" && is_tonic_symbol(final_symbol),
        "deceptive" => is_dominant_symbol(penultimate) && final_symbol == "vi",
        "half" => is_dominant_symbol(final_symbol),
        _ => true,
    }
}

struct RomanAnalysis {
    normalized_symbol: String,
    degree: Option<u8>,
    applied_to: Option<String>,
    function: Option<&'static str>,
    cadence_role: Option<&'static str>,
}

fn analyze_roman_symbol(symbol: &str) -> RomanAnalysis {
    let normalized_symbol = normalize_roman_symbol(symbol);
    let (base, applied_to) = normalized_symbol
        .split_once('/')
        .map_or((normalized_symbol.as_str(), None), |(base, target)| {
            (base, Some(target.to_string()))
        });
    let degree = roman_degree(base);
    let function = roman_harmonic_function(&normalized_symbol);
    let cadence_role = if is_tonic_symbol(base) {
        Some("arrival")
    } else if is_dominant_symbol(base) {
        Some("dominant_preparation")
    } else if matches!(base, "IV" | "iv" | "ii" | "II") {
        Some("predominant_preparation")
    } else {
        None
    };
    RomanAnalysis {
        normalized_symbol,
        degree,
        applied_to,
        function,
        cadence_role,
    }
}

fn roman_harmonic_function(symbol: &str) -> Option<&'static str> {
    let normalized = normalize_roman_symbol(symbol);
    let base = normalized
        .split_once('/')
        .map_or(normalized.as_str(), |(base, _)| base);
    if is_tonic_symbol(base) {
        Some("tonic")
    } else if matches!(base, "ii" | "II" | "IV" | "iv") {
        Some("predominant")
    } else if normalized.contains('/') {
        Some("secondary_dominant")
    } else if is_dominant_symbol(base) {
        Some("dominant")
    } else if base == "vi" || base == "VI" {
        Some("submediant")
    } else {
        None
    }
}

fn roman_degree(symbol: &str) -> Option<u8> {
    let base = symbol.trim_end_matches("dim");
    match base.to_ascii_uppercase().as_str() {
        "I" => Some(1),
        "II" => Some(2),
        "III" => Some(3),
        "IV" => Some(4),
        "V" => Some(5),
        "VI" => Some(6),
        "VII" => Some(7),
        _ => None,
    }
}

fn is_tonic_symbol(symbol: &str) -> bool {
    matches!(symbol, "I" | "i")
}

fn is_dominant_symbol(symbol: &str) -> bool {
    matches!(symbol, "V" | "v" | "viidim" | "vii") || symbol.starts_with("V/")
}

fn normalize_roman_symbol(symbol: &str) -> String {
    let (base, target) = symbol
        .split_once('/')
        .map_or((symbol, None), |(base, target)| (base, Some(target)));
    let mut body = base.trim();
    while let Some(stripped) = body.strip_prefix('b') {
        body = stripped;
    }
    while let Some(stripped) = body.strip_prefix('#') {
        body = stripped;
    }
    for suffix in ["64", "65", "43", "42", "6", "7"] {
        if let Some(stripped) = body.strip_suffix(suffix) {
            body = stripped;
            break;
        }
    }
    let mut normalized = body.trim_end_matches('°').trim_end_matches('+').to_string();
    if body.ends_with('°') || body.ends_with("dim") {
        normalized = normalized.trim_end_matches("dim").to_string();
        normalized.push_str("dim");
    }
    if let Some(target) = target {
        normalized.push('/');
        normalized.push_str(target.trim());
    }
    normalized
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
    let harmonic_tracks = tracks
        .iter()
        .filter(|track| track.channel != 9)
        .collect::<Vec<_>>();
    let mut ticks = harmonic_tracks
        .iter()
        .flat_map(|track| track.events.iter().map(|event| event.start_tick))
        .collect::<Vec<_>>();
    ticks.sort_unstable();
    ticks.dedup();
    ticks
        .into_iter()
        .map(|tick| sonority_at(&harmonic_tracks, tick))
        .collect()
}

fn final_sonorities(tracks: &[TrackIr]) -> Option<(Vec<PitchClass>, Vec<PitchClass>)> {
    let functional = sonority_sequence(tracks)
        .into_iter()
        .filter(|classes| harmonic_function(classes).is_some())
        .collect::<Vec<_>>();
    let [.., penultimate, final_sonority] = functional.as_slice() else {
        return None;
    };
    Some((penultimate.clone(), final_sonority.clone()))
}

fn sonority_at(tracks: &[&TrackIr], tick: u32) -> Vec<PitchClass> {
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

fn expand_chord_quality(root: Pitch, quality: &str) -> Option<Vec<Pitch>> {
    let catalog = musiclang_core::theory_catalog();
    let entry = catalog
        .entries(TheoryDomain::ChordQualities)
        .iter()
        .find(|entry| entry.id == quality)?;
    entry
        .pattern
        .iter()
        .map(|step| {
            step.parse::<i16>()
                .ok()
                .and_then(|semitones| root.transpose(Interval::new(semitones)).ok())
        })
        .collect()
}

struct ParsedRoman {
    degree: usize,
    accidental: i16,
    quality: &'static str,
    inversion: usize,
}

fn cadence_symbols(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        "authentic" | "perfect_authentic" | "pac" => Some(&["V7", "I"]),
        "imperfect_authentic" | "iac" => Some(&["V", "I6"]),
        "plagal" => Some(&["IV", "I"]),
        "half" => Some(&["I", "V"]),
        "deceptive" => Some(&["V7", "vi"]),
        _ => None,
    }
}

fn parse_scale_degree(value: &str) -> Option<(usize, i16)> {
    let mut body = value.trim();
    let mut accidental = 0;
    while let Some(stripped) = body.strip_prefix('b') {
        accidental -= 1;
        body = stripped;
    }
    while let Some(stripped) = body.strip_prefix('#') {
        accidental += 1;
        body = stripped;
    }
    let degree = body.parse::<usize>().ok()?;
    (1..=7)
        .contains(&degree)
        .then_some((degree - 1, accidental))
}

fn key_scale_pattern(key: KeySignature) -> [i16; 7] {
    if key.is_minor {
        [0, 2, 3, 5, 7, 8, 10]
    } else {
        [0, 2, 4, 5, 7, 9, 11]
    }
}

fn scale_mode_pattern(mode: &str) -> Option<Vec<i16>> {
    let mode = match mode.trim() {
        "major" => "major",
        "minor" | "min" | "natural_minor" => "natural_minor",
        other => other,
    };
    let domain = if matches!(mode, "major" | "natural_minor") {
        TheoryDomain::Scales
    } else {
        TheoryDomain::Modes
    };
    musiclang_core::theory_catalog()
        .entries(domain)
        .iter()
        .find(|entry| entry.id == mode)
        .map(|entry| {
            entry
                .pattern
                .iter()
                .filter_map(pattern_step_semitones)
                .collect()
        })
}

fn roman_chord_pitches(symbol: &str, key: KeySignature) -> Option<Vec<Pitch>> {
    let (symbol, applied_target) = symbol
        .split_once('/')
        .map_or((symbol, None), |(symbol, target)| (symbol, Some(target)));
    let tonic = if let Some(target) = applied_target {
        let target = parse_roman_symbol(target)?;
        roman_root_semitone(&target, key_tonic_semitone(key), key.is_minor)
    } else {
        key_tonic_semitone(key)
    };
    let parsed = parse_roman_symbol(symbol)?;
    let root_class = PitchClass::from_semitone(roman_root_semitone(&parsed, tonic, false));
    let root_octave = if parsed.degree >= 4 { 3 } else { 4 };
    let root = Pitch::new(root_class, root_octave).ok()?;
    let mut pitches = expand_chord_quality(root, parsed.quality)?;
    invert_chord(&mut pitches, parsed.inversion)?;
    Some(pitches)
}

fn roman_root_semitone(parsed: &ParsedRoman, tonic: i16, is_minor: bool) -> i16 {
    let scale = if is_minor {
        [0, 2, 3, 5, 7, 8, 10]
    } else {
        [0, 2, 4, 5, 7, 9, 11]
    };
    tonic + scale[parsed.degree] + parsed.accidental
}

fn parse_roman_symbol(symbol: &str) -> Option<ParsedRoman> {
    let mut body = symbol.trim();
    let mut accidental = 0;
    while let Some(stripped) = body.strip_prefix('b') {
        accidental -= 1;
        body = stripped;
    }
    while let Some(stripped) = body.strip_prefix('#') {
        accidental += 1;
        body = stripped;
    }

    let figures = ["64", "65", "43", "42", "6", "7"];
    let mut figure = "";
    for candidate in figures {
        if let Some(stripped) = body.strip_suffix(candidate) {
            figure = candidate;
            body = stripped;
            break;
        }
    }

    let (body, diminished_suffix) = body
        .strip_suffix("dim")
        .map_or((body, false), |body| (body, true));
    let normalized = body.trim_end_matches('°').trim_end_matches('+');
    let upper = normalized.to_ascii_uppercase();
    let degree = match upper.as_str() {
        "I" => 0,
        "II" => 1,
        "III" => 2,
        "IV" => 3,
        "V" => 4,
        "VI" => 5,
        "VII" => 6,
        _ => return None,
    };
    let quality = if diminished_suffix || body.ends_with('°') {
        "diminished"
    } else if body.ends_with('+') {
        "augmented"
    } else if matches!(figure, "7" | "65" | "43" | "42") && upper == "V" {
        "dominant7"
    } else if normalized.chars().next()?.is_ascii_lowercase() {
        if matches!(figure, "7" | "65" | "43" | "42") {
            "minor7"
        } else {
            "minor"
        }
    } else if matches!(figure, "7" | "65" | "43" | "42") {
        "major7"
    } else {
        "major"
    };
    let inversion = match figure {
        "6" | "65" => 1,
        "64" | "43" => 2,
        "42" => 3,
        _ => 0,
    };
    Some(ParsedRoman {
        degree,
        accidental,
        quality,
        inversion,
    })
}

fn invert_chord(pitches: &mut [Pitch], inversion: usize) -> Option<()> {
    if inversion > pitches.len() {
        return None;
    }
    for index in 0..inversion {
        let pitch = pitches.get_mut(index)?;
        *pitch = pitch.transpose(Interval::new(12)).ok()?;
    }
    pitches.sort_by_key(|pitch| pitch.midi_number().ok());
    Some(())
}

fn key_tonic_semitone(key: KeySignature) -> i16 {
    match (key.fifths, key.is_minor) {
        (-7, false) => 11,
        (-6, false) => 6,
        (-5, false) => 1,
        (-4, false) => 8,
        (-3, false) => 3,
        (-2, false) => 10,
        (-1, false) => 5,
        (0, false) => 0,
        (1, false) => 7,
        (2, false) => 2,
        (3, false) => 9,
        (4, false) => 4,
        (5, false) => 11,
        (6, false) => 6,
        (7, false) => 1,
        (-7, true) => 8,
        (-6, true) => 3,
        (-5, true) => 10,
        (-4, true) => 5,
        (-3, true) => 0,
        (-2, true) => 7,
        (-1, true) => 2,
        (0, true) => 9,
        (1, true) => 4,
        (2, true) => 11,
        (3, true) => 6,
        (4, true) => 1,
        (5, true) => 8,
        (6, true) => 3,
        (7, true) => 10,
        _ => 0,
    }
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

fn drum_midi_number(name: &str) -> Option<u8> {
    match name {
        "kick" | "bass_drum" => Some(36),
        "snare" => Some(38),
        "rimshot" => Some(37),
        "clap" => Some(39),
        "closed_hat" | "hihat" => Some(42),
        "pedal_hat" => Some(44),
        "open_hat" => Some(46),
        "low_tom" => Some(45),
        "mid_tom" => Some(47),
        "high_tom" => Some(50),
        "ride" => Some(51),
        "crash" => Some(49),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use musiclang_core::SourceMap;

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
    fn rest_advances_track_cursor_without_events() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    note C4, 1/4
    rest 1/2
    note E4, 1/4
  }
}
"#,
        )
        .unwrap();

        let track = &ir.tracks[0];
        assert_eq!(track.events.len(), 2);
        assert_eq!(track.events[0].start_tick, 0);
        assert_eq!(track.events[1].start_tick, 1440);
    }

    #[test]
    fn expands_pedal_tone_to_repeated_events() {
        let ir = compile_source(
            r#"
score demo {
  voice bass {
    pedal C3, 4, 1/4
  }
}
"#,
        )
        .unwrap();

        let track = &ir.tracks[0];
        assert_eq!(track.events.len(), 4);
        assert!(track
            .events
            .iter()
            .all(|event| event.pitch.to_string() == "C3"));
        assert_eq!(track.events[0].start_tick, 0);
        assert_eq!(track.events[1].start_tick, 480);
        assert_eq!(track.events[2].start_tick, 960);
        assert_eq!(track.events[3].start_tick, 1440);
    }

    #[test]
    fn rejects_non_positive_pedal_count() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice bass {
    pedal C3, 0, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_THEORY_PEDAL"));
    }

    #[test]
    fn compiles_scale_degrees_against_active_key() {
        let ir = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    degree 1 4, 1/8
    degree 3 4, 1/8
    degree b3 4, 1/8
    modulate G major
    degree 1 4, 1/8
    sequence 2 by M2 {
      degree 2 4, 1/8
    }
  }
}
"#,
        )
        .unwrap();

        let pitches = ir.tracks[0]
            .events
            .iter()
            .map(|event| event.pitch.to_string())
            .collect::<Vec<_>>();
        assert_eq!(pitches, vec!["C4", "E4", "D#4", "G4", "A4", "B4"]);
    }

    #[test]
    fn compiles_scale_run() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    scale C major 4, 1/8
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events.len(), 8);
        assert_eq!(events[0].pitch.class(), PitchClass::C);
        assert_eq!(events[1].pitch.class(), PitchClass::D);
        assert_eq!(events[2].pitch.class(), PitchClass::E);
        assert_eq!(events[7].pitch.class(), PitchClass::C);
        assert_eq!(events[0].start_tick, 0);
        assert_eq!(events[7].start_tick, 1680);
        assert_eq!(events[0].duration_ticks, 240);
    }

    #[test]
    fn compiles_modal_scale_run() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    scale D dorian 4, 1/8
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events.len(), 8);
        assert_eq!(events[0].pitch.class(), PitchClass::D);
        assert_eq!(events[1].pitch.class(), PitchClass::E);
        assert_eq!(events[2].pitch.class(), PitchClass::F);
        assert_eq!(events[6].pitch.class(), PitchClass::C);
    }

    #[test]
    fn transpose_block_transposes_scale_run() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    transpose M2 {
      scale C major 4, 1/8
    }
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events[0].pitch.class(), PitchClass::D);
        assert_eq!(events[1].pitch.class(), PitchClass::E);
        assert_eq!(events[2].pitch.class(), PitchClass::Fs);
    }

    #[test]
    fn rejects_unknown_scale_mode() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice lead {
    scale C imaginary 4, 1/8
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_THEORY_SCALE");
    }

    #[test]
    fn rejects_scale_degree_without_key() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice lead {
    degree 1 4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_THEORY_DEGREE"));
    }

    #[test]
    fn rejects_invalid_scale_degree() {
        let diagnostics = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    degree 8 4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_THEORY_DEGREE"));
    }

    #[test]
    fn expands_ostinato_block_to_repeated_events() {
        let ir = compile_source(
            r#"
score demo {
  voice bass {
    ostinato 3 {
      note C3, 1/8
      note G3, 1/8
    }
  }
}
"#,
        )
        .unwrap();

        let track = &ir.tracks[0];
        let pitches = track
            .events
            .iter()
            .map(|event| event.pitch.to_string())
            .collect::<Vec<_>>();
        assert_eq!(pitches, vec!["C3", "G3", "C3", "G3", "C3", "G3"]);
        assert_eq!(track.events[0].start_tick, 0);
        assert_eq!(track.events[1].start_tick, 240);
        assert_eq!(track.events[5].start_tick, 1200);
    }

    #[test]
    fn rejects_non_positive_ostinato_count() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice bass {
    ostinato 0 {
      note C3, 1/8
    }
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_THEORY_OSTINATO"));
    }

    #[test]
    fn transposes_nested_block_events() {
        let ir = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    transpose M2 {
      note C4, 1/8
      chord [E4, G4], 1/8
      degree 1 4, 1/8
    }
  }
}
"#,
        )
        .unwrap();

        let pitches = ir.tracks[0]
            .events
            .iter()
            .map(|event| event.pitch.to_string())
            .collect::<Vec<_>>();
        assert_eq!(pitches, vec!["D4", "F#4", "A4", "D4"]);
    }

    #[test]
    fn expands_sequence_block_with_transposed_repetitions() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    sequence 3 by M2 {
      note C4, 1/8
      chord [E4, G4], 1/8
    }
  }
}
"#,
        )
        .unwrap();

        let track = &ir.tracks[0];
        let pitches = track
            .events
            .iter()
            .map(|event| event.pitch.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            pitches,
            vec!["C4", "E4", "G4", "D4", "F#4", "A4", "E4", "G#4", "B4"]
        );
        assert_eq!(track.events[0].start_tick, 0);
        assert_eq!(track.events[3].start_tick, 480);
        assert_eq!(track.events[6].start_tick, 960);
    }

    #[test]
    fn rejects_non_positive_sequence_count() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice lead {
    sequence 0 by M2 {
      note C4, 1/8
    }
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_THEORY_SEQUENCE"));
    }

    #[test]
    fn tuplet_scales_enclosed_durations_into_target_space() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    tuplet 3 in 1/4 {
      note C4, 1/8
      rest 1/8
      chord [E4, G4], 1/8
    }
    note C5, 1/4
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].duration_ticks, 160);
        assert_eq!(events[0].start_tick, 0);
        assert_eq!(events[1].start_tick, 320);
        assert_eq!(events[2].start_tick, 320);
        assert_eq!(events[3].start_tick, 480);
        assert_eq!(events[3].duration_ticks, 480);
    }

    #[test]
    fn tuplet_rejects_non_positive_count() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice lead {
    tuplet 0 in 1/4 {
      note C4, 1/8
    }
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_THEORY_TUPLET");
    }

    #[test]
    fn expands_named_chord_quality_to_ir_events() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    chord D3 minor7, 1/2
  }
}
"#,
        )
        .unwrap();

        let pitches = ir.tracks[0]
            .events
            .iter()
            .map(|event| event.pitch.to_string())
            .collect::<Vec<_>>();
        assert_eq!(pitches, vec!["D3", "F3", "A3", "C4"]);
        assert!(ir.tracks[0]
            .events
            .iter()
            .all(|event| event.start_tick == 0 && event.duration_ticks == 960));
    }

    #[test]
    fn named_chord_inversion_reorders_voicing() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    chord C4 major inv 1, 1/2
  }
}
"#,
        )
        .unwrap();

        let pitches = ir.tracks[0]
            .events
            .iter()
            .map(|event| event.pitch.to_string())
            .collect::<Vec<_>>();
        assert_eq!(pitches, vec!["E4", "G4", "C5"]);
    }

    #[test]
    fn rejects_unknown_named_chord_inversion() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice lead {
    chord C4 major inv 4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_THEORY_CHORD_INVERSION"));
    }

    #[test]
    fn rejects_unknown_named_chord_quality() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice lead {
    chord C4 quartal9, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_THEORY_CHORD_QUALITY"));
    }

    #[test]
    fn expands_roman_numeral_chords_against_score_key() {
        let ir = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    roman I, 1/4
    roman I6, 1/4
    roman V65, 1/2
    roman bVII, 1/4
    roman V/V, 1/4
    roman viidim/V, 1/4
  }
}
"#,
        )
        .unwrap();

        let pitches = ir.tracks[0]
            .events
            .iter()
            .map(|event| event.pitch.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            pitches,
            vec![
                "C4", "E4", "G4", "E4", "G4", "C5", "B3", "D4", "F4", "G4", "A#3", "D4", "F4",
                "D3", "F#3", "A3", "F#3", "A3", "C4"
            ]
        );
    }

    #[test]
    fn compiles_harmonic_progression_against_score_key() {
        let ir = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    progression I vi ii V7 I, 1/4
  }
}
"#,
        )
        .unwrap();

        let pitches = ir.tracks[0]
            .events
            .iter()
            .map(|event| event.pitch.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            pitches,
            vec![
                "C4", "E4", "G4", "A3", "C4", "E4", "D4", "F4", "A4", "G3", "B3", "D4", "F4", "C4",
                "E4", "G4"
            ]
        );
        assert_eq!(ir.tracks[0].events[0].start_tick, 0);
        assert_eq!(ir.tracks[0].events[3].start_tick, 480);
        assert_eq!(ir.tracks[0].events[6].start_tick, 960);
        assert_eq!(ir.tracks[0].events[9].start_tick, 1440);
        assert_eq!(ir.tracks[0].events[13].start_tick, 1920);
    }

    #[test]
    fn compiles_named_cadence_against_score_key() {
        let ir = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    cadence authentic, 1/2
    cadence deceptive, 1/4
  }
}
"#,
        )
        .unwrap();

        let pitches = ir.tracks[0]
            .events
            .iter()
            .map(|event| event.pitch.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            pitches,
            vec![
                "G3", "B3", "D4", "F4", "C4", "E4", "G4", "G3", "B3", "D4", "F4", "A3", "C4", "E4"
            ]
        );
        assert_eq!(ir.tracks[0].events[0].start_tick, 0);
        assert_eq!(ir.tracks[0].events[4].start_tick, 960);
        assert_eq!(ir.tracks[0].events[7].start_tick, 1920);
        assert_eq!(ir.tracks[0].events[11].start_tick, 2400);
    }

    #[test]
    fn rejects_unknown_cadence_kind() {
        let diagnostics = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    cadence backdoor, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_THEORY_CADENCE"));
    }

    #[test]
    fn modulation_changes_active_roman_key() {
        let ir = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    roman I, 1/4
    modulate G major
    roman I, 1/4
    cadence authentic, 1/4
  }
}
"#,
        )
        .unwrap();

        let pitches = ir.tracks[0]
            .events
            .iter()
            .map(|event| event.pitch.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            pitches,
            vec!["C4", "E4", "G4", "G4", "B4", "D5", "D3", "F#3", "A3", "C4", "G4", "B4", "D5"]
        );
    }

    #[test]
    fn rejects_unknown_score_key() {
        let diagnostics = compile_source(
            r#"
score demo {
  key H major
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_THEORY_KEY"));
    }

    #[test]
    fn rejects_unknown_statement_key() {
        let diagnostics = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    key H major
    roman I, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_THEORY_KEY"));
    }

    #[test]
    fn rejects_unknown_modulation_key() {
        let diagnostics = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    modulate H major
    roman I, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_THEORY_KEY"));
    }

    #[test]
    fn roman_numeral_requires_score_key() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice lead {
    roman I, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_THEORY_ROMAN_KEY"));
    }

    #[test]
    fn lowers_score_title_and_composer_metadata() {
        let ir = compile_source(
            r#"
score demo {
  title "String Quartet"
  composer "Ada Lovelace"
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.title, "String Quartet");
        assert_eq!(ir.composer.as_deref(), Some("Ada Lovelace"));
        assert_eq!(
            ir.metadata.get("title").map(String::as_str),
            Some("String Quartet")
        );
        assert_eq!(
            ir.metadata.get("composer").map(String::as_str),
            Some("Ada Lovelace")
        );
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
    fn source_id_aware_facade_preserves_diagnostic_spans() {
        let diagnostics = diagnose_source_with_source_id(
            SourceId(5),
            r#"
style Classical
score demo {
  voice lead {
    note F#4, 1/4
  }
}
"#,
        );
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "ML_STYLE_SCALE")
            .unwrap();

        assert_eq!(diagnostic.span.unwrap().source_id, SourceId(5));
    }

    #[test]
    fn source_file_facade_preserves_registered_source_id() {
        let mut sources = SourceMap::new();
        sources.add("valid.music", "score ok { voice lead { note C4, 1/4 } }");
        let id = sources.add(
            "violation.music",
            r#"
style Classical
score demo {
  voice lead {
    note F#4, 1/4
  }
}
"#,
        );
        let source_file = sources.get(id).unwrap();
        let diagnostics = diagnose_source_file(source_file);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "ML_STYLE_SCALE")
            .unwrap();

        assert_eq!(source_file.id, SourceId(1));
        assert_eq!(diagnostic.span.unwrap().source_id, source_file.id);
    }

    #[test]
    fn source_file_facade_compiles_registered_file() {
        let mut sources = SourceMap::new();
        let id = sources.add("valid.music", "score ok { voice lead { note C4, 1/4 } }");
        let source_file = sources.get(id).unwrap();
        let ir = compile_source_file(source_file).unwrap();

        assert_eq!(ir.title, "ok");
        assert_eq!(ir.tracks[0].events[0].source_span.unwrap().source_id, id);
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
    fn first_accepts_tuple_values() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    note first((C4, E4, G4)), 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events[0].pitch, "C4".parse().unwrap());
    }

    #[test]
    fn first_rejects_empty_list_with_collection_diagnostic() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice lead {
    note first([]), 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "expected non-empty collection"
        }));
    }

    #[test]
    fn compares_music_expression_values() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    if "lead" == "lead" {
      note C4, 1/8
    }
    if C4 == C4 {
      note D4, 1/8
    }
    if 1/8 != 1/4 {
      note E4, 1/8
    }
    if M3 == M3 {
      note F4, 1/8
    }
  }
}
"#,
        )
        .unwrap();

        let pitches = ir.tracks[0]
            .events
            .iter()
            .map(|event| event.pitch.to_string())
            .collect::<Vec<_>>();
        assert_eq!(pitches, vec!["C4", "D4", "E4", "F4"]);
    }

    #[test]
    fn rejects_mismatched_expression_comparisons() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice lead {
    if C4 == "C4" {
      note C4, 1/4
    }
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "unsupported expression operand types"
        }));
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
        let source = r#"
style TheoryRich {
  harmonic_functions: imaginary_function
}
score demo {
  voice lead {
    note C4, 1/4
  }
}
"#;
        let diagnostics = compile_source(source).unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_UNKNOWN_THEORY_ENTRY");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("harmonic_functions"));
        let span = diagnostics[0].span.unwrap();
        let expected_start = source.find("harmonic_functions").unwrap();
        assert_eq!(span.start, expected_start);
        assert_eq!(span.end, expected_start + "harmonic_functions".len());
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
    fn idiom_style_rule_inputs_validate_entries() {
        for key in [
            "melodic_concept",
            "phrase_concept",
            "ensemble_concept",
            "bass_concept",
        ] {
            let invalid_source = format!(
                r#"
style Idiom {{
  {key}: imaginary_entry
}}
score demo style Idiom {{
  voice lead {{
    note C4, 1/4
  }}
}}
"#
            );
            let diagnostics = compile_source(&invalid_source).unwrap_err();

            assert_eq!(diagnostics[0].code, "ML_STYLE_UNKNOWN_IDIOM_ENTRY");
            assert_eq!(diagnostics[0].rule.as_deref(), Some(key));
        }
    }

    #[test]
    fn unknown_style_key_fails() {
        let source = r#"
style TheoryRich {
  imaginary_domain: anything
}
score demo {
  voice lead {
    note C4, 1/4
  }
}
"#;
        let diagnostics = compile_source(source).unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_UNKNOWN_KEY");
        let span = diagnostics[0].span.unwrap();
        let expected_start = source.find("imaginary_domain").unwrap();
        assert_eq!(span.start, expected_start);
        assert_eq!(span.end, expected_start + "imaginary_domain".len());
    }

    #[test]
    fn unknown_name_uses_stable_diagnostic_code() {
        let source = r#"
score demo {
  voice lead {
    note missing, 1/4
  }
}
"#;
        let diagnostics = compile_source(source).unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_RESOLVE_UNKNOWN_NAME");
        let span = diagnostics[0].span.unwrap();
        let expected_start = source.find("missing").unwrap();
        assert_eq!(span.start, expected_start);
        assert_eq!(span.end, expected_start + "missing".len());
    }

    #[test]
    fn unused_function_unknown_expression_name_uses_stable_diagnostic_code() {
        let source = r#"
fn hidden {
  note missing, 1/4
}
score demo {
  voice lead {
    note C4, 1/4
  }
}
"#;
        let diagnostics = compile_source(source).unwrap_err();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_RESOLVE_UNKNOWN_NAME"
                && diagnostic.message == "unknown name `missing`"
        }));
    }

    #[test]
    fn unexecuted_branch_unknown_expression_name_uses_stable_diagnostic_code() {
        let source = r#"
score demo {
  voice lead {
    if false == true {
      note missing, 1/4
    }
    note C4, 1/4
  }
}
"#;
        let diagnostics = compile_source(source).unwrap_err();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_RESOLVE_UNKNOWN_NAME"
                && diagnostic.message == "unknown name `missing`"
        }));
    }

    #[test]
    fn static_expression_resolution_accepts_let_and_for_variables() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    let d = duration 1/4
    note C4, d
    for i in 0..2 {
      if i == 1 {
        note E4, d
      }
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
    }

    #[test]
    fn duplicate_let_binding_uses_stable_diagnostic_code() {
        let source = r#"
score demo {
  voice lead {
    let d = duration 1/4
    let d = duration 1/8
    note C4, d
  }
}
"#;
        let diagnostics = compile_source(source).unwrap_err();

        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.code == "ML_RESOLVE_DUPLICATE_NAME"
                    && diagnostic.message == "duplicate binding `d`"
            })
            .unwrap();
        assert_eq!(diagnostic.related.len(), 1);
        assert_eq!(diagnostic.related[0].message, "first binding");
        assert_eq!(
            diagnostic.related[0].span.start,
            source.find("let d").unwrap()
        );
    }

    #[test]
    fn unexecuted_branch_duplicate_let_binding_fails() {
        let source = r#"
score demo {
  voice lead {
    if false == true {
      let d = duration 1/4
      let d = duration 1/8
    }
    note C4, 1/4
  }
}
"#;
        let diagnostics = compile_source(source).unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_RESOLVE_DUPLICATE_NAME"));
    }

    #[test]
    fn nested_for_scope_can_shadow_outer_binding() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    let i = 1
    for i in 0..2 {
      if i == 1 {
        note C4, 1/4
      }
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn parameterized_function_binds_call_arguments() {
        let ir = compile_source(
            r#"
fn motif(root, dur) {
  note root, dur
  note root + M3, dur
}
score demo {
  voice lead {
    call motif(C4, 1/8)
    call motif(G4, 1/4)
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].pitch.to_string(), "C4");
        assert_eq!(events[1].pitch.to_string(), "E4");
        assert_eq!(events[2].pitch.to_string(), "G4");
        assert_eq!(events[3].pitch.to_string(), "B4");
    }

    #[test]
    fn play_expands_phrase_values() {
        let ir = compile_source(
            r#"
fn riff(root) = [(root, 1/8), (root |> transpose(M3), 1/8), {p:root |> transpose(P5), d:1/4}]
score demo {
  voice lead {
    play cat(riff(C4) |> transpose(M2) |> stretch(2) |> repeat(2), riff(G4))
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 9);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "D4");
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "F#4");
        assert_eq!(ir.tracks[0].events[2].pitch.to_string(), "A4");
        assert_eq!(ir.tracks[0].events[3].pitch.to_string(), "D4");
        assert_eq!(ir.tracks[0].events[4].pitch.to_string(), "F#4");
        assert_eq!(ir.tracks[0].events[5].pitch.to_string(), "A4");
        assert_eq!(ir.tracks[0].events[6].pitch.to_string(), "G4");
        assert_eq!(ir.tracks[0].events[7].pitch.to_string(), "B4");
        assert_eq!(ir.tracks[0].events[8].pitch.to_string(), "D5");
        assert_eq!(ir.tracks[0].events[0].duration_ticks, 480);
        assert_eq!(ir.tracks[0].events[2].duration_ticks, 960);
    }

    #[test]
    fn concat_flattens_tuple_phrase_values() {
        let ir = compile_source(
            r#"
fn left(root) = ((root, 1/8), (root |> transpose(M3), 1/8))
fn right(root) = ({p:root, d:1/8}, {p:root |> transpose(P5), d:1/8})
fn phrase(root) = concat(left(root), right(G4))
score demo {
  voice lead {
    play first(phrase(C4))
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "C4");
    }

    #[test]
    fn expression_bodied_function_returns_value() {
        let ir = compile_source(
            r#"
fn up(p, i) = p |> transpose(i)
score demo {
  voice lead {
    note up(C4, M3), 1/8
    note up(G4, M3), 1/8
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "E4");
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "B4");
    }

    #[test]
    fn maps_phrase_values_through_expression_function() {
        let ir = compile_source(
            r#"
fn riff(root) = [(root, 1/8), {p:root |> transpose(M3), d:1/4}]
fn lift(event) = event |> transpose(P5)
score demo {
  voice lead {
    play riff(C4) |> map(lift)
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "G4");
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "B4");
        assert_eq!(ir.tracks[0].events[0].duration_ticks, 240);
        assert_eq!(ir.tracks[0].events[1].duration_ticks, 480);
    }

    #[test]
    fn filters_phrase_values_through_expression_function() {
        let ir = compile_source(
            r#"
fn riff(root) = [{p:root, d:1/8, keep:true}, {p:root |> transpose(M3), d:1/8, keep:false}, {p:root |> transpose(P5), d:1/4, keep:true}]
fn keep(event) = event.keep == true
score demo {
  voice lead {
    play riff(C4) |> filter(keep)
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "C4");
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "G4");
        assert_eq!(ir.tracks[0].events[0].duration_ticks, 240);
        assert_eq!(ir.tracks[0].events[1].duration_ticks, 480);
    }

    #[test]
    fn maps_phrase_values_with_index() {
        let ir = compile_source(
            r#"
fn riff(root) = [{p:root, d:1/8}, {p:root |> transpose(M3), d:1/8}, {p:root |> transpose(P5), d:1/4}]
fn mark(i, event) = {p:event.p, d:event.d, middle:i == 1}
fn keep(event) = event.middle == true
score demo {
  voice lead {
    play riff(C4) |> mapi(mark) |> filter(keep)
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "E4");
        assert_eq!(ir.tracks[0].events[0].duration_ticks, 240);
    }

    #[test]
    fn direct_transform_calls_accept_bare_function_names() {
        let ir = compile_source(
            r#"
fn riff(root) = [{p:root, d:1/8}, {p:root |> transpose(M3), d:1/8}, {p:root |> transpose(P5), d:1/8}]
fn mark(i, event) = event.with({middle:i == 1})
fn keep(event) = event.middle == true
fn lift(event) = event |> transpose(M2)
score demo {
  voice lead {
    play map(filter(mapi(riff(C4), mark), keep), lift)
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "F#4");
    }

    #[test]
    fn method_transform_calls_accept_bare_function_names() {
        let ir = compile_source(
            r#"
fn riff(root) = [{p:root, d:1/8}, {p:root |> transpose(M3), d:1/8}, {p:root |> transpose(P5), d:1/8}]
fn mark(i, event) = event.with({middle:i == 1})
fn keep(event) = event.middle == true
fn lift(event) = event |> transpose(M2)
score demo {
  voice lead {
    play riff(C4).mapi(mark).filter(keep).map(lift)
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "F#4");
    }

    #[test]
    fn method_transform_calls_accept_string_function_names() {
        let ir = compile_source(
            r#"
fn riff(root) = [{p:root, d:1/8}, {p:root |> transpose(M3), d:1/8}, {p:root |> transpose(P5), d:1/8}]
fn mark(i, event) = event.with({middle:i == 1})
fn keep(event) = event.middle == true
fn lift(event) = event |> transpose(M2)
score demo {
  voice lead {
    play riff(C4).mapi("mark").filter("keep").map("lift")
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "F#4");
    }

    #[test]
    fn maps_filters_and_indexes_tuple_phrase_values() {
        let ir = compile_source(
            r#"
fn riff(root) = ({p:root, d:1/8}, {p:root |> transpose(M3), d:1/8}, {p:root |> transpose(P5), d:1/8})
fn mark(i, event) = event.with({middle:i == 1})
fn keep(event) = event.middle == true
fn lift(event) = event |> transpose(M2)
score demo {
  voice lead {
    play riff(C4) |> mapi(mark) |> filter(keep) |> map(lift)
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 1);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "F#4");
    }

    #[test]
    fn conditional_expression_shapes_phrase_transform() {
        let ir = compile_source(
            r#"
fn riff(root) = [(root, 1/8), (root |> transpose(M3), 1/8), (root |> transpose(P5), 1/8)]
fn vary(i, event) = if i == 1 then event |> transpose(M2) else event
score demo {
  voice lead {
    play riff(C4) |> mapi(vary)
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 3);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "C4");
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "F#4");
        assert_eq!(ir.tracks[0].events[2].pitch.to_string(), "G4");
    }

    #[test]
    fn comparison_expressions_shape_phrase_predicates() {
        let ir = compile_source(
            r#"
fn riff(root) = [{p:root, d:1/8}, {p:root |> transpose(M2), d:1/8}, {p:root |> transpose(M3), d:1/8}, {p:root |> transpose(P5), d:1/8}]
fn ne(i, event) = {p:event.p, d:event.d, keep:i != 1}
fn lt(i, event) = {p:event.p, d:event.d, keep:i < 3}
fn le(i, event) = {p:event.p, d:event.d, keep:i <= 2}
fn gt(i, event) = {p:event.p, d:event.d, keep:i > 0}
fn ge(i, event) = {p:event.p, d:event.d, keep:i >= 1}
fn keep(event) = event.keep == true
score demo {
  voice lead {
    play riff(C4) |> mapi(ne) |> filter(keep)
    play riff(C4) |> mapi(lt) |> filter(keep)
    play riff(C4) |> mapi(le) |> filter(keep)
    play riff(C4) |> mapi(gt) |> filter(keep)
    play riff(C4) |> mapi(ge) |> filter(keep)
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 15);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "C4");
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "E4");
        assert_eq!(ir.tracks[0].events[2].pitch.to_string(), "G4");
        assert_eq!(ir.tracks[0].events[14].pitch.to_string(), "G4");
    }

    #[test]
    fn boolean_composition_and_nested_conditionals_shape_phrase_predicates() {
        let ir = compile_source(
            r#"
fn riff(root) = [{p:root, d:1/8, accent:true}, {p:root |> transpose(M2), d:1/8, accent:false}, {p:root |> transpose(M3), d:1/8, accent:true}, {p:root |> transpose(P5), d:1/8, accent:false}]
fn mark(i, event) = {p:event.p, d:event.d, accent:event.accent, early:i < 2, late:i >= 2}
fn keep(event) = if event.accent == true then event.early == true or event.late == true else event.late == true and event.early != true
score demo {
  voice lead {
    play riff(C4) |> mapi(mark) |> filter(keep)
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 3);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "C4");
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "E4");
        assert_eq!(ir.tracks[0].events[2].pitch.to_string(), "G4");
    }

    #[test]
    fn range_comprehension_generates_indexed_phrase_material() {
        let ir = compile_source(
            r#"
fn line() = [{p:at([C4, D4, E4, G4], i), d:1/8} for i in 0..4]
score demo {
  voice lead {
    play line()
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 4);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "C4");
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "D4");
        assert_eq!(ir.tracks[0].events[2].pitch.to_string(), "E4");
        assert_eq!(ir.tracks[0].events[3].pitch.to_string(), "G4");
    }

    #[test]
    fn descending_range_comprehension_generates_material() {
        let ir = compile_source(
            r#"
fn line() = [{p:at([C4, D4, E4, G4], i), d:1/8} for i in 3..0]
score demo {
  voice lead {
    play line()
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 3);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "G4");
        assert_eq!(ir.tracks[0].events[2].pitch.to_string(), "D4");
    }

    #[test]
    fn range_rejects_non_int_bounds() {
        let diagnostics = diagnose_source(
            r#"
fn bad() = [i for i in C4..4]
score demo {
  voice lead {
    let x = bad()
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "expected int range bounds"
        }));
    }

    #[test]
    fn list_comprehension_shapes_phrase_material() {
        let ir = compile_source(
            r#"
fn riff(root) = [{p:root, d:1/8, skip:false}, {p:root |> transpose(M2), d:1/8, skip:true}, {p:root |> transpose(M3), d:1/8, skip:false}]
fn lift(events) = [event.with({d:1/2}) for event in events if not event.skip]
score demo {
  voice lead {
    play lift(riff(C4))
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "C4");
        assert_eq!(ir.tracks[0].events[0].duration_ticks, 960);
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "E4");
        assert_eq!(ir.tracks[0].events[1].duration_ticks, 960);
    }

    #[test]
    fn tuple_comprehension_shapes_phrase_material() {
        let ir = compile_source(
            r#"
fn riff(root) = ({p:root, d:1/8, skip:false}, {p:root |> transpose(M2), d:1/8, skip:true}, {p:root |> transpose(M3), d:1/8, skip:false})
fn lift(events) = [event.with({d:1/2}) for event in events if not event.skip]
score demo {
  voice lead {
    play lift(riff(C4))
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "C4");
        assert_eq!(ir.tracks[0].events[0].duration_ticks, 960);
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "E4");
        assert_eq!(ir.tracks[0].events[1].duration_ticks, 960);
    }

    #[test]
    fn list_comprehension_rejects_non_collection_source() {
        let diagnostics = diagnose_source(
            r#"
fn bad(p) = [event for event in p]
score demo {
  voice lead {
    let x = bad(C4)
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "expected collection source"
        }));
    }

    #[test]
    fn list_comprehension_rejects_non_bool_condition() {
        let diagnostics = diagnose_source(
            r#"
fn bad(events) = [event for event in events if event.p]
score demo {
  voice lead {
    let x = bad([{p:C4, d:1/4}])
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "expected comprehension condition to be bool"
        }));
    }

    #[test]
    fn unary_not_inverts_boolean_predicate() {
        let ir = compile_source(
            r#"
fn riff(root) = [{p:root, d:1/8, keep:true}, {p:root |> transpose(M2), d:1/8, keep:false}, {p:root |> transpose(M3), d:1/8, keep:true}]
fn keep(event) = not event.keep == false
score demo {
  voice lead {
    play riff(C4) |> filter(keep)
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "C4");
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "E4");
    }

    #[test]
    fn unary_not_non_bool_reports_type_mismatch() {
        let diagnostics = diagnose_source(
            r#"
fn bad(p) = not p
score demo {
  voice lead {
    let x = bad(C4)
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH" && diagnostic.message == "expected bool operand"
        }));
    }

    #[test]
    fn integer_arithmetic_shapes_phrase_predicates() {
        let ir = compile_source(
            r#"
fn riff(root) = [{p:root, d:1/8}, {p:root |> transpose(M2), d:1/8}, {p:root |> transpose(M3), d:1/8}, {p:root |> transpose(P5), d:1/8}]
fn mark(i, event) = {p:event.p, d:event.d, keep:i * 2 + 1 >= 5 - 2 and i / 2 == 1}
fn keep(event) = event.keep == true
score demo {
  voice lead {
    play riff(C4) |> mapi(mark) |> filter(keep)
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "E4");
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "G4");
    }

    #[test]
    fn integer_division_by_zero_reports_eval_diagnostic() {
        let diagnostics = diagnose_source(
            r#"
fn bad(i) = i / 0
score demo {
  voice lead {
    note C4, 1/8
    let x = bad(1)
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_EVAL_UNSUPPORTED_OP" && diagnostic.message == "division by zero"
        }));
    }

    #[test]
    fn collection_builtins_index_phrase_material() {
        let ir = compile_source(
            r#"
fn motif(root) = [{p:root, d:1/8}, {p:root |> transpose(M3), d:1/8}, {p:root |> transpose(P5), d:1/4}]
fn choose(events) = [at(events, 0), at(events, len(events) - 1)]
score demo {
  voice lead {
    play choose(motif(C4))
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "C4");
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "G4");
        assert_eq!(ir.tracks[0].events[1].duration_ticks, 480);
    }

    #[test]
    fn collection_index_out_of_range_reports_diagnostic() {
        let diagnostics = diagnose_source(
            r#"
fn bad(events) = at(events, 3)
score demo {
  voice lead {
    let x = bad([(C4, 1/8)])
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "collection index out of range"
        }));
    }

    #[test]
    fn dict_merge_builtin_produces_updated_phrase_events() {
        let ir = compile_source(
            r#"
fn riff(root) = [{p:root, d:1/8}, {p:root |> transpose(M3), d:1/8}, {p:root |> transpose(P5), d:1/4}]
fn lift(event) = merge(event, {d:1/2})
score demo {
  voice lead {
    play riff(C4) |> map(lift)
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 3);
        assert_eq!(ir.tracks[0].events[0].duration_ticks, 960);
        assert_eq!(ir.tracks[0].events[1].duration_ticks, 960);
        assert_eq!(ir.tracks[0].events[2].duration_ticks, 960);
    }

    #[test]
    fn method_style_dict_with_updates_phrase_events() {
        let ir = compile_source(
            r#"
fn riff(root) = [{p:root, d:1/8}, {p:root |> transpose(P5), d:1/4}]
fn long(event) = event.with({d:1/2})
score demo {
  voice lead {
    play riff(C4) |> map(long)
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
        assert_eq!(ir.tracks[0].events[0].duration_ticks, 960);
        assert_eq!(ir.tracks[0].events[1].duration_ticks, 960);
    }

    #[test]
    fn dict_non_dict_with_reports_type_mismatch() {
        let diagnostics = diagnose_source(
            r#"
fn bad(event) = with(event, 42)
score demo {
  voice lead {
    let x = bad({p:C4, d:1/4})
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "builtin `with` expects dict arguments"
        }));
    }

    #[test]
    fn builtin_wrong_argument_count_reports_type_mismatch() {
        let diagnostics = diagnose_source(
            r#"
fn bad(events) = at(events)
score demo {
  voice lead {
    let x = bad([C4])
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "builtin `at` expects 2 arguments, got 1"
        }));
    }

    #[test]
    fn builtin_wrong_argument_type_reports_type_mismatch() {
        let diagnostics = diagnose_source(
            r#"
fn bad(events) = at(events, "zero")
score demo {
  voice lead {
    let x = bad([C4])
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "builtin `at` expects collection and integer index"
        }));
    }

    #[test]
    fn non_at_builtin_wrong_argument_type_reports_type_mismatch() {
        let diagnostics = diagnose_source(
            r#"
fn bad(events) = repeat(events, "twice")
score demo {
  voice lead {
    let x = bad([C4])
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "builtin `repeat` expects value and integer count"
        }));
    }

    #[test]
    fn pipe_builtin_wrong_argument_type_reports_type_mismatch() {
        let diagnostics = diagnose_source(
            r#"
fn bad(events) = events |> repeat("twice")
score demo {
  voice lead {
    let x = bad([C4])
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "builtin `repeat` expects value and integer count"
        }));
    }

    #[test]
    fn pipe_builtin_wrong_argument_count_reports_type_mismatch() {
        let diagnostics = diagnose_source(
            r#"
fn bad(events) = events |> at()
score demo {
  voice lead {
    let x = bad([C4])
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "builtin `at` expects 2 arguments, got 1"
        }));
    }

    #[test]
    fn method_builtin_wrong_argument_type_reports_type_mismatch() {
        let diagnostics = diagnose_source(
            r#"
fn bad(event) = event.with(42)
score demo {
  voice lead {
    let x = bad({p:C4, d:1/4})
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "builtin `with` expects dict arguments"
        }));
    }

    #[test]
    fn method_builtin_wrong_argument_count_reports_type_mismatch() {
        let diagnostics = diagnose_source(
            r#"
fn bad(event) = event.with()
score demo {
  voice lead {
    let x = bad({p:C4, d:1/4})
  }
}
"#,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "builtin `with` expects 2 arguments, got 1"
        }));
    }

    #[test]
    fn function_accepts_compact_dict_argument() {
        let ir = compile_source(
            r#"
fn hit(cfg) {
  note cfg.root |> transpose(M3), cfg.dur
}
score demo {
  voice lead {
    call hit({root:C4, dur:1/8})
    call hit({root:E4, dur:1/4})
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "E4");
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "G#4");
    }

    #[test]
    fn function_accepts_compact_tuple_argument() {
        let ir = compile_source(
            r#"
fn hit(pair) {
  note pair.0, pair.1
}
score demo {
  voice lead {
    call hit((C4, 1/8))
    call hit((G4, 1/4))
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks[0].events.len(), 2);
        assert_eq!(ir.tracks[0].events[0].pitch.to_string(), "C4");
        assert_eq!(ir.tracks[0].events[1].pitch.to_string(), "G4");
    }

    #[test]
    fn wrong_function_argument_count_uses_stable_diagnostic_code() {
        let diagnostics = compile_source(
            r#"
fn motif(root, dur) {
  note root, dur
}
score demo {
  voice lead {
    call motif(C4)
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "function `motif` expects 2 arguments, got 1"
        }));
    }

    #[test]
    fn wrong_expression_function_argument_count_uses_stable_diagnostic_code() {
        let diagnostics = compile_source(
            r#"
fn pick(root, dur) = {p:root, d:dur}
score demo {
  voice lead {
    play [pick(C4)]
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "function `pick` expects 2 arguments, got 1"
        }));
    }

    #[test]
    fn expression_call_to_block_function_uses_stable_diagnostic_code() {
        let diagnostics = compile_source(
            r#"
fn motif(root) {
  note root, 1/4
}
score demo {
  voice lead {
    play [motif(C4)]
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_TYPE_MISMATCH"
                && diagnostic.message == "function `motif` is not expression-bodied"
        }));
    }

    #[test]
    fn call_argument_unknown_name_uses_stable_diagnostic_code() {
        let diagnostics = compile_source(
            r#"
fn motif(root) {
  note root, 1/4
}
score demo {
  voice lead {
    call motif(missing)
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "ML_RESOLVE_UNKNOWN_NAME"
                && diagnostic.message == "unknown name `missing`"
        }));
    }

    #[test]
    fn unused_function_unknown_call_uses_stable_diagnostic_code() {
        let source = r#"
fn hidden {
  call missing_motif
}
score demo {
  voice lead {
    note C4, 1/4
  }
}
"#;
        let diagnostics = compile_source(source).unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_RESOLVE_UNKNOWN_NAME"));
    }

    #[test]
    fn unexecuted_branch_unknown_call_uses_stable_diagnostic_code() {
        let source = r#"
score demo {
  voice lead {
    if false == true {
      call missing_motif
    }
    note C4, 1/4
  }
}
"#;
        let diagnostics = compile_source(source).unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_RESOLVE_UNKNOWN_NAME"));
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
        assert_eq!(diagnostics[0].related.len(), 1);
        assert_eq!(
            diagnostics[0].related[0].message,
            "first function definition"
        );
        assert_eq!(
            diagnostics[0].related[0].span.start,
            source.find("fn motif").unwrap()
        );
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
    fn unused_recursive_call_uses_stable_diagnostic_code() {
        let diagnostics = compile_source(
            r#"
fn motif {
  call motif
}
score demo {
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_RESOLVE_RECURSIVE_CALL"));
    }

    #[test]
    fn unused_indirect_recursive_call_uses_stable_diagnostic_code() {
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
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_RESOLVE_RECURSIVE_CALL"));
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
        assert_eq!(
            diagnostics[0].help.as_deref(),
            Some("use a built-in rule id or declare a custom rule with rule_<id> in the active style")
        );
        let span = diagnostics[0].span.unwrap();
        let expected_start = source.find("override imaginary").unwrap();
        assert_eq!(span.start, expected_start);
        assert_eq!(span.end, expected_start + "override".len());
    }

    #[test]
    fn unused_function_unknown_override_rule_fails() {
        let diagnostics = compile_source(
            r#"
fn hidden {
  override imaginary allow {
    note C4, 1/4
  }
}
score demo {
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_STYLE_UNKNOWN_RULE"));
    }

    #[test]
    fn unexecuted_branch_unknown_override_rule_fails() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice lead {
    if false == true {
      override imaginary allow {
        note C4, 1/4
      }
    }
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_STYLE_UNKNOWN_RULE"));
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
    fn lowers_scale_degree_to_melodic_event() {
        let ir = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    degree #4 4, 1/8
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.melodic_events.len(), 1);
        assert_eq!(ir.melodic_events[0].kind, "scale_degree");
        assert_eq!(ir.melodic_events[0].degree, Some(4));
        assert_eq!(ir.melodic_events[0].accidental, 1);
        assert_eq!(ir.melodic_events[0].start_tick, 0);
        assert_eq!(ir.melodic_events[0].duration_ticks, 240);
    }

    #[test]
    fn lowers_scale_run_to_melodic_events() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    scale C major 4, 1/8
  }
}
"#,
        )
        .unwrap();

        let degrees = ir
            .melodic_events
            .iter()
            .map(|event| event.degree)
            .collect::<Vec<_>>();
        assert_eq!(
            degrees,
            vec![
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(7),
                Some(1)
            ]
        );
        assert!(ir
            .melodic_events
            .iter()
            .all(|event| event.kind == "scale_run"));
    }

    #[test]
    fn lowers_section_to_form_event() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    section A {
      note C4, 1/4
      note D4, 1/4
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.form_events.len(), 1);
        assert_eq!(ir.form_events[0].kind, "section");
        assert_eq!(ir.form_events[0].label, "A");
        assert_eq!(ir.form_events[0].start_tick, 0);
        assert_eq!(ir.form_events[0].duration_ticks, 960);
        assert!(ir.form_events[0].source_span.is_some());
        assert_eq!(ir.phrase_events.len(), 1);
        assert_eq!(ir.phrase_events[0].kind, "section");
        assert_eq!(ir.phrase_events[0].label.as_deref(), Some("A"));
        assert_eq!(ir.phrase_events[0].start_tick, 0);
        assert_eq!(ir.phrase_events[0].duration_ticks, 960);
        assert!(ir.phrase_events[0].source_span.is_some());
    }

    #[test]
    fn lowers_function_call_to_motif_event() {
        let ir = compile_source(
            r#"
fn motif(root) {
  note root, 1/8
  note root + M2, 1/8
}

score demo {
  voice lead {
    call motif(C4)
    call motif(G4)
  }
}
"#,
        )
        .unwrap();

        let names = ir
            .motif_events
            .iter()
            .map(|event| event.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["motif", "motif"]);
        assert_eq!(
            ir.motif_events[0].transform.as_deref(),
            Some("transposition")
        );
        assert_eq!(ir.motif_events[0].start_tick, 0);
        assert_eq!(ir.motif_events[0].duration_ticks, 480);
        assert_eq!(
            ir.motif_events[1].transform.as_deref(),
            Some("transposition")
        );
        assert_eq!(ir.motif_events[1].start_tick, 480);
        assert_eq!(ir.motif_events[1].duration_ticks, 480);
        assert!(ir
            .motif_events
            .iter()
            .all(|event| event.source_span.is_some()));
        let phrase_labels = ir
            .phrase_events
            .iter()
            .map(|event| event.label.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(phrase_labels, vec![Some("motif"), Some("motif")]);
        assert!(ir
            .phrase_events
            .iter()
            .all(|event| event.kind == "motif_call"));
        assert_eq!(ir.phrase_events[0].start_tick, 0);
        assert_eq!(ir.phrase_events[0].duration_ticks, 480);
        assert_eq!(ir.phrase_events[1].start_tick, 480);
        assert_eq!(ir.phrase_events[1].duration_ticks, 480);
    }

    #[test]
    fn classifies_duration_argument_as_motif_rhythmic_variation() {
        let ir = compile_source(
            r#"
fn motif(d) {
  note C4, d
}

score demo {
  voice lead {
    call motif(1/8)
    call motif(1/4)
  }
}
"#,
        )
        .unwrap();

        let transforms = ir
            .motif_events
            .iter()
            .map(|event| event.transform.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(
            transforms,
            vec![Some("rhythmic_variation"), Some("rhythmic_variation")]
        );
    }

    #[test]
    fn lowers_roman_progression_to_harmonic_events() {
        let ir = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    progression I ii V7 I, 1/4
  }
}
"#,
        )
        .unwrap();

        let functions = ir
            .harmonic_events
            .iter()
            .filter_map(|event| event.function.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(functions, vec!["tonic", "predominant", "dominant", "tonic"]);
        assert_eq!(ir.harmonic_events[2].symbol, "V7");
        assert_eq!(ir.harmonic_events[2].normalized_symbol, "V");
        assert_eq!(ir.harmonic_events[2].degree, Some(5));
        assert_eq!(
            ir.harmonic_events[2].cadence_role.as_deref(),
            Some("dominant_preparation")
        );
        assert_eq!(ir.harmonic_events[2].start_tick, 960);
        assert_eq!(ir.harmonic_events[2].duration_ticks, 480);
    }

    #[test]
    fn applied_roman_preserves_target_and_secondary_function() {
        let ir = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    roman V7/V, 1/4
  }
}
"#,
        )
        .unwrap();

        let event = &ir.harmonic_events[0];
        assert_eq!(event.normalized_symbol, "V/V");
        assert_eq!(event.degree, Some(5));
        assert_eq!(event.applied_to.as_deref(), Some("V"));
        assert_eq!(event.function.as_deref(), Some("secondary_dominant"));
    }

    #[test]
    fn harmonic_progression_style_prefers_explicit_roman_function() {
        let compilation = compile_source_with_diagnostics(
            r#"
style Functional {
  harmonic_progression: tonic predominant dominant tonic
}
score demo style Functional {
  key C major
  voice lead {
    progression I ii V7 I, 1/4
  }
}
"#,
        )
        .unwrap();

        assert!(compilation.diagnostics.is_empty());
        assert_eq!(compilation.ir.harmonic_events.len(), 4);
    }

    #[test]
    fn explicit_harmonic_events_support_cadence_checks() {
        let compilation = compile_source_with_diagnostics(
            r#"
style Cadential {
  cadence: authentic
}
score demo style Cadential {
  key C major
  voice lead {
    cadence authentic, 1/2
  }
}
"#,
        )
        .unwrap();

        assert!(compilation.diagnostics.is_empty());
        assert_eq!(
            compilation.ir.harmonic_events[0].function.as_deref(),
            Some("dominant")
        );
        assert_eq!(
            compilation.ir.harmonic_events[1].function.as_deref(),
            Some("tonic")
        );
        assert_eq!(
            compilation.ir.harmonic_events[1].cadence_role.as_deref(),
            Some("arrival")
        );
    }

    #[test]
    fn lowers_tempo_changes_at_current_tick() {
        let ir = compile_source(
            r#"
score demo {
  tempo 96
  voice lead {
    note C4, 1/4
    tempo 144
    note E4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tempo_bpm, 96);
        assert_eq!(ir.tempo_changes.len(), 1);
        assert_eq!(ir.tempo_changes[0].bpm, 144);
        assert_eq!(ir.tempo_changes[0].tick, 480);
    }

    #[test]
    fn lowers_meter_changes_at_current_tick() {
        let ir = compile_source(
            r#"
score demo {
  meter 4/4
  voice lead {
    note C4, 1/4
    meter 6/8
    note E4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.meter.unwrap().numerator, 4);
        assert_eq!(ir.meter_changes.len(), 1);
        assert_eq!(ir.meter_changes[0].meter.numerator, 6);
        assert_eq!(ir.meter_changes[0].meter.denominator, 8);
        assert_eq!(ir.meter_changes[0].tick, 480);
    }

    #[test]
    fn lowers_key_changes_at_current_tick() {
        let ir = compile_source(
            r#"
score demo {
  key C major
  voice lead {
    note C4, 1/4
    key G major
    roman I, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.key.unwrap().fifths, 0);
        assert_eq!(ir.key_changes.len(), 1);
        assert_eq!(ir.key_changes[0].key.fifths, 1);
        assert!(!ir.key_changes[0].key.is_minor);
        assert_eq!(ir.key_changes[0].tick, 480);
        assert_eq!(ir.tracks[0].events[1].pitch.class(), PitchClass::G);
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
    fn meter_statement_rule_fails() {
        let diagnostics = compile_source(
            r#"
style Three {
  meter: 3/4
}
score demo {
  meter 3/4
  voice lead {
    meter 4/4
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_STYLE_METER"));
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
    fn voice_spacing_rule_fails() {
        let diagnostics = compile_source(
            r#"
style CloseVoicing {
  voice_spacing: P8
}
score demo style CloseVoicing {
  voice soprano {
    note C6, 1/4
  }
  voice alto {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_VOICE_SPACING");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("voice_spacing"));
    }

    #[test]
    fn override_allows_voice_spacing_violation() {
        let ir = compile_source(
            r#"
style CloseVoicing {
  voice_spacing: P8
}
score demo style CloseVoicing {
  override voice_spacing allow reason "registral antiphony" {
    voice soprano {
      note C6, 1/4
    }
    voice alto {
      note C4, 1/4
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks.len(), 2);
        assert_eq!(ir.overrides[0].rule, "voice_spacing");
    }

    #[test]
    fn voice_spacing_rule_can_be_disabled() {
        let ir = compile_source(
            r#"
style OpenVoicing {
  voice_spacing: P8
  severity_voice_spacing: off
}
score demo style OpenVoicing {
  voice soprano {
    note C6, 1/4
  }
  voice alto {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks.len(), 2);
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
    fn builtin_jazz_warns_without_swing_identity() {
        let compilation = compile_source_with_diagnostics(
            r#"
score demo style Jazz {
  voice lead {
    note C4, 1/8
    note D4, 1/8
    note E4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert!(compilation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule.as_deref() == Some("rhythm_concept")));
    }

    #[test]
    fn builtin_jazz_warns_without_functional_harmony() {
        let compilation = compile_source_with_diagnostics(
            r#"
score demo style Jazz {
  voice lead {
    note C4, 1/6
    note D4, 1/12
    rest 1/8
    note Eb4, 1/8
    note E4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert!(compilation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule.as_deref() == Some("harmonic_progression")));
    }

    #[test]
    fn builtin_jazz_warns_without_blues_inflection() {
        let compilation = compile_source_with_diagnostics(
            r#"
score demo style Jazz {
  voice horn {
    note C5, 1/6
    note D5, 1/12
    rest 1/8
    note E5, 1/8
    rest 1/4
  }
  voice lead {
    rest 1/2
    note E4, 1/8
    note G4, 1/8
    note B4, 1/4
  }
  voice piano {
    chord D3 minor7, 1/4
    chord G3 dominant7, 1/4
    chord C3 major7, 1/4
    chord G3 dominant7, 1/4
    chord C3 major7, 1/2
  }
  voice bass {
    note C2, 1/4
    note E2, 1/4
    note G2, 1/4
    note B2, 1/4
  }
}
"#,
        )
        .unwrap();

        assert!(compilation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule.as_deref() == Some("melodic_concept")));
    }

    #[test]
    fn melodic_concept_prefers_explicit_degree_inflection() {
        let compilation = compile_source_with_diagnostics(
            r#"
style BluesDegree {
  melodic_concept: blues_inflection
}
score demo style BluesDegree {
  key C major
  voice lead {
    degree b3 4, 1/4
  }
}
"#,
        )
        .unwrap();

        assert!(!compilation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule.as_deref() == Some("melodic_concept")));
    }

    #[test]
    fn builtin_jazz_warns_without_call_response() {
        let compilation = compile_source_with_diagnostics(
            r#"
score demo style Jazz {
  voice lead {
    note C5, 1/6
    note D5, 1/12
    rest 1/8
    note Eb5, 1/8
    rest 1/4
  }
  voice piano {
    chord D3 minor7, 1/4
    chord G3 dominant7, 1/4
    chord C3 major7, 1/4
    chord G3 dominant7, 1/4
    chord C3 major7, 1/2
  }
  voice bass {
    note C2, 1/4
    note E2, 1/4
    note G2, 1/4
    note B2, 1/4
  }
}
"#,
        )
        .unwrap();

        assert!(compilation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule.as_deref() == Some("ensemble_concept")));
    }

    #[test]
    fn builtin_jazz_warns_without_bass_support() {
        let compilation = compile_source_with_diagnostics(
            r#"
score demo style Jazz {
  voice horn {
    note C5, 1/6
    note D5, 1/12
    rest 1/8
    note Eb5, 1/8
    rest 1/4
  }
  voice lead {
    rest 1/2
    note Eb4, 1/8
    note G4, 1/8
    note Bb4, 1/4
  }
  voice piano {
    chord D3 minor7, 1/4
    chord G3 dominant7, 1/4
    chord C3 major7, 1/4
    chord G3 dominant7, 1/4
    chord C3 major7, 1/2
  }
}
"#,
        )
        .unwrap();

        assert!(compilation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule.as_deref() == Some("bass_concept")));
    }

    #[test]
    fn builtin_jazz_accepts_swing_syncopation_and_functional_cadence() {
        let compilation = compile_source_with_diagnostics(
            r#"
score demo style Jazz {
  voice horn {
    note C5, 1/6
    note D5, 1/12
    rest 1/8
    note Eb5, 1/8
    rest 1/4
  }
  voice lead {
    rest 1/2
    note Eb4, 1/8
    note G4, 1/8
    note Bb4, 1/4
  }
  voice piano {
    chord D3 minor7, 1/4
    chord G3 dominant7, 1/4
    chord C3 major7, 1/4
    chord G3 dominant7, 1/4
    chord C3 major7, 1/2
  }
  voice bass {
    note C2, 1/4
    note E2, 1/4
    note G2, 1/4
    note B2, 1/4
  }
}
"#,
        )
        .unwrap();

        assert!(
            compilation.diagnostics.is_empty(),
            "unexpected jazz diagnostics: {:?}",
            compilation.diagnostics
        );
    }

    #[test]
    fn jazz_cadence_ignores_drum_track_pitches() {
        let compilation = compile_source_with_diagnostics(
            r#"
score demo style Jazz {
  voice horn {
    note C5, 1/6
    note D5, 1/12
    rest 1/8
    note Eb5, 1/8
    rest 1/4
  }
  voice lead {
    rest 1/2
    note Eb4, 1/8
    note G4, 1/8
    note Bb4, 1/4
  }
  voice piano {
    chord D3 minor7, 1/4
    chord G3 dominant7, 1/4
    chord C3 major7, 1/4
    chord G3 dominant7, 1/4
    chord C3 major7, 1/2
  }
  voice bass {
    note C2, 1/4
    note E2, 1/4
    note G2, 1/4
    note B2, 1/4
  }
  voice kit {
    instrument drums
    channel 9
    rest 1/1
    drum kick, 1/2
  }
}
"#,
        )
        .unwrap();

        assert!(
            compilation.diagnostics.is_empty(),
            "unexpected jazz diagnostics: {:?}",
            compilation.diagnostics
        );
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
    fn phrase_concept_periodic_phrase_accepts_two_sections() {
        let ir = compile_source(
            r#"
style Periodic {
  phrase_concept: periodic_phrase
}
score demo style Periodic {
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

        assert_eq!(ir.phrase_events.len(), 2);
    }

    #[test]
    fn phrase_concept_periodic_phrase_rejects_single_section() {
        let diagnostics = compile_source(
            r#"
style Periodic {
  phrase_concept: periodic_phrase
}
score demo style Periodic {
  voice lead {
    section A {
      note C4, 1/4
    }
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_STYLE_PHRASE_CONCEPT");
        assert_eq!(diagnostics[0].rule.as_deref(), Some("phrase_concept"));
    }

    #[test]
    fn phrase_concept_motivic_development_accepts_transformed_motif() {
        compile_source(
            r#"
fn motif(root) {
  note root, 1/4
}
style Developed {
  phrase_concept: motivic_development
}
score demo style Developed {
  voice lead {
    call motif(C4)
    call motif(G4)
  }
}
"#,
        )
        .unwrap();
    }

    #[test]
    fn phrase_concept_can_be_overridden() {
        compile_source(
            r#"
style Periodic {
  phrase_concept: periodic_phrase
}
score demo style Periodic {
  voice lead {
    override phrase_concept allow reason "intro fragment" {
      section A {
        note C4, 1/4
      }
    }
  }
}
"#,
        )
        .unwrap();
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
        assert_eq!(ir.form_events.len(), 2);
        assert_eq!(ir.form_events[0].label, "A");
        assert_eq!(ir.form_events[1].label, "B");
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
    fn tempo_statement_range_rule_fails() {
        let diagnostics = compile_source(
            r#"
style Slow {
  tempo_range: 40..80
}
score demo {
  tempo 60
  voice lead {
    section Bridge {
      tempo 120
      note C4, 1/4
    }
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_STYLE_TEMPO_RANGE"));
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
    fn counterpoint_rules_ignore_unpitched_drum_track() {
        let ir = compile_source(
            r#"
score demo {
  voice bass {
    note C2, 1/4
  }
  voice kit {
    instrument drums
    channel 9
    drum snare, 1/4
  }
}
"#,
        )
        .unwrap();

        assert_eq!(ir.tracks.len(), 2);
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
    fn unused_function_unknown_local_style_fails() {
        let diagnostics = compile_source(
            r#"
style Classical
fn hidden {
  with style Missing {
    note C4, 1/4
  }
}
score demo style Classical {
  voice lead {
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_STYLE_UNKNOWN_NAME"));
    }

    #[test]
    fn unexecuted_branch_unknown_local_style_fails() {
        let diagnostics = compile_source(
            r#"
style Classical
score demo style Classical {
  voice lead {
    if false == true {
      with style Missing {
        note C4, 1/4
      }
    }
    note C4, 1/4
  }
}
"#,
        )
        .unwrap_err();

        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ML_STYLE_UNKNOWN_NAME"));
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
    fn non_scale_style_warning_does_not_block_compile() {
        let compilation = compile_source_with_diagnostics(
            r#"
style Tiny {
  instrument_range: 40 C4 C5
  severity_instrument_range: warning
}
score demo style Tiny {
  voice lead {
    program 40
    note C6, 1/4
  }
}
"#,
        )
        .unwrap();

        let diagnostic = compilation
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "ML_STYLE_INSTRUMENT_RANGE")
            .unwrap();
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(diagnostic.rule.as_deref(), Some("instrument_range"));
        assert_eq!(diagnostic.style.as_deref(), Some("Tiny"));
        assert!(diagnostic
            .help
            .as_deref()
            .unwrap()
            .contains("audited override"));
        assert_eq!(compilation.ir.tracks[0].events.len(), 1);
    }

    #[test]
    fn non_scale_style_off_suppresses_rule() {
        let ir = compile_source(
            r#"
style Tiny {
  instrument_range: 40 C4 C5
  severity_instrument_range: off
}
score demo style Tiny {
  voice lead {
    program 40
    note C6, 1/4
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

    #[test]
    fn glissando_emits_stepped_chromatic_motion() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    glissando C4 to G4 steps 5, 1/16
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].pitch.to_string(), "C4");
        assert_eq!(events[1].pitch.to_string(), "C#4");
        assert_eq!(events[2].pitch.to_string(), "D#4");
        assert_eq!(events[3].pitch.to_string(), "F4");
        assert_eq!(events[4].pitch.to_string(), "G4");
        assert_eq!(events[0].start_tick, 0);
        assert_eq!(events[1].start_tick, 120);
        assert_eq!(events[4].start_tick, 480);
        assert_eq!(events[0].duration_ticks, 120);
    }

    #[test]
    fn transpose_block_transposes_glissando() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    transpose M2 {
      glissando C4 to G4 steps 3, 1/8
    }
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].pitch.to_string(), "D4");
        assert_eq!(events[1].pitch.to_string(), "F4");
        assert_eq!(events[2].pitch.to_string(), "A4");
        assert_eq!(events[1].start_tick, 240);
    }

    #[test]
    fn glissando_rejects_non_positive_steps() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice lead {
    glissando C4 to G4 steps 0, 1/16
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_THEORY_GLISSANDO");
    }

    #[test]
    fn tremolo_emits_alternating_repeated_notes() {
        let ir = compile_source(
            r#"
score demo {
  voice strings {
    tremolo C4 with G4 repeats 4, 1/32
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].pitch.to_string(), "C4");
        assert_eq!(events[1].pitch.to_string(), "G4");
        assert_eq!(events[2].pitch.to_string(), "C4");
        assert_eq!(events[3].pitch.to_string(), "G4");
        assert_eq!(events[1].start_tick, 60);
        assert_eq!(events[3].start_tick, 180);
        assert_eq!(events[0].duration_ticks, 60);
    }

    #[test]
    fn transpose_block_transposes_tremolo() {
        let ir = compile_source(
            r#"
score demo {
  voice strings {
    transpose M2 {
      tremolo C4 with G4 repeats 2, 1/16
    }
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].pitch.to_string(), "D4");
        assert_eq!(events[1].pitch.to_string(), "A4");
        assert_eq!(events[1].start_tick, 120);
    }

    #[test]
    fn tremolo_rejects_non_positive_repeats() {
        let diagnostics = compile_source(
            r#"
score demo {
  voice strings {
    tremolo C4 with G4 repeats 0, 1/32
  }
}
"#,
        )
        .unwrap_err();

        assert_eq!(diagnostics[0].code, "ML_THEORY_TREMOLO");
    }

    #[test]
    fn strum_emits_staggered_overlapping_notes() {
        let ir = compile_source(
            r#"
score demo {
  voice guitar {
    strum [C4, E4, G4], 1/2 by 1/32
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].pitch.class(), PitchClass::C);
        assert_eq!(events[1].pitch.class(), PitchClass::E);
        assert_eq!(events[2].pitch.class(), PitchClass::G);
        assert_eq!(events[0].start_tick, 0);
        assert_eq!(events[1].start_tick, 60);
        assert_eq!(events[2].start_tick, 120);
        assert_eq!(events[0].duration_ticks, 960);
    }

    #[test]
    fn named_strum_supports_inversion() {
        let ir = compile_source(
            r#"
score demo {
  voice guitar {
    strum C4 dominant7 inv 2, 1/2 by 1/64
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].pitch.to_string(), "G4");
        assert_eq!(events[1].pitch.to_string(), "A#4");
        assert_eq!(events[2].pitch.to_string(), "C5");
        assert_eq!(events[3].pitch.to_string(), "E5");
        assert_eq!(events[1].start_tick, 30);
        assert_eq!(events[3].start_tick, 90);
    }

    #[test]
    fn arpeggio_emits_sequential_notes() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    arpeggio [C4, E4, G4], 1/8
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].pitch.class(), PitchClass::C);
        assert_eq!(events[1].pitch.class(), PitchClass::E);
        assert_eq!(events[2].pitch.class(), PitchClass::G);
        assert_eq!(events[0].start_tick, 0);
        assert_eq!(events[1].start_tick, 240);
        assert_eq!(events[2].start_tick, 480);
        assert_eq!(events[0].duration_ticks, 240);
    }

    #[test]
    fn named_arpeggio_expands_quality() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    arpeggio D3 minor, 1/16
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].pitch.class(), PitchClass::D);
        assert_eq!(events[1].pitch.class(), PitchClass::F);
        assert_eq!(events[2].pitch.class(), PitchClass::A);
        assert_eq!(events[0].start_tick, 0);
        assert_eq!(events[1].start_tick, 120);
        assert_eq!(events[2].start_tick, 240);
    }

    #[test]
    fn named_arpeggio_inversion_reorders_sequence() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    arpeggio C4 dominant7 inv 2, 1/8
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].pitch.to_string(), "G4");
        assert_eq!(events[1].pitch.to_string(), "A#4");
        assert_eq!(events[2].pitch.to_string(), "C5");
        assert_eq!(events[3].pitch.to_string(), "E5");
        assert_eq!(events[3].start_tick, 720);
    }

    #[test]
    fn transpose_block_transposes_arpeggio() {
        let ir = compile_source(
            r#"
score demo {
  voice lead {
    transpose M2 {
      arpeggio [C4, E4, G4], 1/8
    }
  }
}
"#,
        )
        .unwrap();

        let events = &ir.tracks[0].events;
        assert_eq!(events[0].pitch.class(), PitchClass::D);
        assert_eq!(events[1].pitch.class(), PitchClass::Fs);
        assert_eq!(events[2].pitch.class(), PitchClass::A);
        assert_eq!(events[1].start_tick, 240);
        assert_eq!(events[2].start_tick, 480);
    }
}
