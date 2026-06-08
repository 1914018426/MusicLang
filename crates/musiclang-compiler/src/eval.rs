use std::collections::HashMap;

use musiclang_core::{Duration, Interval, Pitch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Value {
    Int(i32),
    Bool(bool),
    Pitch(Pitch),
    Interval(Interval),
    Duration(Duration),
    String(String),
    List(Vec<Value>),
    Tuple(Vec<Value>),
    Dict(HashMap<String, Value>),
}
