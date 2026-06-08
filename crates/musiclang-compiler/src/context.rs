use std::collections::HashMap;

use musiclang_core::{Diagnostic, StyleContext};
use musiclang_parser::{FunctionDecl, Program};

use crate::style_from_program;

pub(super) fn style(program: &Program) -> (StyleContext, Vec<Diagnostic>) {
    if let Some(active_style) = &program.score.style {
        if let Some(style) = program
            .styles
            .iter()
            .find(|style| &style.name == active_style)
        {
            return style_from_program(program, style);
        }
        if let Some(style) = StyleContext::built_in(active_style) {
            return (style, Vec::new());
        }
        return (
            StyleContext::core(),
            vec![Diagnostic::error(
                "ML_STYLE_UNKNOWN_NAME",
                format!("unknown style `{active_style}`"),
                program.score.line,
                program.score.column,
            )],
        );
    }
    program
        .style
        .as_ref()
        .map(|style| style_from_program(program, style))
        .unwrap_or_else(|| (StyleContext::core(), Vec::new()))
}

pub(super) fn functions(program: &Program) -> (HashMap<String, FunctionDecl>, Vec<Diagnostic>) {
    let mut functions = HashMap::new();
    let mut diagnostics = Vec::new();
    for function in &program.functions {
        if functions
            .insert(function.name.clone(), function.clone())
            .is_some()
        {
            diagnostics.push(Diagnostic::error(
                "ML_RESOLVE_DUPLICATE_NAME",
                format!("duplicate function `{}`", function.name),
                function.line,
                function.column,
            ));
        }
    }
    (functions, diagnostics)
}
