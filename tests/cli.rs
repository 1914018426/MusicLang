use std::fs;
use std::process::Command;

fn run_music(args: &[&str]) -> std::process::Output {
    run_music_in(args, env!("CARGO_MANIFEST_DIR"))
}

fn run_music_in(args: &[&str], current_dir: &str) -> std::process::Output {
    let manifest = format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"));
    let mut command = Command::new(env!("CARGO"));
    command.current_dir(current_dir);
    command.args([
        "run",
        "-q",
        "--manifest-path",
        &manifest,
        "-p",
        "musiclang-cli",
        "--bin",
        "music",
        "--",
    ]);
    command.args(args).output().unwrap()
}

#[test]
fn music_version_reports_workspace_version() {
    let output = run_music(&["--version"]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "music 0.1.0"
    );
}

#[test]
fn music_check_accepts_example() {
    let output = run_music(&["check", "examples/minimal.music"]);

    assert!(output.status.success());
}

#[test]
fn music_styles_lists_builtin_registry() {
    let output = run_music(&["styles"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Classical"));
    assert!(stdout.contains("Jazz"));
    assert!(stdout.contains("Minimalist"));
}

#[test]
fn music_theory_lists_scales_domain() {
    let output = run_music(&["theory", "--domain", "scales"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("scales:blues"));
    assert!(stdout.contains("major pentatonic"));
}

#[test]
fn music_theory_lists_dynamics_domain() {
    let output = run_music(&["theory", "--domain", "dynamics"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("dynamics:mf"));
    assert!(stdout.contains("mezzo forte"));
}

#[test]
fn music_theory_lists_ornaments_domain() {
    let output = run_music(&["theory", "--domain", "ornaments"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ornaments:staccato"));
    assert!(stdout.contains("short detached articulation"));
}

#[test]
fn music_new_and_build_create_project_output() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let sandbox = format!("{workspace}/target/music-cli-test-project");
    let project = format!("{sandbox}/demo_song");
    let _ = fs::remove_dir_all(&sandbox);
    fs::create_dir_all(&sandbox).unwrap();

    let new_output = run_music_in(&["new", "demo_song"], &sandbox);
    assert!(new_output.status.success());
    assert!(fs::metadata(format!("{project}/music.toml"))
        .unwrap()
        .is_file());
    assert!(fs::metadata(format!("{project}/src/main.music"))
        .unwrap()
        .is_file());

    let build_output = run_music_in(&["build"], &project);
    assert!(build_output.status.success());
    assert!(fs::read(format!("{project}/build/demo_song.mid"))
        .unwrap()
        .starts_with(b"MThd"));
}

#[test]
fn music_diagnose_detects_style_violation() {
    let output = run_music(&["diagnose", "examples/style_violation.music"]);

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ML_STYLE_SCALE"));
    assert!(stderr.contains("pitch"));
}

#[test]
fn music_diagnose_json_machine_readable() {
    let output = run_music(&["diagnose", "examples/style_violation.music", "--json"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\":\"ML_STYLE_SCALE\""));
    assert!(stdout.contains("\"severity\""));
    assert!(stdout.contains("\"line\""));
}

#[test]
fn music_export_midi_produces_valid_file() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let output_path = format!("{workspace}/target/test-export.mid");
    let _ = fs::remove_file(&output_path);

    let output = run_music(&[
        "export",
        "examples/minimal.music",
        "--format",
        "midi",
        "-o",
        &output_path,
    ]);

    assert!(output.status.success());
    assert!(output.stdout.iter().any(|&b| b == b'\n'));
    let bytes = fs::read(&output_path).unwrap();
    assert!(bytes.starts_with(b"MThd"));
}

#[test]
fn music_export_musicxml_produces_valid_file() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let output_path = format!("{workspace}/target/test-export.musicxml");
    let _ = fs::remove_file(&output_path);

    let output = run_music(&[
        "export",
        "examples/minimal.music",
        "--format",
        "musicxml",
        "-o",
        &output_path,
    ]);

    assert!(output.status.success());
    let xml = fs::read_to_string(&output_path).unwrap();
    assert!(xml.starts_with("<?xml"));
    assert!(xml.contains("<score-partwise"));
}

#[test]
fn music_export_wav_produces_valid_file() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let output_path = format!("{workspace}/target/test-export.wav");
    let _ = fs::remove_file(&output_path);

    let output = run_music(&[
        "export",
        "examples/minimal.music",
        "--format",
        "wav",
        "-o",
        &output_path,
    ]);

    assert!(output.status.success());
    let bytes = fs::read(&output_path).unwrap();
    assert!(bytes.starts_with(b"RIFF"));
    assert_eq!(&bytes[8..12], b"WAVE");
}

#[test]
fn music_ast_prints_parsed_program() {
    let output = run_music(&["ast", "examples/minimal.music"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Program"));
    assert!(stdout.contains("ScoreDecl"));
}

#[test]
fn music_ir_prints_lowered_score() {
    let output = run_music(&["ir", "examples/minimal.music"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ScoreIr"));
    assert!(stdout.contains("tempo_bpm"));
}

#[test]
fn music_export_rejects_unknown_format() {
    let output = run_music(&[
        "export",
        "examples/minimal.music",
        "--format",
        "tracker",
        "-o",
        "target/test-export.tracker",
    ]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported export format `tracker`"));
}

#[test]
fn music_check_reports_error_on_violation() {
    let output = run_music(&["check", "examples/style_violation.music"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ML_STYLE_SCALE"));
}

#[test]
fn music_diagnose_reports_ok_for_valid_override() {
    let output = run_music(&["diagnose", "examples/override.music"]);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "ok");
    assert!(output.stderr.is_empty());
}

#[test]
fn music_check_reports_unknown_function_as_resolve_error() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/unknown-function.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    call missing
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["check", &input_path]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ML_RESOLVE_UNKNOWN_NAME"));
    assert!(stderr.contains("unknown function `missing`"));
}

#[test]
fn music_diagnose_json_reports_recursive_call() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/recursive-call.music");
    fs::write(
        &input_path,
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
    .unwrap();

    let output = run_music(&["diagnose", &input_path, "--json"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"code\":\"ML_RESOLVE_RECURSIVE_CALL\""));
    assert!(stdout.contains("recursive function call `motif`"));
}
