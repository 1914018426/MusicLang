use std::collections::BTreeMap;

use musiclang_core::{
    FormEventIr, HarmonicEventIr, KeyChangeIr, KeySignature, MarkerIr, MelodicEventIr, Meter,
    MeterChangeIr, MotifEventIr, OverrideTrace, PhraseEventIr, ScoreIr, TempoChangeIr, TrackIr,
    DEFAULT_TICKS_PER_QUARTER,
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

pub(super) struct ScoreLoweringParts {
    pub tracks: Vec<TrackIr>,
    pub markers: Vec<MarkerIr>,
    pub tempo_changes: Vec<TempoChangeIr>,
    pub meter_changes: Vec<MeterChangeIr>,
    pub key_changes: Vec<KeyChangeIr>,
    pub harmonic_events: Vec<HarmonicEventIr>,
    pub melodic_events: Vec<MelodicEventIr>,
    pub form_events: Vec<FormEventIr>,
    pub motif_events: Vec<MotifEventIr>,
    pub phrase_events: Vec<PhraseEventIr>,
    pub overrides: Vec<OverrideTrace>,
}

pub(super) fn score_ir(metadata: ScoreMetadata, parts: ScoreLoweringParts) -> ScoreIr {
    let mut metadata_map = BTreeMap::from([("title".to_string(), metadata.title.clone())]);
    if let Some(composer) = &metadata.composer {
        metadata_map.insert("composer".to_string(), composer.clone());
    }

    ScoreIr {
        title: metadata.title,
        composer: metadata.composer,
        ticks_per_quarter: DEFAULT_TICKS_PER_QUARTER,
        tempo_bpm: metadata.tempo_bpm,
        meter: metadata.meter,
        key: metadata.key,
        metadata: metadata_map,
        tracks: parts.tracks,
        markers: parts.markers,
        tempo_changes: parts.tempo_changes,
        meter_changes: parts.meter_changes,
        key_changes: parts.key_changes,
        harmonic_events: parts.harmonic_events,
        melodic_events: parts.melodic_events,
        form_events: parts.form_events,
        motif_events: parts.motif_events,
        phrase_events: parts.phrase_events,
        overrides: parts.overrides,
    }
}
