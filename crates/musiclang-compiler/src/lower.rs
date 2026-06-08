use musiclang_core::{
    KeySignature, MarkerIr, Meter, OverrideTrace, ScoreIr, TrackIr, DEFAULT_TICKS_PER_QUARTER,
};
use musiclang_parser::{Program, ScoreMeta};

use crate::key_signature;

pub(super) fn score_metadata(program: &Program) -> (u16, Option<Meter>, Option<KeySignature>) {
    let mut tempo_bpm = 120;
    let mut meter = None;
    let mut key = None;
    for meta in &program.score.metadata {
        match meta {
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
    (tempo_bpm, meter, key)
}

pub(super) fn score_ir(
    program: Program,
    tempo_bpm: u16,
    meter: Option<Meter>,
    key: Option<KeySignature>,
    tracks: Vec<TrackIr>,
    markers: Vec<MarkerIr>,
    overrides: Vec<OverrideTrace>,
) -> ScoreIr {
    ScoreIr {
        title: program.score.name,
        ticks_per_quarter: DEFAULT_TICKS_PER_QUARTER,
        tempo_bpm,
        meter,
        key,
        tracks,
        markers,
        overrides,
    }
}
