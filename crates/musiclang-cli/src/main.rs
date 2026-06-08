use std::collections::BTreeMap;
use std::collections::BTreeSet;
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

        #[arg(long)]
        strict: bool,
    },
    Export {
        input: String,

        #[arg(short, long)]
        output: Option<String>,

        #[arg(long, default_value = "midi")]
        format: String,

        #[arg(long)]
        strict: bool,
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

        #[arg(long)]
        strict: bool,
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
        Command::Check { input, strict } => check_file(&input, strict),
        Command::Export {
            input,
            output,
            format,
            strict,
        } => export_file(&input, output.as_deref(), &format, strict),
        Command::Diagnose { input, json } => diagnose_file(&input, json),
        Command::Ast { input } => ast_file(&input),
        Command::Ir { input } => ir_file(&input),
        Command::Analyze {
            input,
            json,
            strict,
        } => analyze_file(&input, json, strict),
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
        format!("score {name} {{\n  tempo 96\n  meter 4/4\n  key C major\n  voice lead {{\n    instrument violin\n    channel 0\n    volume 96\n    pan 64\n    note C4, 1/4\n    note C4 + M3, 1/4\n    note G4, 1/2\n  }}\n  voice drums {{\n    instrument drums\n    channel 9\n    drum kick, 1/4\n    drum snare, 1/4\n  }}\n}}\n"),
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
    export_file_to(&input, Some(&output), &project.format, false)?;
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

fn export_file(
    input: &str,
    output: Option<&str>,
    format: &str,
    strict: bool,
) -> Result<(), String> {
    export_file_to(
        Path::new(input),
        output.map(PathBuf::from).as_deref(),
        format,
        strict,
    )
}

fn export_file_to(
    input: &Path,
    output: Option<&Path>,
    format: &str,
    strict: bool,
) -> Result<(), String> {
    let source = fs::read_to_string(input)
        .map_err(|error| format!("failed to read {}: {error}", input.display()))?;
    let ir = if strict {
        let compilation = musiclang_compiler::compile_source_with_diagnostics(&source)
            .map_err(format_diagnostics)?;
        if !compilation.diagnostics.is_empty() {
            return Err(format_diagnostics(compilation.diagnostics));
        }
        compilation.ir
    } else {
        musiclang_compiler::compile_source(&source).map_err(format_diagnostics)?
    };
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

fn check_file(input: &str, strict: bool) -> Result<(), String> {
    let source =
        fs::read_to_string(input).map_err(|error| format!("failed to read {input}: {error}"))?;
    if strict {
        let compilation = musiclang_compiler::compile_source_with_diagnostics(&source)
            .map_err(format_diagnostics)?;
        if !compilation.diagnostics.is_empty() {
            return Err(format_diagnostics(compilation.diagnostics));
        }
    } else {
        musiclang_compiler::compile_source(&source).map_err(format_diagnostics)?;
    }
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
struct TrackAnalysis {
    name: String,
    event_count: usize,
    density_per_bar: u32,
    pitch_min: Option<String>,
    pitch_max: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SonorityAnalysis {
    tick: u32,
    pitch_classes: Vec<String>,
    root: Option<String>,
    quality: Option<String>,
    roman: Option<String>,
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
    bar_ticks: u32,
    duration_bars: u32,
    density_per_bar: u32,
    repeated_bar_count: u32,
    repeated_bar_ratio_percent: u32,
    longest_repeated_bar_run: u32,
    max_simultaneous_events: usize,
    texture: String,
    pitch_min: Option<String>,
    pitch_max: Option<String>,
    pitch_classes: Vec<String>,
    roman_roots: Vec<String>,
    sonorities: Vec<SonorityAnalysis>,
    tracks: Vec<TrackAnalysis>,
    override_count: usize,
    diagnostic_count: usize,
    warning_count: usize,
}

fn analyze_file(input: &str, json: bool, strict: bool) -> Result<(), String> {
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
    if strict {
        enforce_analysis_quality(&analysis)?;
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
    let meter = ir.meter.unwrap_or_default();
    let bar_ticks =
        ir.ticks_per_quarter * u32::from(meter.numerator) * 4 / u32::from(meter.denominator);
    let duration_bars = duration_ticks.div_ceil(bar_ticks.max(1));
    let score_density_per_bar = density_per_bar(events.len(), duration_bars);
    let repeated_bars = analyze_repeated_bars(ir, bar_ticks.max(1), duration_bars);
    let max_simultaneous_events = max_simultaneous_events(&events);
    let texture = classify_texture(ir.tracks.len(), max_simultaneous_events).to_string();
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
    let pitch_classes = events
        .iter()
        .map(|event| event.pitch.class().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let roman_roots = ir
        .key
        .map(|key| {
            events
                .iter()
                .map(|event| roman_degree(event.pitch.class(), key))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let sonorities = analyze_sonorities(&events, ir.key);
    let tracks = ir
        .tracks
        .iter()
        .map(|track| {
            let track_events = track.events.iter().collect::<Vec<_>>();
            let pitch_min = track_events
                .iter()
                .filter_map(|event| event.pitch.midi_number().ok())
                .min()
                .and_then(|midi| {
                    track_events
                        .iter()
                        .find(|event| event.pitch.midi_number() == Ok(midi))
                        .map(|event| event.pitch.to_string())
                });
            let pitch_max = track_events
                .iter()
                .filter_map(|event| event.pitch.midi_number().ok())
                .max()
                .and_then(|midi| {
                    track_events
                        .iter()
                        .find(|event| event.pitch.midi_number() == Ok(midi))
                        .map(|event| event.pitch.to_string())
                });
            TrackAnalysis {
                name: track.name.clone(),
                event_count: track.events.len(),
                density_per_bar: density_per_bar(track.events.len(), duration_bars),
                pitch_min,
                pitch_max,
            }
        })
        .collect();
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
        bar_ticks,
        duration_bars,
        density_per_bar: score_density_per_bar,
        repeated_bar_count: repeated_bars.repeated_count,
        repeated_bar_ratio_percent: repeated_bars.ratio_percent,
        longest_repeated_bar_run: repeated_bars.longest_run,
        max_simultaneous_events,
        texture,
        pitch_min,
        pitch_max,
        pitch_classes,
        roman_roots,
        sonorities,
        tracks,
        override_count: ir.overrides.len(),
        diagnostic_count: diagnostics.len(),
        warning_count,
    }
}

fn enforce_analysis_quality(analysis: &ScoreAnalysis) -> Result<(), String> {
    let mut failures = Vec::new();
    if analysis.diagnostic_count > 0 {
        failures.push(format!(
            "diagnostics {} exceeds 0",
            analysis.diagnostic_count
        ));
    }
    if analysis.repeated_bar_ratio_percent > 50 {
        failures.push(format!(
            "repeated_bar_ratio_percent {} exceeds 50",
            analysis.repeated_bar_ratio_percent
        ));
    }
    if analysis.longest_repeated_bar_run > 4 {
        failures.push(format!(
            "longest_repeated_bar_run {} exceeds 4",
            analysis.longest_repeated_bar_run
        ));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "analysis quality gate failed: {}",
            failures.join("; ")
        ))
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
    println!("bar_ticks: {}", analysis.bar_ticks);
    println!("duration_bars: {}", analysis.duration_bars);
    println!("density_per_bar: {}", analysis.density_per_bar);
    println!("repeated_bar_count: {}", analysis.repeated_bar_count);
    println!(
        "repeated_bar_ratio_percent: {}",
        analysis.repeated_bar_ratio_percent
    );
    println!(
        "longest_repeated_bar_run: {}",
        analysis.longest_repeated_bar_run
    );
    println!(
        "max_simultaneous_events: {}",
        analysis.max_simultaneous_events
    );
    println!("texture: {}", analysis.texture);
    if let (Some(low), Some(high)) = (&analysis.pitch_min, &analysis.pitch_max) {
        println!("pitch_range: {low}..{high}");
    }
    if !analysis.pitch_classes.is_empty() {
        println!("pitch_classes: {}", analysis.pitch_classes.join(","));
    }
    if !analysis.roman_roots.is_empty() {
        println!("roman_roots: {}", analysis.roman_roots.join(","));
    }
    for sonority in &analysis.sonorities {
        println!(
            "sonority tick={}: pcs={}, root={}, quality={}, roman={}",
            sonority.tick,
            sonority.pitch_classes.join(","),
            sonority.root.as_deref().unwrap_or("unknown"),
            sonority.quality.as_deref().unwrap_or("unknown"),
            sonority.roman.as_deref().unwrap_or("unknown")
        );
    }
    for track in &analysis.tracks {
        if let (Some(low), Some(high)) = (&track.pitch_min, &track.pitch_max) {
            println!(
                "track {}: events={}, density_per_bar={}, range={}..{}",
                track.name, track.event_count, track.density_per_bar, low, high
            );
        } else {
            println!(
                "track {}: events={}, density_per_bar={}, range=none",
                track.name, track.event_count, track.density_per_bar
            );
        }
    }
    println!("overrides: {}", analysis.override_count);
    println!("diagnostics: {}", analysis.diagnostic_count);
    println!("warnings: {}", analysis.warning_count);
}

fn print_analysis_json(analysis: &ScoreAnalysis) {
    print!(
        "{{\"title\":\"{}\",\"composer\":{},\"tempo_bpm\":{},\"meter\":{},\"key\":{},\"track_count\":{},\"event_count\":{},\"duration_ticks\":{},\"bar_ticks\":{},\"duration_bars\":{},\"density_per_bar\":{},\"repeated_bar_count\":{},\"repeated_bar_ratio_percent\":{},\"longest_repeated_bar_run\":{},\"max_simultaneous_events\":{},\"texture\":\"{}\",\"pitch_min\":{},\"pitch_max\":{},\"pitch_classes\":{},\"roman_roots\":{},\"sonorities\":{},\"tracks\":{},\"override_count\":{},\"diagnostic_count\":{},\"warning_count\":{}}}",
        json_escape(&analysis.title),
        json_option(analysis.composer.as_deref()),
        analysis.tempo_bpm,
        json_meter(analysis.meter),
        json_key_signature(analysis.key),
        analysis.track_count,
        analysis.event_count,
        analysis.duration_ticks,
        analysis.bar_ticks,
        analysis.duration_bars,
        analysis.density_per_bar,
        analysis.repeated_bar_count,
        analysis.repeated_bar_ratio_percent,
        analysis.longest_repeated_bar_run,
        analysis.max_simultaneous_events,
        json_escape(&analysis.texture),
        json_option(analysis.pitch_min.as_deref()),
        json_option(analysis.pitch_max.as_deref()),
        json_string_array(&analysis.pitch_classes),
        json_string_array(&analysis.roman_roots),
        json_sonority_analysis(&analysis.sonorities),
        json_track_analysis(&analysis.tracks),
        analysis.override_count,
        analysis.diagnostic_count,
        analysis.warning_count,
    );
    println!();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RepeatedBarAnalysis {
    repeated_count: u32,
    ratio_percent: u32,
    longest_run: u32,
}

fn analyze_repeated_bars(
    ir: &musiclang_core::ScoreIr,
    bar_ticks: u32,
    duration_bars: u32,
) -> RepeatedBarAnalysis {
    let mut signatures = Vec::new();
    for bar in 0..duration_bars {
        let bar_start = bar * bar_ticks;
        let bar_end = bar_start + bar_ticks;
        let mut entries = Vec::new();
        for track in &ir.tracks {
            for event in &track.events {
                if event.start_tick >= bar_start && event.start_tick < bar_end {
                    entries.push(format!(
                        "{}:{}:{}:{}",
                        track.name,
                        event.start_tick - bar_start,
                        event.duration_ticks,
                        event.pitch.midi_number().unwrap_or(0)
                    ));
                }
            }
        }
        entries.sort();
        signatures.push(entries.join("|"));
    }

    let repeated_count = signatures
        .iter()
        .fold(BTreeMap::<&String, u32>::new(), |mut counts, signature| {
            *counts.entry(signature).or_default() += 1;
            counts
        })
        .values()
        .map(|count| count.saturating_sub(1))
        .sum::<u32>();
    let mut longest_run = 0;
    let mut current_run = 0;
    let mut previous = None;
    for signature in &signatures {
        if Some(signature) == previous {
            current_run += 1;
        } else {
            current_run = 1;
            previous = Some(signature);
        }
        longest_run = longest_run.max(current_run);
    }
    RepeatedBarAnalysis {
        repeated_count,
        ratio_percent: repeated_count * 100 / duration_bars.max(1),
        longest_run,
    }
}

fn analyze_sonorities(
    events: &[&musiclang_core::NoteEventIr],
    key: Option<musiclang_core::KeySignature>,
) -> Vec<SonorityAnalysis> {
    let ticks = events
        .iter()
        .map(|event| event.start_tick)
        .collect::<BTreeSet<_>>();
    ticks
        .into_iter()
        .filter_map(|tick| {
            let pitch_classes = events
                .iter()
                .filter(|event| event.start_tick == tick)
                .map(|event| event.pitch.class())
                .collect::<BTreeSet<_>>();
            if pitch_classes.len() < 2 {
                return None;
            }
            let pitch_class_names = pitch_classes
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let chord = infer_triad(&pitch_classes.iter().copied().collect::<Vec<_>>());
            let roman = chord
                .as_ref()
                .and_then(|(root, quality)| key.map(|key| roman_chord(*root, quality, key)));
            Some(SonorityAnalysis {
                tick,
                pitch_classes: pitch_class_names,
                root: chord.as_ref().map(|(root, _)| root.to_string()),
                quality: chord.as_ref().map(|(_, quality)| quality.to_string()),
                roman,
            })
        })
        .collect()
}

fn infer_triad(
    pitch_classes: &[musiclang_core::PitchClass],
) -> Option<(musiclang_core::PitchClass, &'static str)> {
    for root in pitch_classes {
        let intervals = pitch_classes
            .iter()
            .map(|pitch_class| (pitch_class.semitone() - root.semitone()).rem_euclid(12))
            .collect::<BTreeSet<_>>();
        let quality = if intervals.contains(&0) && intervals.contains(&4) && intervals.contains(&7)
        {
            Some("major")
        } else if intervals.contains(&0) && intervals.contains(&3) && intervals.contains(&7) {
            Some("minor")
        } else if intervals.contains(&0) && intervals.contains(&3) && intervals.contains(&6) {
            Some("diminished")
        } else if intervals.contains(&0) && intervals.contains(&4) && intervals.contains(&8) {
            Some("augmented")
        } else {
            None
        };
        if let Some(quality) = quality {
            return Some((*root, quality));
        }
    }
    None
}

fn roman_chord(
    root: musiclang_core::PitchClass,
    quality: &str,
    key: musiclang_core::KeySignature,
) -> String {
    let degree = roman_degree(root, key);
    match quality {
        "major" => degree.to_ascii_uppercase(),
        "minor" => degree.to_ascii_lowercase(),
        "diminished" => format!("{}°", degree.to_ascii_lowercase()),
        "augmented" => format!("{}+", degree.to_ascii_uppercase()),
        _ => degree,
    }
}

fn density_per_bar(event_count: usize, duration_bars: u32) -> u32 {
    if duration_bars == 0 {
        0
    } else {
        (event_count as u32).div_ceil(duration_bars)
    }
}

fn max_simultaneous_events(events: &[&musiclang_core::NoteEventIr]) -> usize {
    events
        .iter()
        .map(|event| {
            events
                .iter()
                .filter(|candidate| candidate.start_tick == event.start_tick)
                .count()
        })
        .max()
        .unwrap_or(0)
}

fn classify_texture(track_count: usize, max_simultaneous_events: usize) -> &'static str {
    match (track_count, max_simultaneous_events) {
        (0, _) => "empty",
        (1, 0 | 1) => "monophonic",
        (1, _) => "chordal",
        (_, 0 | 1) => "heterophonic",
        (_, 2) => "polyphonic",
        _ => "dense_polyphonic",
    }
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

fn json_string_array(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| format!("\"{}\"", json_escape(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

fn json_sonority_analysis(sonorities: &[SonorityAnalysis]) -> String {
    let sonorities = sonorities
        .iter()
        .map(|sonority| {
            format!(
                "{{\"tick\":{},\"pitch_classes\":{},\"root\":{},\"quality\":{},\"roman\":{}}}",
                sonority.tick,
                json_string_array(&sonority.pitch_classes),
                json_option(sonority.root.as_deref()),
                json_option(sonority.quality.as_deref()),
                json_option(sonority.roman.as_deref())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{sonorities}]")
}

fn json_track_analysis(tracks: &[TrackAnalysis]) -> String {
    let tracks = tracks
        .iter()
        .map(|track| {
            format!(
                "{{\"name\":\"{}\",\"event_count\":{},\"density_per_bar\":{},\"pitch_min\":{},\"pitch_max\":{}}}",
                json_escape(&track.name),
                track.event_count,
                track.density_per_bar,
                json_option(track.pitch_min.as_deref()),
                json_option(track.pitch_max.as_deref())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{tracks}]")
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

fn key_signature_tonic_semitone(key: musiclang_core::KeySignature) -> i16 {
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

fn roman_degree(
    pitch_class: musiclang_core::PitchClass,
    key: musiclang_core::KeySignature,
) -> String {
    let offset = (pitch_class.semitone() - key_signature_tonic_semitone(key)).rem_euclid(12);
    let label = match offset {
        0 => "I",
        1 => "bII",
        2 => "II",
        3 => "bIII",
        4 => "III",
        5 => "IV",
        6 => "#IV",
        7 => "V",
        8 => "bVI",
        9 => "VI",
        10 => "bVII",
        _ => "VII",
    };
    if key.is_minor {
        label.to_ascii_lowercase()
    } else {
        label.to_string()
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
