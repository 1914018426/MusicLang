use musiclang_core::{
    KeySignature, MarkerIr, Meter, OverrideTrace, ScoreIr, TrackIr, DEFAULT_TICKS_PER_QUARTER,
};
use musiclang_parser::{Program, ScoreMeta};

use crate::key_signature;

pub(super) fn score_metadata(
    program: &Program,
) -> (
    String,
    Option<String>,
    u16,
    Option<Meter>,
    Option<KeySignature>,
) {
    let mut title = program.score.name.clone();
    let mut composer = None;
    let mut tempo_bpm = 120;
    let mut meter = None;
    let mut key = None;
    for meta in &program.score.metadata {
        match meta {
            ScoreMeta::Title(value) => title = value.value.clone(),
            ScoreMeta::Composer(value) => composer = Some(value.value.clone()),
            ScoreMeta::Tempo(tempo) => tempo_bpm = tempo.bpm,
            ScoreMeta::Meter(value) => {
                meter = Some(Meter {
                    numerator: value.numerator,
                    denominator: value.denominator,
                });
            }
            ScoreMeta::Key(value) => key = key_signature(&value.tonic, &value.mode),
        }
    }
    (title, composer, tempo_bpm, meter, key)
}

pub(super) struct ScoreMetadata {
    pub title: String,
    pub composer: Option<String>,
    pub tempo_bpm: u16,
    pub meter: Option<Meter>,
    pub key: Option<KeySignature>,
}

pub(super) fn score_ir(
    metadata: ScoreMetadata,
    tracks: Vec<TrackIr>,
    markers: Vec<MarkerIr>,
    overrides: Vec<OverrideTrace>,
) -> ScoreIr {
    ScoreIr {
        title: metadata.title,
        composer: metadata.composer,
        ticks_per_quarter: DEFAULT_TICKS_PER_QUARTER,
        tempo_bpm: metadata.tempo_bpm,
        meter: metadata.meter,
        key: metadata.key,
        tracks,
        markers,
        overrides,
    }
}
