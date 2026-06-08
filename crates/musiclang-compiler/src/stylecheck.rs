use musiclang_core::Pitch;

pub(super) fn interval_mod_12(upper: Pitch, lower: Pitch) -> Option<u8> {
    let upper = upper.midi_number().ok()?;
    let lower = lower.midi_number().ok()?;
    Some(upper.wrapping_sub(lower) % 12)
}

pub(super) fn known_rule(rule: &str) -> bool {
    matches!(
        rule,
        "scale"
            | "chord_vocab"
            | "chord_quality_vocab"
            | "set_class_vocab"
            | "meter"
            | "meter_catalog"
            | "tempo_range"
            | "rhythm_vocab"
            | "rhythm_concept"
            | "melodic_concept"
            | "phrase_concept"
            | "ensemble_concept"
            | "bass_concept"
            | "dynamic_vocab"
            | "articulation_vocab"
            | "ornament"
            | "non_chord_tone"
            | "tuning_system"
            | "world_tradition"
            | "historical_era"
            | "harmonic_function"
            | "max_melodic_leap"
            | "contrapuntal_motion"
            | "voice_spacing"
            | "cadence"
            | "harmonic_progression"
            | "texture"
            | "form"
            | "instrument_range"
            | "parallel_fifths"
            | "voice_crossing"
    )
}
