use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};

fn run_music(args: &[&str]) -> std::process::Output {
    run_music_in(args, env!("CARGO_MANIFEST_DIR"))
}

fn run_music_in(args: &[&str], current_dir: &str) -> std::process::Output {
    music_command(args, current_dir).output().unwrap()
}

fn run_music_with_stdin(args: &[&str], input: &str) -> std::process::Output {
    let mut child = music_command(args, env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn music_command(args: &[&str], current_dir: &str) -> Command {
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
    command.args(args);
    command
}

fn assert_valid_midi(bytes: &[u8]) {
    assert!(bytes.len() > 22);
    assert_eq!(&bytes[0..4], b"MThd");
    assert_eq!(u32::from_be_bytes(bytes[4..8].try_into().unwrap()), 6);
    assert!(u16::from_be_bytes(bytes[10..12].try_into().unwrap()) >= 1);
    assert!(u16::from_be_bytes(bytes[12..14].try_into().unwrap()) > 0);
    assert!(bytes.windows(4).any(|chunk| chunk == b"MTrk"));
    Smf::parse(bytes).unwrap();
}

fn assert_valid_wav(bytes: &[u8]) {
    assert!(bytes.len() > 44);
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(u16::from_le_bytes(bytes[20..22].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(bytes[22..24].try_into().unwrap()), 2);
    assert_eq!(u16::from_le_bytes(bytes[34..36].try_into().unwrap()), 16);
    assert!(bytes.windows(4).any(|chunk| chunk == b"data"));
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
fn music_check_strict_rejects_warning_only_diagnostics() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/check-strict-warning.music");
    fs::write(
        &input_path,
        r#"
style WarningScale {
  scale: C major
  severity_scale: warning
}

score warning_only style WarningScale {
  key C major
  voice lead {
    note C4, 1/4
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();

    assert!(run_music(&["check", &input_path]).status.success());
    let strict_output = run_music(&["check", &input_path, "--strict"]);

    assert!(!strict_output.status.success());
    let stderr = String::from_utf8_lossy(&strict_output.stderr);
    assert!(stderr.contains("ML_STYLE_SCALE"));
    assert!(stderr.contains("warning"));
}

#[test]
fn music_check_strict_rejects_disabled_style_rules() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/check-strict-off.music");
    fs::write(
        &input_path,
        r#"
style HiddenScale {
  scale: C major
  severity_scale: off
}

score hidden style HiddenScale {
  key C major
  voice lead {
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();

    let strict_output = run_music(&["check", &input_path, "--strict"]);

    assert!(!strict_output.status.success());
    let stderr = String::from_utf8_lossy(&strict_output.stderr);
    assert!(stderr.contains("strict quality gate rejects disabled style rule"));
    assert!(stderr.contains("severity_scale"));
}

#[test]
fn music_check_strict_rejects_override_suppression() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/check-strict-override.music");
    fs::write(
        &input_path,
        r#"
style StrictScale {
  scale: C major
}

score hidden style StrictScale {
  key C major
  voice lead {
    override scale allow reason "hide bad chromatic note" {
      note F#4, 1/4
    }
  }
}
"#,
    )
    .unwrap();

    let strict_output = run_music(&["check", &input_path, "--strict"]);

    assert!(!strict_output.status.success());
    let stderr = String::from_utf8_lossy(&strict_output.stderr);
    assert!(stderr.contains("strict quality gate rejects override suppression"));
}

#[test]
fn music_check_strict_rejects_disabled_style_rules_with_trailing_comment() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/check-strict-off-comment.music");
    fs::write(
        &input_path,
        r#"
style HiddenScale {
  scale: C major
  severity_scale: off // old cleanup bypass
}

score hidden style HiddenScale {
  key C major
  voice lead {
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();

    let strict_output = run_music(&["check", &input_path, "--strict"]);

    assert!(!strict_output.status.success());
    let stderr = String::from_utf8_lossy(&strict_output.stderr);
    assert!(stderr.contains("strict quality gate rejects disabled style rule"));
    assert!(stderr.contains("severity_scale"));
}

#[test]
fn music_check_strict_ignores_suppression_words_in_comments() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/check-strict-commented-suppression.music");
    fs::write(
        &input_path,
        r#"
// override scale allow reason "old cleanup" {
style CleanScale {
  scale: C major
  // severity_scale: off
}

score clean style CleanScale {
  key C major
  voice lead {
    note C4, 1/4
  }
}
"#,
    )
    .unwrap();

    let strict_output = run_music(&["check", &input_path, "--strict"]);

    assert!(strict_output.status.success());
}

#[test]
fn music_formats_lists_export_backends() {
    let output = run_music(&["formats"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("midi"));
    assert!(stdout.contains("musicxml"));
    assert!(stdout.contains("wav"));
    assert!(stdout.contains("aliases"));
}

#[test]
fn music_formats_json_is_machine_readable() {
    let output = run_music(&["formats", "--json"]);

    assert!(output.status.success());
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let formats = payload["formats"].as_array().unwrap();
    let midi = formats
        .iter()
        .find(|format| format["id"] == "midi")
        .unwrap();
    assert!(midi["aliases"]
        .as_array()
        .unwrap()
        .contains(&serde_json::Value::String("mid".to_string())));
    assert!(midi["description"].is_string());
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
fn music_styles_json_is_machine_readable() {
    let output = run_music(&["styles", "--json"]);

    assert!(output.status.success());
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let styles = payload["styles"].as_array().unwrap();
    let jazz = styles.iter().find(|style| style["id"] == "Jazz").unwrap();
    assert!(jazz["name"].is_string());
    assert!(jazz["description"].is_string());
}

#[test]
fn music_idioms_lists_phrase_concepts() {
    let output = run_music(&["idioms"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("phrase_concept"));
    assert!(stdout.contains("periodic_phrase"));
    assert!(stdout.contains("motivic_development"));
}

#[test]
fn music_idioms_json_is_machine_readable() {
    let output = run_music(&["idioms", "--json"]);

    assert!(output.status.success());
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let rules = payload["rules"].as_array().unwrap();
    let phrase = rules
        .iter()
        .find(|rule| rule["rule"] == "phrase_concept")
        .unwrap();
    let entries = phrase["entries"].as_array().unwrap();
    assert!(entries.contains(&serde_json::Value::String("periodic_phrase".to_string())));
    assert!(entries.contains(&serde_json::Value::String(
        "motivic_development".to_string()
    )));
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
fn music_theory_domain_json_is_machine_readable() {
    let output = run_music(&["theory", "--domain", "scales", "--json"]);

    assert!(output.status.success());
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["domain"], "scales");
    let entries = payload["entries"].as_array().unwrap();
    let blues = entries.iter().find(|entry| entry["id"] == "blues").unwrap();
    assert_eq!(blues["domain"], "scales");
    assert_eq!(blues["id"], "blues");
    assert!(blues["name"].is_string());
    assert!(blues["description"].is_string());
    assert!(blues["pattern"].is_array());
}

#[test]
fn music_theory_find_json_is_machine_readable() {
    let output = run_music(&["theory", "--find", "maqam", "--json"]);

    assert!(output.status.success());
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["domain"], "world_traditions");
    assert_eq!(payload["id"], "maqam");
    assert!(payload["name"].is_string());
    assert!(payload["description"].is_string());
    assert!(payload["pattern"].is_array());
}

#[test]
fn music_theory_catalog_json_is_machine_readable() {
    let output = run_music(&["theory", "--json"]);

    assert!(output.status.success());
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let domains = payload["domains"].as_array().unwrap();
    let dynamics = domains
        .iter()
        .find(|domain| domain["domain"] == "dynamics")
        .unwrap();
    assert!(dynamics["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["id"] == "mf"));
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
    let manifest = fs::read_to_string(format!("{project}/music.toml")).unwrap();
    assert!(manifest.contains("strict = false"));
    let source = fs::read_to_string(format!("{project}/src/main.music")).unwrap();
    assert!(source.contains("instrument violin"));
    assert!(source.contains("channel 0"));
    assert!(source.contains("volume 96"));
    assert!(source.contains("pan 64"));
    assert!(source.contains("instrument drums"));
    assert!(source.contains("channel 9"));
    assert!(source.contains("drum kick"));

    let build_output = run_music_in(&["build"], &project);
    assert!(build_output.status.success());
    assert!(fs::read(format!("{project}/build/demo_song.mid"))
        .unwrap()
        .starts_with(b"MThd"));
}

#[test]
fn music_build_uses_explicit_manifest_path() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let sandbox = format!("{workspace}/target/music-cli-manifest-path-project");
    let project = format!("{sandbox}/demo_song");
    let _ = fs::remove_dir_all(&sandbox);
    fs::create_dir_all(format!("{project}/src")).unwrap();
    fs::write(
        format!("{project}/music.toml"),
        "name = \"manifest-path\"\nsource = \"src/main.music\"\noutput = \"build/out.mid\"\nformat = \"midi\"\n",
    )
    .unwrap();
    fs::write(
        format!("{project}/src/main.music"),
        r#"
score manifest_path {
  tempo 96
  meter 4/4
  key C major
  voice lead {
    note C4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music_in(
        &["build", "--manifest", &format!("{project}/music.toml")],
        &sandbox,
    );

    assert!(output.status.success());
    assert!(fs::read(format!("{project}/build/out.mid"))
        .unwrap()
        .starts_with(b"MThd"));
}

#[test]
fn music_build_manifest_accepts_inline_comments() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let project = format!("{workspace}/target/music-cli-manifest-comments-project");
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(format!("{project}/src")).unwrap();
    fs::write(
        format!("{project}/music.toml"),
        "# demo project\nname = \"manifest-comments\" # display name\nsource = \"src/main.music\" # input file\noutput = \"build/out.mid\" # output file\nformat = \"midi\" # renderer\nstrict = false # local draft\n",
    )
    .unwrap();
    fs::write(
        format!("{project}/src/main.music"),
        r#"
score manifest_comments {
  voice lead {
    note C4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music_in(&["build"], &project);

    assert!(output.status.success());
    assert!(fs::read(format!("{project}/build/out.mid"))
        .unwrap()
        .starts_with(b"MThd"));
}

#[test]
fn music_build_honors_manifest_musicxml_format() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let project = format!("{workspace}/target/music-cli-manifest-musicxml-project");
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(format!("{project}/src")).unwrap();
    fs::write(
        format!("{project}/music.toml"),
        "name = \"manifest-musicxml\"\nsource = \"src/main.music\"\noutput = \"build/out.musicxml\"\nformat = \"musicxml\"\n",
    )
    .unwrap();
    fs::write(
        format!("{project}/src/main.music"),
        r#"
score manifest_musicxml {
  title "Manifest MusicXML"
  voice lead {
    note C4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music_in(&["build"], &project);

    assert!(output.status.success());
    let bytes = fs::read(format!("{project}/build/out.musicxml")).unwrap();
    let xml = String::from_utf8(bytes).unwrap();
    assert!(xml.contains("<score-partwise"));
    assert!(xml.contains("Manifest MusicXML"));
}

#[test]
fn music_build_honors_manifest_wav_format() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let project = format!("{workspace}/target/music-cli-manifest-wav-project");
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(format!("{project}/src")).unwrap();
    fs::write(
        format!("{project}/music.toml"),
        "name = \"manifest-wav\"\nsource = \"src/main.music\"\noutput = \"build/out.wav\"\nformat = \"wav\"\n",
    )
    .unwrap();
    fs::write(
        format!("{project}/src/main.music"),
        r#"
score manifest_wav {
  voice lead {
    note C4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music_in(&["build"], &project);

    assert!(output.status.success());
    assert_valid_wav(&fs::read(format!("{project}/build/out.wav")).unwrap());
}

#[test]
fn music_build_rejects_unknown_manifest_format() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let project = format!("{workspace}/target/music-cli-manifest-unknown-format-project");
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(format!("{project}/src")).unwrap();
    fs::write(
        format!("{project}/music.toml"),
        "name = \"manifest-unknown-format\"\nsource = \"src/main.music\"\noutput = \"build/out.tracker\"\nformat = \"tracker\"\n",
    )
    .unwrap();
    fs::write(
        format!("{project}/src/main.music"),
        r#"
score manifest_unknown_format {
  voice lead {
    note C4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music_in(&["build"], &project);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported export format `tracker`"));
    assert!(!std::path::Path::new(&format!("{project}/build/out.tracker")).exists());
}

#[test]
fn music_build_rejects_invalid_manifest_strict_value() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let project = format!("{workspace}/target/music-cli-invalid-strict-project");
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(format!("{project}/src")).unwrap();
    fs::write(
        format!("{project}/music.toml"),
        "name = \"invalid-strict\"\nsource = \"src/main.music\"\noutput = \"build/out.mid\"\nformat = \"midi\"\nstrict = maybe\n",
    )
    .unwrap();
    fs::write(
        format!("{project}/src/main.music"),
        r#"
score invalid_strict {
  voice lead {
    note C4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music_in(&["build"], &project);

    assert!(!output.status.success());
    assert!(!std::path::Path::new(&format!("{project}/build/out.mid")).exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid music.toml strict value"));
    assert!(stderr.contains("expected true or false"));
}

#[test]
fn music_build_strict_rejects_warning_only_diagnostics() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let project = format!("{workspace}/target/music-cli-strict-build-project");
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(format!("{project}/src")).unwrap();
    fs::write(
        format!("{project}/music.toml"),
        "name = \"strict-build\"\nsource = \"src/main.music\"\noutput = \"build/out.mid\"\nformat = \"midi\"\n",
    )
    .unwrap();
    fs::write(
        format!("{project}/src/main.music"),
        r#"
style WarningScale {
  scale: C major
  severity_scale: warning
}

score warning_only style WarningScale {
  key C major
  voice lead {
    note C4, 1/4
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();

    let build_output = run_music_in(&["build"], &project);
    assert!(build_output.status.success());
    assert!(fs::read(format!("{project}/build/out.mid"))
        .unwrap()
        .starts_with(b"MThd"));
    fs::remove_file(format!("{project}/build/out.mid")).unwrap();

    let strict_output = run_music_in(&["build", "--strict"], &project);

    assert!(!strict_output.status.success());
    assert!(!std::path::Path::new(&format!("{project}/build/out.mid")).exists());
    let stderr = String::from_utf8_lossy(&strict_output.stderr);
    assert!(stderr.contains("ML_STYLE_SCALE"));
    assert!(stderr.contains("warning"));
}

#[test]
fn music_build_manifest_strict_rejects_warning_only_diagnostics() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let project = format!("{workspace}/target/music-cli-manifest-strict-build-project");
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(format!("{project}/src")).unwrap();
    fs::write(
        format!("{project}/music.toml"),
        "name = \"manifest-strict-build\"\nsource = \"src/main.music\"\noutput = \"build/out.mid\"\nformat = \"midi\"\nstrict = true\n",
    )
    .unwrap();
    fs::write(
        format!("{project}/src/main.music"),
        r#"
style WarningScale {
  scale: C major
  severity_scale: warning
}

score warning_only style WarningScale {
  key C major
  voice lead {
    note C4, 1/4
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music_in(&["build"], &project);

    assert!(!output.status.success());
    assert!(!std::path::Path::new(&format!("{project}/build/out.mid")).exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ML_STYLE_SCALE"));
    assert!(stderr.contains("warning"));
}

#[test]
fn music_build_strict_rejects_override_suppression() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let project = format!("{workspace}/target/music-cli-strict-build-override-project");
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(format!("{project}/src")).unwrap();
    fs::write(
        format!("{project}/music.toml"),
        "name = \"strict-build-override\"\nsource = \"src/main.music\"\noutput = \"build/out.mid\"\nformat = \"midi\"\n",
    )
    .unwrap();
    fs::write(
        format!("{project}/src/main.music"),
        r#"
style StrictScale {
  scale: C major
}

score hidden style StrictScale {
  key C major
  voice lead {
    override scale allow reason "hide bad chromatic note" {
      note F#4, 1/4
    }
  }
}
"#,
    )
    .unwrap();

    let output = run_music_in(&["build", "--strict"], &project);

    assert!(!output.status.success());
    assert!(!std::path::Path::new(&format!("{project}/build/out.mid")).exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("strict quality gate rejects override suppression"));
}

#[test]
fn music_build_manifest_strict_rejects_disabled_style_rules() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let project = format!("{workspace}/target/music-cli-manifest-strict-build-off-project");
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(format!("{project}/src")).unwrap();
    fs::write(
        format!("{project}/music.toml"),
        "name = \"manifest-strict-build-off\"\nsource = \"src/main.music\"\noutput = \"build/out.mid\"\nformat = \"midi\"\nstrict = true\n",
    )
    .unwrap();
    fs::write(
        format!("{project}/src/main.music"),
        r#"
style HiddenScale {
  scale: C major
  severity_scale: off
}

score hidden style HiddenScale {
  key C major
  voice lead {
    note C4, 1/4
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music_in(&["build"], &project);

    assert!(!output.status.success());
    assert!(!std::path::Path::new(&format!("{project}/build/out.mid")).exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("strict quality gate rejects disabled style rule"));
    assert!(stderr.contains("severity_scale"));
}

#[test]
fn music_build_manifest_strict_rejects_override_suppression() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let project = format!("{workspace}/target/music-cli-manifest-strict-build-override-project");
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(format!("{project}/src")).unwrap();
    fs::write(
        format!("{project}/music.toml"),
        "name = \"manifest-strict-build-override\"\nsource = \"src/main.music\"\noutput = \"build/out.mid\"\nformat = \"midi\"\nstrict = true\n",
    )
    .unwrap();
    fs::write(
        format!("{project}/src/main.music"),
        r#"
style StrictScale {
  scale: C major
}

score hidden style StrictScale {
  key C major
  voice lead {
    override scale allow reason "hide bad chromatic note" {
      note F#4, 1/4
    }
  }
}
"#,
    )
    .unwrap();

    let output = run_music_in(&["build"], &project);

    assert!(!output.status.success());
    assert!(!std::path::Path::new(&format!("{project}/build/out.mid")).exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("strict quality gate rejects override suppression"));
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
    let diagnostics: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostic = diagnostics.as_array().unwrap().first().unwrap();
    assert_eq!(diagnostic["code"], "ML_STYLE_SCALE");
    assert_eq!(diagnostic["severity"], "error");
    assert!(diagnostic["line"].is_number());
    assert!(diagnostic["column"].is_number());
    assert!(diagnostic["span"].is_object());
    assert_eq!(diagnostic["span"]["source_id"], 0);
    assert_eq!(
        diagnostic["span"]["source_name"],
        "examples/style_violation.music"
    );
    assert!(diagnostic["span"]["start"].is_number());
    assert!(diagnostic["span"]["end"].is_number());
    assert!(diagnostic["labels"].as_array().unwrap().is_empty());
    assert!(diagnostic["related"].as_array().unwrap().is_empty());
}

#[test]
fn music_compile_strict_rejects_warning_only_diagnostics() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/compile-strict-warning.music");
    let output_path = format!("{workspace}/target/compile-strict-warning.mid");
    let _ = fs::remove_file(&output_path);
    fs::write(
        &input_path,
        r#"
style WarningScale {
  scale: C major
  severity_scale: warning
}

score warning_only style WarningScale {
  key C major
  voice lead {
    note C4, 1/4
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();

    let compile_output = run_music(&["compile", &input_path, "-o", &output_path]);
    assert!(compile_output.status.success());
    assert!(fs::read(&output_path).unwrap().starts_with(b"MThd"));
    fs::remove_file(&output_path).unwrap();

    let strict_output = run_music(&["compile", &input_path, "-o", &output_path, "--strict"]);

    assert!(!strict_output.status.success());
    assert!(!std::path::Path::new(&output_path).exists());
    let stderr = String::from_utf8_lossy(&strict_output.stderr);
    assert!(stderr.contains("ML_STYLE_SCALE"));
    assert!(stderr.contains("warning"));
}

#[test]
fn music_compile_strict_rejects_disabled_style_rules() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/compile-strict-off.music");
    let output_path = format!("{workspace}/target/compile-strict-off.mid");
    let _ = fs::remove_file(&output_path);
    fs::write(
        &input_path,
        r#"
style HiddenScale {
  scale: C major
  severity_scale: off
}

score hidden style HiddenScale {
  key C major
  voice lead {
    note C4, 1/4
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();

    let strict_output = run_music(&["compile", &input_path, "-o", &output_path, "--strict"]);

    assert!(!strict_output.status.success());
    assert!(!std::path::Path::new(&output_path).exists());
    let stderr = String::from_utf8_lossy(&strict_output.stderr);
    assert!(stderr.contains("strict quality gate rejects disabled style rule"));
    assert!(stderr.contains("severity_scale"));
}

#[test]
fn music_compile_strict_rejects_override_suppression() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/compile-strict-override.music");
    let output_path = format!("{workspace}/target/compile-strict-override.mid");
    let _ = fs::remove_file(&output_path);
    fs::write(
        &input_path,
        r#"
style StrictScale {
  scale: C major
}

score hidden style StrictScale {
  key C major
  voice lead {
    override scale allow reason "hide bad chromatic note" {
      note F#4, 1/4
    }
  }
}
"#,
    )
    .unwrap();

    let strict_output = run_music(&["compile", &input_path, "-o", &output_path, "--strict"]);

    assert!(!strict_output.status.success());
    assert!(!std::path::Path::new(&output_path).exists());
    let stderr = String::from_utf8_lossy(&strict_output.stderr);
    assert!(stderr.contains("strict quality gate rejects override suppression"));
}

#[test]
fn music_export_strict_rejects_warning_only_diagnostics() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/export-strict-warning.music");
    let output_path = format!("{workspace}/target/export-strict-warning.mid");
    let _ = fs::remove_file(&output_path);
    fs::write(
        &input_path,
        r#"
style WarningScale {
  scale: C major
  severity_scale: warning
}

score warning_only style WarningScale {
  key C major
  voice lead {
    note C4, 1/4
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&[
        "export",
        &input_path,
        "--format",
        "midi",
        "-o",
        &output_path,
        "--strict",
    ]);

    assert!(!output.status.success());
    assert!(!std::path::Path::new(&output_path).exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ML_STYLE_SCALE"));
    assert!(stderr.contains("warning"));
}

#[test]
fn music_export_strict_rejects_disabled_style_rules() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/export-strict-off.music");
    let output_path = format!("{workspace}/target/export-strict-off.mid");
    let _ = fs::remove_file(&output_path);
    fs::write(
        &input_path,
        r#"
style HiddenScale {
  scale: C major
  severity_scale: off
}

score hidden style HiddenScale {
  key C major
  voice lead {
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&[
        "export",
        &input_path,
        "--format",
        "midi",
        "-o",
        &output_path,
        "--strict",
    ]);

    assert!(!output.status.success());
    assert!(!std::path::Path::new(&output_path).exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("strict quality gate rejects disabled style rule"));
    assert!(stderr.contains("severity_scale"));
}

#[test]
fn music_export_strict_rejects_override_suppression() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/export-strict-override.music");
    let output_path = format!("{workspace}/target/export-strict-override.mid");
    let _ = fs::remove_file(&output_path);
    fs::write(
        &input_path,
        r#"
style StrictScale {
  scale: C major
}

score hidden style StrictScale {
  key C major
  voice lead {
    override scale allow reason "hide bad chromatic note" {
      note F#4, 1/4
    }
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&[
        "export",
        &input_path,
        "--format",
        "midi",
        "-o",
        &output_path,
        "--strict",
    ]);

    assert!(!output.status.success());
    assert!(!std::path::Path::new(&output_path).exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("strict quality gate rejects override suppression"));
}

#[test]
fn music_export_midi_preserves_score_and_track_metadata() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/test-export-metadata.music");
    let output_path = format!("{workspace}/target/test-export.mid");
    let _ = fs::remove_file(&output_path);
    fs::write(
        &input_path,
        r#"
score midi_metadata {
  title "MIDI Metadata"
  composer "Ada Lovelace"
  tempo 90
  meter 3/4
  key D minor
  voice lead {
    instrument violin
    channel 2
    volume 96
    pan 40
    note C4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&[
        "export",
        &input_path,
        "--format",
        "midi",
        "-o",
        &output_path,
    ]);

    assert!(output.status.success());
    assert!(output.stdout.contains(&b'\n'));
    let bytes = fs::read(&output_path).unwrap();
    assert_valid_midi(&bytes);
    let smf = Smf::parse(&bytes).unwrap();

    assert!(smf.tracks[0].iter().any(|event| matches!(
        event.kind,
        TrackEventKind::Meta(MetaMessage::TrackName(b"MIDI Metadata"))
    )));
    assert!(smf.tracks[0].iter().any(|event| matches!(
        event.kind,
        TrackEventKind::Meta(MetaMessage::Text(b"Ada Lovelace"))
    )));
    assert!(smf.tracks[0].iter().any(|event| matches!(
        event.kind,
        TrackEventKind::Meta(MetaMessage::Tempo(value)) if value.as_int() == 666_666
    )));
    assert!(smf.tracks[0].iter().any(|event| matches!(
        event.kind,
        TrackEventKind::Meta(MetaMessage::TimeSignature(3, 2, 24, 8))
    )));
    assert!(smf.tracks[0].iter().any(|event| matches!(
        event.kind,
        TrackEventKind::Meta(MetaMessage::KeySignature(-1, true))
    )));
    assert!(matches!(
        smf.tracks[1][0].kind,
        TrackEventKind::Meta(MetaMessage::TrackName(b"lead"))
    ));
    assert!(smf.tracks[1].iter().any(|event| matches!(
        event.kind,
        TrackEventKind::Midi {
            channel,
            message: MidiMessage::ProgramChange { program },
        } if channel.as_int() == 2 && program.as_int() == 40
    )));
    assert!(smf.tracks[1].iter().any(|event| matches!(
        event.kind,
        TrackEventKind::Midi {
            channel,
            message: MidiMessage::Controller { controller, value },
        } if channel.as_int() == 2 && controller.as_int() == 7 && value.as_int() == 96
    )));
    assert!(smf.tracks[1].iter().any(|event| matches!(
        event.kind,
        TrackEventKind::Midi {
            channel,
            message: MidiMessage::Controller { controller, value },
        } if channel.as_int() == 2 && controller.as_int() == 10 && value.as_int() == 40
    )));
}

#[test]
fn music_export_strict_compiles_algorithmic_expression_example() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let output_path = format!("{workspace}/target/algorithmic-expression-strict.mid");
    let _ = fs::remove_file(&output_path);

    let output = run_music(&[
        "export",
        "examples/algorithmic_expression.music",
        "--format",
        "midi",
        "-o",
        &output_path,
        "--strict",
    ]);

    assert!(output.status.success());
    let bytes = fs::read(&output_path).unwrap();
    assert_valid_midi(&bytes);
}

#[test]
fn music_export_midi_compiles_expression_generated_material() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/expression-material.music");
    let output_path = format!("{workspace}/target/expression-material.mid");
    let _ = fs::remove_file(&output_path);
    fs::write(
        &input_path,
        r#"
fn line() = [{p:at([C4, D4, E4, G4], i), d:1/8, skip:i == 1} for i in 0..4]
fn shape(events) = [event.with({d:1/4}) for event in events if not event.skip]
score demo {
  voice lead {
    play shape(line())
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&[
        "export",
        &input_path,
        "--format",
        "midi",
        "-o",
        &output_path,
    ]);

    assert!(output.status.success());
    let bytes = fs::read(&output_path).unwrap();
    assert_valid_midi(&bytes);
}

#[test]
fn music_ir_advances_over_rest() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/rest.music");
    fs::write(
        &input_path,
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

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: E"));
    assert!(stdout.contains("start_tick: 1440"));
}

#[test]
fn music_ir_expands_pedal_tone() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/pedal-tone.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice bass {
    pedal C3, 4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("octave: 3"));
    assert!(stdout.contains("start_tick: 1440"));
    assert!(stdout.contains("duration_ticks: 480"));
}

#[test]
fn music_ir_expands_scale_degree_notes() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/scale-degree.music");
    fs::write(
        &input_path,
        r#"
score demo {
  key C major
  voice lead {
    degree 1 4, 1/8
    degree b3 4, 1/8
    modulate G major
    degree 1 4, 1/8
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: Ds"));
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("duration_ticks: 240"));
}

#[test]
fn music_ir_expands_scale_run() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/scale-run.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    scale C major 4, 1/8
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("class: E"));
    assert!(stdout.contains("start_tick: 1680"));
    assert!(stdout.contains("duration_ticks: 240"));
}

#[test]
fn music_ir_expands_ostinato_block() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/ostinato.music");
    fs::write(
        &input_path,
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

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("start_tick: 1200"));
    assert!(stdout.contains("duration_ticks: 240"));
}

#[test]
fn music_ir_expands_transpose_block() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/transpose.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    transpose M2 {
      note C4, 1/8
      chord [E4, G4], 1/8
    }
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("class: Fs"));
    assert!(stdout.contains("class: A"));
    assert!(stdout.contains("duration_ticks: 240"));
}

#[test]
fn music_ir_expands_sequence_block() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/sequence.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    sequence 3 by M2 {
      note C4, 1/8
    }
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("class: E"));
    assert!(stdout.contains("start_tick: 480"));
    assert!(stdout.contains("duration_ticks: 240"));
}

#[test]
fn music_ir_scales_tuplet_block() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/tuplet.music");
    fs::write(
        &input_path,
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

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("duration_ticks: 160"));
    assert!(stdout.contains("start_tick: 320"));
    assert!(stdout.contains("start_tick: 480"));
    assert!(stdout.contains("duration_ticks: 480"));
}

#[test]
fn music_ir_expands_glissando_as_stepped_notes() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/glissando.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    glissando C4 to G4 steps 5, 1/16
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: Cs"));
    assert!(stdout.contains("class: Ds"));
    assert!(stdout.contains("class: F"));
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("start_tick: 120"));
    assert!(stdout.contains("start_tick: 480"));
    assert!(stdout.contains("duration_ticks: 120"));
}

#[test]
fn music_ir_expands_tremolo_as_alternating_notes() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/tremolo.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice strings {
    tremolo C4 with G4 repeats 4, 1/32
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("start_tick: 60"));
    assert!(stdout.contains("start_tick: 180"));
    assert!(stdout.contains("duration_ticks: 60"));
}

#[test]
fn music_ir_expands_strum_as_staggered_notes() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/strum.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice guitar {
    strum [C4, E4, G4], 1/2 by 1/32
    strum C4 dominant7 inv 2, 1/2 by 1/64
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: E"));
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("class: As"));
    assert!(stdout.contains("start_tick: 60"));
    assert!(stdout.contains("start_tick: 120"));
    assert!(stdout.contains("start_tick: 1050"));
    assert!(stdout.contains("duration_ticks: 960"));
}

#[test]
fn music_ir_expands_arpeggio_as_sequential_notes() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/arpeggio.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    arpeggio [C4, E4, G4], 1/8
    arpeggio D3 minor inv 1, 1/16
    transpose M2 {
      arpeggio [C4, E4, G4], 1/8
    }
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: E"));
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("class: F"));
    assert!(stdout.contains("class: A"));
    assert!(stdout.contains("class: Fs"));
    assert!(stdout.contains("start_tick: 0"));
    assert!(stdout.contains("start_tick: 240"));
    assert!(stdout.contains("start_tick: 480"));
    assert!(stdout.contains("duration_ticks: 120"));
}

#[test]
fn music_ir_expands_named_chord_quality() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/named-chord.music");
    fs::write(
        &input_path,
        r#"
score demo {
  voice lead {
    chord D3 minor7 inv 1, 1/2
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: F"));
    assert!(stdout.contains("class: A"));
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("octave: 4"));
    assert!(stdout.contains("duration_ticks: 960"));
}

#[test]
fn music_ir_expands_roman_numeral_chord() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/roman-chord.music");
    fs::write(
        &input_path,
        r#"
score demo {
  key C major
  voice lead {
    roman V65/V, 1/2
    roman bVII, 1/4
    roman viidim/V, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: Fs"));
    assert!(stdout.contains("class: As"));
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("class: F"));
    assert!(stdout.contains("duration_ticks: 960"));
}

#[test]
fn music_ir_expands_harmonic_progression() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/harmonic-progression.music");
    fs::write(
        &input_path,
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

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("class: A"));
    assert!(stdout.contains("class: B"));
    assert!(stdout.contains("start_tick: 1920"));
    assert!(stdout.contains("duration_ticks: 480"));
}

#[test]
fn music_ir_expands_named_cadence() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/named-cadence.music");
    fs::write(
        &input_path,
        r#"
score demo {
  key C major
  voice lead {
    cadence authentic, 1/2
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("class: B"));
    assert!(stdout.contains("class: C"));
    assert!(stdout.contains("start_tick: 960"));
    assert!(stdout.contains("duration_ticks: 960"));
}

#[test]
fn music_ir_applies_modulation_to_roman_chords() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/modulation.music");
    fs::write(
        &input_path,
        r#"
score demo {
  key C major
  voice lead {
    roman I, 1/4
    modulate G major
    roman I, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("class: G"));
    assert!(stdout.contains("class: B"));
    assert!(stdout.contains("class: D"));
    assert!(stdout.contains("octave: 5"));
    assert!(stdout.contains("start_tick: 480"));
}

#[test]
fn music_export_musicxml_produces_valid_file() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = write_analyze_metadata_fixture();
    let output_path = format!("{workspace}/target/test-export.musicxml");
    let _ = fs::remove_file(&output_path);

    let output = run_music(&[
        "export",
        &input_path,
        "--format",
        "musicxml",
        "-o",
        &output_path,
    ]);

    assert!(output.status.success());
    let xml = fs::read_to_string(&output_path).unwrap();
    assert!(xml.starts_with("<?xml"));
    assert!(xml.contains("<score-partwise"));
    assert!(xml.contains("<work-title>String Quartet</work-title>"));
    assert!(xml.contains("<creator type=\"composer\">Ada Lovelace</creator>"));
    assert!(xml.contains("<score-part id=\"P1\">"));
    assert!(xml.contains("<part-name>lead</part-name>"));
    assert!(xml.contains(
        "<score-instrument id=\"P1-I1\"><instrument-name>lead</instrument-name></score-instrument>"
    ));
    assert!(xml.contains("<midi-instrument id=\"P1-I1\">"));
    assert!(xml.contains("<score-part id=\"P2\">"));
    assert!(xml.contains("<part-name>alto</part-name>"));
    assert!(xml.contains("<score-part id=\"P3\">"));
    assert!(xml.contains("<part-name>bass</part-name>"));
    assert!(xml.contains("<measure number=\"1\">"));
    assert!(xml.contains("<fifths>-1</fifths>"));
    assert!(xml.contains("<mode>minor</mode>"));
    assert!(xml.contains("form section A"));
    assert!(xml.contains("phrase section A"));
    assert!(xml.contains("phrase motif_call motif"));
    assert!(xml.contains("motif motif transposition"));
    assert!(xml.contains("harmony i"));
    assert!(xml.contains("melody scale_degree degree 3"));
    assert!(xml.matches("<attributes>").count() >= 2);
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
    assert_valid_wav(&bytes);
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
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/ir-track-metadata.music");
    fs::write(
        &input_path,
        r#"
fn motif(root) {
  note root, 1/8
}
score ir_track_metadata {
  tempo 144
  meter 6/8
  key G major
  voice lead {
    section Theme {
      tempo 144
      meter 6/8
      key G major
      degree b3 4, 1/8
      progression I V I, 1/8
      call motif(C4)
    }
    program 40
    volume 96
    pan 32
    note C4, 1/4
  }
}
"#,
    )
    .unwrap();
    let output = run_music(&["ir", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ScoreIr"));
    assert!(stdout.contains("tempo_bpm"));
    assert!(stdout.contains("program: Some(\n                40,"));
    assert!(stdout.contains("volume: Some(\n                96,"));
    assert!(stdout.contains("pan: Some(\n                32,"));
    assert!(stdout.contains("tempo_changes"));
    assert!(stdout.contains("bpm: 144"));
    assert!(stdout.contains("meter_changes"));
    assert!(stdout.contains("numerator: 6"));
    assert!(stdout.contains("denominator: 8"));
    assert!(stdout.contains("key_changes"));
    assert!(stdout.contains("fifths: 1"));
    assert!(stdout.contains("markers"));
    assert!(stdout.contains("Theme"));
    assert!(stdout.contains("harmonic_events"));
    assert!(stdout.contains("normalized_symbol"));
    assert!(stdout.contains("melodic_events"));
    assert!(stdout.contains("scale_degree"));
    assert!(stdout.contains("form_events"));
    assert!(stdout.contains("kind: \"section\""));
    assert!(stdout.contains("motif_events"));
    assert!(stdout.contains("name: \"motif\""));
    assert!(stdout.contains("phrase_events"));
    assert!(stdout.contains("kind: \"motif_call\""));
}

fn write_analyze_metadata_fixture() -> String {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/analyze-metadata.music");
    fs::write(
        &input_path,
        r#"
fn motif(root) {
  note root, 1/4
}
score demo {
  title "String Quartet"
  composer "Ada Lovelace"
  tempo 96
  meter 3/4
  key D minor
  voice lead {
    section A {
      degree b3 4, 1/4
      progression i iv, 1/4
      call motif(G4)
      call motif(A4)
    }
  }
  voice alto {
    note A3, 1/4
  }
  voice bass {
    note D3, 1/2
  }
}
"#,
    )
    .unwrap();
    input_path
}

#[test]
fn music_analyze_summarizes_score() {
    let input_path = write_analyze_metadata_fixture();
    let output = run_music(&["analyze", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("title: String Quartet"));
    assert!(stdout.contains("composer: Ada Lovelace"));
    assert!(stdout.contains("tempo: 96 bpm"));
    assert!(stdout.contains("meter: 3/4"));
    assert!(stdout.contains("key: D minor"));
    assert!(stdout.contains("tracks: 3"));
    assert!(stdout.contains("events: 11"));
    assert!(stdout.contains("duration_ticks: 2400"));
    assert!(stdout.contains("bar_ticks: 1440"));
    assert!(stdout.contains("duration_bars: 2"));
    assert!(stdout.contains("density_per_bar: 6"));
    assert!(stdout.contains("repeated_bar_count: 0"));
    assert!(stdout.contains("repeated_bar_ratio_percent: 0"));
    assert!(stdout.contains("longest_repeated_bar_run: 1"));
    assert!(stdout.contains("max_simultaneous_events: 3"));
    assert!(stdout.contains("texture: dense_polyphonic"));
    assert!(stdout.contains("pitch_range: D3..D5"));
    assert!(stdout.contains("pitch_classes: A,A#,D,E,F,G"));
    assert!(stdout.contains("roman_roots: biii,bvi,i,ii,iv,v"));
    assert!(stdout.contains("sonority tick=480: pcs=D,F,A, root=D, quality=minor, roman=i"));
    assert!(stdout.contains("harmonic_events: 2"));
    assert!(stdout.contains("melodic_events: 1"));
    assert!(stdout.contains("form_events: 1"));
    assert!(stdout.contains("motif_events: 2"));
    assert!(stdout.contains("phrase_events: 3"));
    assert!(stdout.contains("section_phrases: 1"));
    assert!(stdout.contains("motif_phrases: 2"));
    assert!(stdout.contains("periodic_phrase_candidate: false"));
    assert!(stdout.contains("longest_phrase_duration_ticks: 2400"));
    assert!(stdout.contains("phrase section: label=A, start_tick=0, duration_ticks=2400"));
    assert!(stdout.contains("phrase motif_call: label=motif, start_tick=1440, duration_ticks=480"));
    assert!(stdout.contains("phrase motif_call: label=motif, start_tick=1920, duration_ticks=480"));
    assert!(stdout.contains("distinct_motifs: 1"));
    assert!(stdout.contains("repeated_motifs: 1"));
    assert!(stdout.contains("transformed_motifs: 2"));
    assert!(stdout.contains("longest_motif_run: 2"));
    assert!(
        stdout.contains("motif motif: count=2, total_duration_ticks=960, transforms=transposition")
    );
    assert!(stdout.contains("track lead: events=9, density_per_bar=5, range=D4..D5"));
    assert!(stdout.contains("track alto: events=1, density_per_bar=1, range=A3..A3"));
    assert!(stdout.contains("track bass: events=1, density_per_bar=1, range=D3..D3"));
}

#[test]
fn music_analyze_reports_repeated_bars() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/analyze-repetition.music");
    fs::write(
        &input_path,
        r#"
score repetitive {
  tempo 120
  meter 4/4
  voice lead {
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
  }
}
"#,
    )
    .unwrap();
    let output = run_music(&["analyze", &input_path]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("duration_bars: 4"));
    assert!(stdout.contains("repeated_bar_count: 3"));
    assert!(stdout.contains("repeated_bar_ratio_percent: 75"));
    assert!(stdout.contains("longest_repeated_bar_run: 4"));
}

#[test]
fn music_analyze_strict_rejects_repeated_arrangement() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/analyze-strict-repetition.music");
    fs::write(
        &input_path,
        r#"
score repetitive {
  tempo 120
  meter 4/4
  voice lead {
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
    note C4, 1/4
    note E4, 1/4
    note G4, 1/4
    note E4, 1/4
  }
}
"#,
    )
    .unwrap();
    let output = run_music(&["analyze", &input_path, "--strict"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("analysis quality gate failed"));
    assert!(stderr.contains("repeated_bar_ratio_percent 75 exceeds 50"));
}

#[test]
fn music_analyze_strict_rejects_warning_only_diagnostics() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/analyze-strict-warning.music");
    fs::write(
        &input_path,
        r#"
style WarningScale {
  scale: C major
  severity_scale: warning
}

score warning_only style WarningScale {
  key C major
  voice lead {
    note C4, 1/4
    note F#4, 1/4
  }
}
"#,
    )
    .unwrap();
    let output = run_music(&["analyze", &input_path, "--strict"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("analysis quality gate failed"));
    assert!(stderr.contains("diagnostics 1 exceeds 0"));
}

#[test]
fn music_analyze_strict_rejects_override_suppression() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/analyze-strict-override.music");
    fs::write(
        &input_path,
        r#"
style StrictScale {
  scale: C major
}

score hidden style StrictScale {
  key C major
  voice lead {
    override scale allow reason "hide bad chromatic note" {
      note F#4, 1/4
    }
  }
}
"#,
    )
    .unwrap();
    let output = run_music(&["analyze", &input_path, "--strict"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("strict quality gate rejects override suppression"));
}

#[test]
fn music_analyze_strict_accepts_demo() {
    let output = run_music(&["analyze", "examples/demo_jazz_complete.music", "--strict"]);

    assert!(output.status.success());
}

#[test]
fn music_analyze_json_is_machine_readable() {
    let input_path = write_analyze_metadata_fixture();
    let output = run_music(&["analyze", &input_path, "--json"]);

    assert!(output.status.success());
    let analysis: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(analysis["title"], "String Quartet");
    assert_eq!(analysis["composer"], "Ada Lovelace");
    assert_eq!(analysis["tempo_bpm"], 96);
    assert_eq!(analysis["meter"]["numerator"], 3);
    assert_eq!(analysis["meter"]["denominator"], 4);
    assert_eq!(analysis["key"]["tonic"], "D");
    assert_eq!(analysis["key"]["mode"], "minor");
    assert_eq!(analysis["key"]["fifths"], -1);
    assert_eq!(analysis["track_count"], 3);
    assert_eq!(analysis["event_count"], 11);
    assert_eq!(analysis["duration_ticks"], 2400);
    assert_eq!(analysis["bar_ticks"], 1440);
    assert_eq!(analysis["duration_bars"], 2);
    assert_eq!(analysis["density_per_bar"], 6);
    assert_eq!(analysis["repeated_bar_count"], 0);
    assert_eq!(analysis["repeated_bar_ratio_percent"], 0);
    assert_eq!(analysis["longest_repeated_bar_run"], 1);
    assert_eq!(analysis["max_simultaneous_events"], 3);
    assert_eq!(analysis["texture"], "dense_polyphonic");
    assert_eq!(analysis["pitch_min"], "D3");
    assert_eq!(analysis["pitch_max"], "D5");
    assert_eq!(analysis["pitch_classes"].as_array().unwrap().len(), 6);
    assert!(analysis["pitch_classes"]
        .as_array()
        .unwrap()
        .contains(&serde_json::Value::String("A".to_string())));
    assert!(analysis["pitch_classes"]
        .as_array()
        .unwrap()
        .contains(&serde_json::Value::String("D".to_string())));
    assert_eq!(analysis["roman_roots"].as_array().unwrap().len(), 6);
    assert_eq!(analysis["sonorities"].as_array().unwrap().len(), 3);
    let sonority = &analysis["sonorities"][1];
    assert_eq!(sonority["tick"], 480);
    assert_eq!(sonority["root"], "D");
    assert_eq!(sonority["quality"], "minor");
    assert_eq!(sonority["roman"], "i");
    assert_eq!(analysis["tracks"].as_array().unwrap().len(), 3);
    assert_eq!(analysis["harmonic_event_count"], 2);
    assert_eq!(analysis["melodic_event_count"], 1);
    assert_eq!(analysis["form_event_count"], 1);
    assert_eq!(analysis["motif_event_count"], 2);
    assert_eq!(analysis["phrase_event_count"], 3);
    assert_eq!(analysis["section_phrase_count"], 1);
    assert_eq!(analysis["motif_phrase_count"], 2);
    assert_eq!(analysis["periodic_phrase_candidate"], false);
    assert_eq!(analysis["longest_phrase_duration_ticks"], 2400);
    let phrases = analysis["phrases"].as_array().unwrap();
    assert_eq!(phrases.len(), 3);
    assert_eq!(phrases[0]["kind"], "motif_call");
    assert_eq!(phrases[0]["label"], "motif");
    assert_eq!(phrases[0]["start_tick"], 1440);
    assert_eq!(phrases[0]["duration_ticks"], 480);
    assert_eq!(phrases[1]["kind"], "motif_call");
    assert_eq!(phrases[1]["start_tick"], 1920);
    assert_eq!(phrases[1]["duration_ticks"], 480);
    assert_eq!(phrases[2]["kind"], "section");
    assert_eq!(phrases[2]["label"], "A");
    assert_eq!(phrases[2]["start_tick"], 0);
    assert_eq!(phrases[2]["duration_ticks"], 2400);
    assert_eq!(analysis["distinct_motif_count"], 1);
    assert_eq!(analysis["repeated_motif_count"], 1);
    assert_eq!(analysis["transformed_motif_count"], 2);
    assert_eq!(analysis["longest_motif_run"], 2);
    let motifs = analysis["motifs"].as_array().unwrap();
    assert_eq!(motifs.len(), 1);
    assert_eq!(motifs[0]["name"], "motif");
    assert_eq!(motifs[0]["count"], 2);
    assert_eq!(motifs[0]["total_duration_ticks"], 960);
    assert_eq!(motifs[0]["transforms"].as_array().unwrap().len(), 1);
    assert_eq!(motifs[0]["transforms"][0], "transposition");
    let tracks = analysis["tracks"].as_array().unwrap();
    let lead = tracks.iter().find(|t| t["name"] == "lead").unwrap();
    assert_eq!(lead["event_count"], 9);
    assert_eq!(lead["density_per_bar"], 5);
    assert_eq!(lead["pitch_min"], "D4");
    assert_eq!(lead["pitch_max"], "D5");
    assert_eq!(analysis["override_count"], 0);
    assert_eq!(analysis["diagnostic_count"], 0);
    assert_eq!(analysis["warning_count"], 0);
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
fn music_repl_loads_diagnoses_shows_ir_and_exports() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let output_path = format!("{workspace}/target/repl-export.mid");
    let _ = fs::remove_file(&output_path);
    let script = format!(
        ":load examples/minimal.music\n:diagnose\n:show ir\n:export {output_path}\n:quit\n"
    );

    let output = run_music_with_stdin(&["repl"], &script);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("loaded examples/minimal.music"));
    assert!(stdout.contains("ok"));
    assert!(stdout.contains("ScoreIr"));
    assert!(stdout.contains(&format!("wrote {output_path}")));
    assert!(fs::read(&output_path).unwrap().starts_with(b"MThd"));
}

#[test]
fn music_repl_style_applies_active_style_to_buffer() {
    let script = ":style Modal\nscore repl_style {\n  key C major\n  voice lead {\n    note C4, 1/4\n  }\n}\n:diagnose\n:show source\n:quit\n";

    let output = run_music_with_stdin(&["repl"], script);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(":style <name>"));
    assert!(stdout.contains("style Modal"));
    assert!(stdout.contains("ok"));
    assert!(stdout.contains("style Modal\nscore repl_style"));
}

#[test]
fn music_repl_play_renders_current_buffer_without_blocking() {
    let script = "score repl_play {\n  voice lead {\n    note C4, 1/4\n  }\n}\n:play\n:reset\nscore repl_play_next {\n  voice lead {\n    note E4, 1/4\n  }\n}\n:play\n:stop\n:quit\n";

    let output = run_music_with_stdin(&["repl"], script);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(":play"));
    assert!(stdout.contains(":stop"));
    assert!(stdout.contains("set MUSICLANG_PLAYER"));
    let rendered = stdout
        .lines()
        .filter_map(|line| {
            line.split_once("rendered ")
                .map(|(_, path)| path.trim().to_string())
                .filter(|path| path.ends_with(".mid"))
        })
        .collect::<Vec<_>>();
    assert_eq!(rendered.len(), 2);
    let first = fs::read(&rendered[0]).unwrap();
    let second = fs::read(&rendered[1]).unwrap();
    assert_valid_midi(&first);
    assert_valid_midi(&second);
    assert_ne!(first, second);
}

#[test]
fn music_repl_lists_idioms() {
    let output = run_music_with_stdin(&["repl"], ":idioms\n:quit\n");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("phrase_concept"));
    assert!(stdout.contains("periodic_phrase"));
    assert!(stdout.contains("motivic_development"));
}

#[test]
fn music_repl_lists_styles_formats_and_theory() {
    let output = run_music_with_stdin(
        &["repl"],
        ":styles\n:formats\n:theory scales\n:theory maqam\n:quit\n",
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Classical"));
    assert!(stdout.contains("midi"));
    assert!(stdout.contains("musicxml"));
    assert!(stdout.contains("scales:blues"));
    assert!(stdout.contains("world_traditions:maqam"));
}

#[test]
fn music_repl_reset_clears_source_buffer() {
    let script =
        "score demo {\n  voice lead {\n    note C4, 1/4\n  }\n}\n:reset\n:show source\n:quit\n";

    let output = run_music_with_stdin(&["repl"], script);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("reset"));
    let after_reset = stdout.split("reset").last().unwrap();
    assert!(!after_reset.contains("score demo"));
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
fn music_diagnose_json_reports_duplicate_function_related_span() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/duplicate-function.music");
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
    fs::write(&input_path, source).unwrap();

    let output = run_music(&["diagnose", &input_path, "--json"]);

    assert!(output.status.success());
    let diagnostics: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostic = diagnostics.as_array().unwrap().first().unwrap();
    assert_eq!(diagnostic["code"], "ML_RESOLVE_DUPLICATE_NAME");
    assert_eq!(
        diagnostic["help"],
        "rename one function or remove the duplicate definition"
    );
    assert_eq!(
        diagnostic["related"][0]["message"],
        "first function definition"
    );
    assert_eq!(
        diagnostic["related"][0]["span"]["start"],
        source.find("fn motif").unwrap()
    );
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
    let diagnostics: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostic = diagnostics.as_array().unwrap().first().unwrap();
    assert_eq!(diagnostic["code"], "ML_RESOLVE_RECURSIVE_CALL");
    assert_eq!(diagnostic["message"], "recursive function call `motif`");
}

#[test]
fn music_diagnose_json_reports_compiler_span() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/json-unknown-function.music");
    let source = r#"
score demo {
  voice lead {
    call missing
  }
}
"#;
    fs::write(&input_path, source).unwrap();

    let output = run_music(&["diagnose", &input_path, "--json"]);

    assert!(output.status.success());
    let diagnostics: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostic = diagnostics.as_array().unwrap().first().unwrap();
    let expected_start = source.find("call missing").unwrap();
    let expected_end = expected_start + "call".len();
    assert_eq!(diagnostic["code"], "ML_RESOLVE_UNKNOWN_NAME");
    assert_eq!(
        diagnostic["help"],
        "define the function before calling it or correct the function name"
    );
    assert_eq!(diagnostic["span"]["start"], expected_start);
    assert_eq!(diagnostic["span"]["end"], expected_end);
}

#[test]
fn music_diagnose_json_reports_unknown_style_name() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/json-unknown-style.music");
    fs::write(
        &input_path,
        r#"
style Classical
score demo style Missing {
  voice lead {
    note C4, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["diagnose", &input_path, "--json"]);

    assert!(output.status.success());
    let diagnostics: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostic = diagnostics.as_array().unwrap().first().unwrap();
    assert_eq!(diagnostic["code"], "ML_STYLE_UNKNOWN_NAME");
    assert_eq!(diagnostic["style"], "Missing");
    assert_eq!(
        diagnostic["help"],
        "declare the style before selecting it or choose a built-in style name"
    );
}

#[test]
fn music_diagnose_json_reports_style_inheritance_cycle() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/json-style-cycle.music");
    fs::write(
        &input_path,
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
    .unwrap();

    let output = run_music(&["diagnose", &input_path, "--json"]);

    assert!(output.status.success());
    let diagnostics: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostic = diagnostics.as_array().unwrap().first().unwrap();
    assert_eq!(diagnostic["code"], "ML_STYLE_INHERITANCE_CYCLE");
    assert_eq!(diagnostic["style"], "A");
    assert_eq!(
        diagnostic["help"],
        "break the extends cycle by removing or changing one parent style"
    );
}

#[test]
fn music_diagnose_json_reports_style_rule_metadata() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/json-jazz-idiom.music");
    fs::write(
        &input_path,
        r#"
score weak_jazz style Jazz {
  tempo 112
  meter 4/4
  key C major
  voice lead {
    note C4, 1/4
    note E4, 1/4
    note G4, 1/2
  }
  voice bass {
    instrument bass
    note C2, 1/4
    note E2, 1/4
    note G2, 1/4
    note B2, 1/4
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["diagnose", &input_path, "--json"]);

    assert!(output.status.success());
    let diagnostics: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostics = diagnostics.as_array().unwrap();
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic["code"] == "ML_STYLE_MELODIC_CONCEPT")
        .unwrap();

    assert_eq!(diagnostic["severity"], "warning");
    assert_eq!(diagnostic["rule"], "melodic_concept");
    assert_eq!(diagnostic["style"], "Jazz");
    assert_eq!(
        diagnostic["help"],
        "adjust the active style rule `melodic_concept` or use an explicit audited override for intentional local exceptions"
    );
    assert!(diagnostic["span"].is_object());
}

#[test]
fn music_diagnose_json_reports_phrase_concept_metadata() {
    let workspace = env!("CARGO_MANIFEST_DIR");
    let input_path = format!("{workspace}/target/json-phrase-concept.music");
    fs::write(
        &input_path,
        r#"
style Periodic {
  phrase_concept: periodic_phrase
}
score fragment style Periodic {
  voice lead {
    section A {
      note C4, 1/4
    }
  }
}
"#,
    )
    .unwrap();

    let output = run_music(&["diagnose", &input_path, "--json"]);

    assert!(output.status.success());
    let diagnostics: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostic = diagnostics
        .as_array()
        .unwrap()
        .iter()
        .find(|diagnostic| diagnostic["code"] == "ML_STYLE_PHRASE_CONCEPT")
        .unwrap();

    assert_eq!(diagnostic["severity"], "error");
    assert_eq!(diagnostic["rule"], "phrase_concept");
    assert_eq!(diagnostic["style"], "Periodic");
    assert_eq!(
        diagnostic["help"],
        "adjust the active style rule `phrase_concept` or use an explicit audited override for intentional local exceptions"
    );
    assert!(diagnostic["span"].is_object());
}
