use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "music")]
#[command(version)]
#[command(about = "MusicLang compiler and REPL")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    New {
        name: String,
    },
    Build {
        #[arg(long)]
        manifest: Option<String>,
    },
    Compile {
        input: String,

        #[arg(short, long)]
        output: Option<String>,
    },
    Check {
        input: String,
    },
    Export {
        input: String,

        #[arg(short, long)]
        output: Option<String>,

        #[arg(long, default_value = "midi")]
        format: String,
    },
    Diagnose {
        input: String,

        #[arg(long)]
        json: bool,
    },
    Ast {
        input: String,
    },
    Ir {
        input: String,
    },
    Analyze {
        input: String,

        #[arg(long)]
        json: bool,
    },
    Theory {
        #[arg(long)]
        domain: Option<String>,

        #[arg(long)]
        find: Option<String>,
    },
    Styles,
    Repl,
}

fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::New { name } => new_project(&name),
        Command::Build { manifest } => build_project(manifest.as_deref()),
        Command::Compile { input, output } => compile_file(&input, output.as_deref()),
        Command::Check { input } => check_file(&input),
        Command::Export {
            input,
            output,
            format,
        } => export_file(&input, output.as_deref(), &format),
        Command::Diagnose { input, json } => diagnose_file(&input, json),
        Command::Ast { input } => ast_file(&input),
        Command::Ir { input } => ir_file(&input),
        Command::Analyze { input, json } => analyze_file(&input, json),
        Command::Theory { domain, find } => theory(domain.as_deref(), find.as_deref()),
        Command::Styles => styles(),
        Command::Repl => repl(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectManifest {
    name: String,
    source: String,
    output: String,
    format: String,
}

impl Default for ProjectManifest {
    fn default() -> Self {
        Self {
            name: "music-project".to_string(),
            source: "src/main.music".to_string(),
            output: "build/main.mid".to_string(),
            format: "midi".to_string(),
        }
    }
}

fn new_project(name: &str) -> Result<(), String> {
    let root = Path::new(name);
    if root.exists() {
        return Err(format!("project `{name}` already exists"));
    }
    fs::create_dir(root).map_err(|error| format!("failed to create {name}: {error}"))?;
    fs::create_dir(root.join("src")).map_err(|error| format!("failed to create src: {error}"))?;
    fs::create_dir(root.join("build"))
        .map_err(|error| format!("failed to create build: {error}"))?;
    fs::write(
        root.join("music.toml"),
        format!("name = \"{name}\"\nsource = \"src/main.music\"\noutput = \"build/{name}.mid\"\nformat = \"midi\"\n"),
    )
    .map_err(|error| format!("failed to write music.toml: {error}"))?;
    fs::write(
        root.join("src/main.music"),
        format!("score {name} {{\n  tempo 96\n  meter 4/4\n  key C major\n  voice lead {{\n    program 40\n    note C4, 1/4\n    note C4 + M3, 1/4\n    note G4, 1/2\n  }}\n}}\n"),
    )
    .map_err(|error| format!("failed to write src/main.music: {error}"))?;
    println!("created {name}");
    Ok(())
}

fn build_project(manifest: Option<&str>) -> Result<(), String> {
    let manifest_path = manifest.unwrap_or("music.toml");
    let manifest_text = fs::read_to_string(manifest_path)
        .map_err(|error| format!("failed to read {manifest_path}: {error}"))?;
    let project = parse_manifest(&manifest_text);
    let root = Path::new(manifest_path).parent().unwrap_or(Path::new("."));
    let input = root.join(&project.source);
    let output = root.join(&project.output);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create output dir: {error}"))?;
    }
    export_file_to(&input, Some(&output), &project.format)?;
    println!("built {}", project.name);
    Ok(())
}

fn parse_manifest(source: &str) -> ProjectManifest {
    let mut manifest = ProjectManifest::default();
    for line in source.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').to_string();
        match key.trim() {
            "name" => manifest.name = value,
            "source" => manifest.source = value,
            "output" => manifest.output = value,
            "format" => manifest.format = value,
            _ => {}
        }
    }
    manifest
}

fn compile_file(input: &str, output: Option<&str>) -> Result<(), String> {
    let source =
        fs::read_to_string(input).map_err(|error| format!("failed to read {input}: {error}"))?;
    let ir = musiclang_compiler::compile_source(&source).map_err(format_diagnostics)?;
    let bytes = musiclang_midi::render_midi(&ir)
        .map_err(|error| format!("failed to render MIDI: {error}"))?;
    let output = output.unwrap_or("output.mid");
    fs::write(output, bytes).map_err(|error| format!("failed to write {output}: {error}"))?;
    println!("wrote {output}");
    if !ir.overrides.is_empty() {
        println!("override trace:");
        for trace in ir.overrides {
            let reason = trace.reason.as_deref().unwrap_or("no reason provided");
            println!(
                "  {} at {}:{} ({reason})",
                trace.rule, trace.line, trace.column
            );
        }
    }
    Ok(())
}

fn export_file(input: &str, output: Option<&str>, format: &str) -> Result<(), String> {
    export_file_to(
        Path::new(input),
        output.map(PathBuf::from).as_deref(),
        format,
    )
}

fn export_file_to(input: &Path, output: Option<&Path>, format: &str) -> Result<(), String> {
    let source = fs::read_to_string(input)
        .map_err(|error| format!("failed to read {}: {error}", input.display()))?;
    let ir = musiclang_compiler::compile_source(&source).map_err(format_diagnostics)?;
    let (output, bytes) = match format {
        "midi" | "mid" => (
            output.unwrap_or(Path::new("output.mid")).to_path_buf(),
            musiclang_midi::render_midi(&ir)
                .map_err(|error| format!("failed to render MIDI: {error}"))?,
        ),
        "musicxml" | "xml" => (
            output.unwrap_or(Path::new("output.musicxml")).to_path_buf(),
            musiclang_render::render_musicxml(&ir).into_bytes(),
        ),
        "wav" | "audio" => (
            output.unwrap_or(Path::new("output.wav")).to_path_buf(),
            musiclang_render::render_wav(&ir)
                .map_err(|error| format!("failed to render WAV: {error}"))?,
        ),
        _ => return Err(format!("unsupported export format `{format}`")),
    };
    fs::write(&output, bytes)
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    println!("wrote {}", output.display());
    Ok(())
}

fn check_file(input: &str) -> Result<(), String> {
    let source =
        fs::read_to_string(input).map_err(|error| format!("failed to read {input}: {error}"))?;
    musiclang_compiler::compile_source(&source).map_err(format_diagnostics)?;
    println!("ok");
    Ok(())
}

fn diagnose_file(input: &str, json: bool) -> Result<(), String> {
    let source =
        fs::read_to_string(input).map_err(|error| format!("failed to read {input}: {error}"))?;
    let diagnostics = musiclang_compiler::diagnose_source(&source);
    if json {
        print_diagnostics_json(&diagnostics);
    } else if diagnostics.is_empty() {
        println!("ok");
    } else {
        print_diagnostics(&diagnostics);
    }
    Ok(())
}

fn ast_file(input: &str) -> Result<(), String> {
    let source =
        fs::read_to_string(input).map_err(|error| format!("failed to read {input}: {error}"))?;
    let ast = musiclang_parser::parse_source(&source).map_err(format_diagnostics)?;
    println!("{ast:#?}");
    Ok(())
}

fn ir_file(input: &str) -> Result<(), String> {
    let source =
        fs::read_to_string(input).map_err(|error| format!("failed to read {input}: {error}"))?;
    let ir = musiclang_compiler::compile_source(&source).map_err(format_diagnostics)?;
    println!("{ir:#?}");
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScoreAnalysis {
    title: String,
    composer: Option<String>,
    tempo_bpm: u16,
    meter: Option<musiclang_core::Meter>,
    key: Option<musiclang_core::KeySignature>,
    track_count: usize,
    event_count: usize,
    duration_ticks: u32,
    pitch_min: Option<String>,
    pitch_max: Option<String>,
    override_count: usize,
    diagnostic_count: usize,
    warning_count: usize,
}

fn analyze_file(input: &str, json: bool) -> Result<(), String> {
    let source =
        fs::read_to_string(input).map_err(|error| format!("failed to read {input}: {error}"))?;
    let compilation =
        musiclang_compiler::compile_source_with_diagnostics(&source).map_err(format_diagnostics)?;
    let analysis = analyze_score(&compilation.ir, &compilation.diagnostics);
    if json {
        print_analysis_json(&analysis);
    } else {
        print_analysis(&analysis);
    }
    Ok(())
}

fn analyze_score(
    ir: &musiclang_core::ScoreIr,
    diagnostics: &[musiclang_core::Diagnostic],
) -> ScoreAnalysis {
    let events = ir
        .tracks
        .iter()
        .flat_map(|track| track.events.iter())
        .collect::<Vec<_>>();
    let duration_ticks = events
        .iter()
        .map(|event| event.start_tick + event.duration_ticks)
        .max()
        .unwrap_or(0);
    let pitch_min = events
        .iter()
        .filter_map(|event| event.pitch.midi_number().ok())
        .min()
        .and_then(|midi| {
            events
                .iter()
                .find(|event| event.pitch.midi_number() == Ok(midi))
                .map(|event| event.pitch.to_string())
        });
    let pitch_max = events
        .iter()
        .filter_map(|event| event.pitch.midi_number().ok())
        .max()
        .and_then(|midi| {
            events
                .iter()
                .find(|event| event.pitch.midi_number() == Ok(midi))
                .map(|event| event.pitch.to_string())
        });
    let warning_count = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == musiclang_core::Severity::Warning)
        .count();
    ScoreAnalysis {
        title: ir.title.clone(),
        composer: ir.composer.clone(),
        tempo_bpm: ir.tempo_bpm,
        meter: ir.meter,
        key: ir.key,
        track_count: ir.tracks.len(),
        event_count: events.len(),
        duration_ticks,
        pitch_min,
        pitch_max,
        override_count: ir.overrides.len(),
        diagnostic_count: diagnostics.len(),
        warning_count,
    }
}

fn print_analysis(analysis: &ScoreAnalysis) {
    println!("title: {}", analysis.title);
    if let Some(composer) = &analysis.composer {
        println!("composer: {composer}");
    }
    println!("tempo: {} bpm", analysis.tempo_bpm);
    if let Some(meter) = analysis.meter {
        println!("meter: {}/{}", meter.numerator, meter.denominator);
    }
    if let Some(key) = analysis.key {
        println!("key: {}", format_key_signature(key));
    }
    println!("tracks: {}", analysis.track_count);
    println!("events: {}", analysis.event_count);
    println!("duration_ticks: {}", analysis.duration_ticks);
    if let (Some(low), Some(high)) = (&analysis.pitch_min, &analysis.pitch_max) {
        println!("pitch_range: {low}..{high}");
    }
    println!("overrides: {}", analysis.override_count);
    println!("diagnostics: {}", analysis.diagnostic_count);
    println!("warnings: {}", analysis.warning_count);
}

fn print_analysis_json(analysis: &ScoreAnalysis) {
    print!(
        "{{\"title\":\"{}\",\"composer\":{},\"tempo_bpm\":{},\"meter\":{},\"key\":{},\"track_count\":{},\"event_count\":{},\"duration_ticks\":{},\"pitch_min\":{},\"pitch_max\":{},\"override_count\":{},\"diagnostic_count\":{},\"warning_count\":{}}}",
        json_escape(&analysis.title),
        json_option(analysis.composer.as_deref()),
        analysis.tempo_bpm,
        json_meter(analysis.meter),
        json_key_signature(analysis.key),
        analysis.track_count,
        analysis.event_count,
        analysis.duration_ticks,
        json_option(analysis.pitch_min.as_deref()),
        json_option(analysis.pitch_max.as_deref()),
        analysis.override_count,
        analysis.diagnostic_count,
        analysis.warning_count,
    );
    println!();
}

fn json_meter(meter: Option<musiclang_core::Meter>) -> String {
    meter
        .map(|meter| {
            format!(
                "{{\"numerator\":{},\"denominator\":{}}}",
                meter.numerator, meter.denominator
            )
        })
        .unwrap_or_else(|| "null".to_string())
}

fn json_key_signature(key: Option<musiclang_core::KeySignature>) -> String {
    key.map(|key| {
        format!(
            "{{\"tonic\":\"{}\",\"mode\":\"{}\",\"fifths\":{}}}",
            key_signature_tonic(key),
            key_signature_mode(key),
            key.fifths
        )
    })
    .unwrap_or_else(|| "null".to_string())
}

fn format_key_signature(key: musiclang_core::KeySignature) -> String {
    format!("{} {}", key_signature_tonic(key), key_signature_mode(key))
}

fn key_signature_mode(key: musiclang_core::KeySignature) -> &'static str {
    if key.is_minor {
        "minor"
    } else {
        "major"
    }
}

fn key_signature_tonic(key: musiclang_core::KeySignature) -> &'static str {
    match (key.fifths, key.is_minor) {
        (-7, false) => "Cb",
        (-6, false) => "Gb",
        (-5, false) => "Db",
        (-4, false) => "Ab",
        (-3, false) => "Eb",
        (-2, false) => "Bb",
        (-1, false) => "F",
        (0, false) => "C",
        (1, false) => "G",
        (2, false) => "D",
        (3, false) => "A",
        (4, false) => "E",
        (5, false) => "B",
        (6, false) => "F#",
        (7, false) => "C#",
        (-7, true) => "Ab",
        (-6, true) => "Eb",
        (-5, true) => "Bb",
        (-4, true) => "F",
        (-3, true) => "C",
        (-2, true) => "G",
        (-1, true) => "D",
        (0, true) => "A",
        (1, true) => "E",
        (2, true) => "B",
        (3, true) => "F#",
        (4, true) => "C#",
        (5, true) => "G#",
        (6, true) => "D#",
        (7, true) => "A#",
        _ => "unknown",
    }
}

fn theory(domain: Option<&str>, find: Option<&str>) -> Result<(), String> {
    let catalog = musiclang_core::theory_catalog();
    if let Some(id) = find {
        let (domain, entry) = catalog
            .find(id)
            .ok_or_else(|| format!("unknown theory entry `{id}`"))?;
        print_theory_entry(domain, entry);
        return Ok(());
    }
    if let Some(domain) = domain {
        let domain = domain
            .parse::<musiclang_core::TheoryDomain>()
            .map_err(|error| error.to_string())?;
        for entry in catalog.entries(domain) {
            print_theory_entry(domain, entry);
        }
        return Ok(());
    }
    for domain in musiclang_core::TheoryCatalog::domains() {
        println!("{domain}");
        for entry in catalog.entries(*domain) {
            println!("  {}: {}", entry.id, entry.name);
        }
    }
    Ok(())
}

fn print_theory_entry(domain: musiclang_core::TheoryDomain, entry: &musiclang_core::TheoryEntry) {
    println!("{}:{}", domain, entry.id);
    println!("  name: {}", entry.name);
    println!("  description: {}", entry.description);
    println!("  pattern: {}", entry.pattern.join(" "));
}

fn styles() -> Result<(), String> {
    for style in musiclang_core::BUILT_IN_STYLES {
        println!("{}: {}", style.id, style.name);
        println!("  {}", style.description);
    }
    Ok(())
}

fn repl() -> Result<(), String> {
    println!("MusicLang REPL. Commands: :help, :load <path>, :reset, :diagnose, :export <path>, :show source, :show ir, :quit");
    let mut buffer = String::new();
    loop {
        print!("> ");
        io::stdout().flush().map_err(|error| error.to_string())?;
        let mut line = String::new();
        let bytes = io::stdin()
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        if bytes == 0 {
            break;
        }
        let trimmed = line.trim();
        match trimmed {
            ":help" => println!("Enter MusicLang source. Use :load path.music, :diagnose, :export path.mid, :show source, :show ir, :reset."),
            ":reset" => {
                buffer.clear();
                println!("reset");
            }
            ":diagnose" => {
                let diagnostics = musiclang_compiler::diagnose_source(&buffer);
                if diagnostics.is_empty() {
                    println!("ok");
                } else {
                    print_diagnostics(&diagnostics);
                }
            }
            ":show source" => print!("{buffer}"),
            ":show ir" => {
                let ir = musiclang_compiler::compile_source(&buffer).map_err(format_diagnostics)?;
                println!("{ir:#?}");
            }
            ":quit" | ":exit" => break,
            command if command.starts_with(":load ") => {
                let path = command.trim_start_matches(":load ").trim();
                buffer = fs::read_to_string(path)
                    .map_err(|error| format!("failed to read {path}: {error}"))?;
                println!("loaded {path}");
            }
            command if command.starts_with(":style ") => {
                let name = command.trim_start_matches(":style ").trim();
                buffer = format!("style {name}\n{buffer}");
                println!("style {name}");
            }
            command if command.starts_with(":export ") => {
                let path = command.trim_start_matches(":export ").trim();
                let ir = musiclang_compiler::compile_source(&buffer).map_err(format_diagnostics)?;
                let bytes = musiclang_midi::render_midi(&ir).map_err(|error| error.to_string())?;
                fs::write(path, bytes).map_err(|error| format!("failed to write {path}: {error}"))?;
                println!("wrote {path}");
            }
            command if command.starts_with(':') => println!("unknown command {command}"),
            _ => buffer.push_str(&line),
        }
    }
    Ok(())
}

fn print_diagnostics(diagnostics: &[musiclang_core::Diagnostic]) {
    for diagnostic in diagnostics {
        eprint!("{diagnostic}");
    }
}

fn print_diagnostics_json(diagnostics: &[musiclang_core::Diagnostic]) {
    print!("[");
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index > 0 {
            print!(",");
        }
        print!(
            "{{\"code\":\"{}\",\"severity\":\"{}\",\"message\":\"{}\",\"line\":{},\"column\":{},\"span\":{},\"labels\":{},\"related\":{},\"rule\":{},\"style\":{},\"help\":{}}}",
            json_escape(&diagnostic.code),
            diagnostic.severity,
            json_escape(&diagnostic.message),
            diagnostic.line,
            diagnostic.column,
            json_span(diagnostic.span),
            json_labels(&diagnostic.labels),
            json_related(&diagnostic.related),
            json_option(diagnostic.rule.as_deref()),
            json_option(diagnostic.style.as_deref()),
            json_option(diagnostic.help.as_deref())
        );
    }
    println!("]");
}

fn json_labels(labels: &[musiclang_core::DiagnosticLabel]) -> String {
    let values = labels
        .iter()
        .map(|label| {
            format!(
                "{{\"span\":{},\"message\":\"{}\"}}",
                json_span(Some(label.span)),
                json_escape(&label.message)
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", values.join(","))
}

fn json_related(related: &[musiclang_core::DiagnosticRelated]) -> String {
    let values = related
        .iter()
        .map(|related| {
            format!(
                "{{\"span\":{},\"message\":\"{}\"}}",
                json_span(Some(related.span)),
                json_escape(&related.message)
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", values.join(","))
}

fn json_span(span: Option<musiclang_core::Span>) -> String {
    span.map(|span| {
        format!(
            "{{\"source_id\":{},\"start\":{},\"end\":{},\"line\":{},\"column\":{}}}",
            span.source_id.0, span.start, span.end, span.line, span.column
        )
    })
    .unwrap_or_else(|| "null".to_string())
}

fn json_option(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", json_escape(value)))
        .unwrap_or_else(|| "null".to_string())
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn format_diagnostics(diagnostics: Vec<musiclang_core::Diagnostic>) -> String {
    diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}
