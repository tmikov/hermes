//! Diagnostic types, handler trait, collecting handler, output options, and
//! warning categories. Port of `SourceErrorManager` diagnostic infrastructure.

use crate::location::SourceCoords;

/// Kind of diagnostic. Port of `SourceErrorManager::DiagKind`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DiagKind {
    Error,
    Warning,
    Note,
}

/// Subsystem that produced a message. Port of `Subsystem`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Subsystem {
    /// No specific system provided.
    Unspecified,
    /// e.g. JSLexer or something with similar functionality.
    Lexer,
    /// e.g. JSParser, JSONParser or something with similar functionality.
    Parser,
}

/// Options for outputting errors. Port of `SourceErrorOutputOptions`.
#[derive(Copy, Clone, Debug)]
pub struct OutputOptions {
    /// Determine whether errors should be colorized.
    pub show_colors: bool,
    /// Soft limit on how wide errors should be (None = unlimited).
    pub preferred_max_error_width: Option<usize>,
}

impl OutputOptions {
    /// Width of a tab.
    pub const TAB_STOP: usize = 8;
    /// Minimum context (in source characters) around a highlighted range.
    pub const MINIMUM_SOURCE_CONTEXT: usize = 16;
}

impl Default for OutputOptions {
    fn default() -> Self {
        OutputOptions {
            show_colors: true,
            preferred_max_error_width: None,
        }
    }
}

/// A fully resolved diagnostic handed to a `DiagHandler`. All buffer lookups
/// have already happened, so handlers are free of the source manager.
#[derive(Clone, Debug)]
pub struct ResolvedDiagnostic {
    pub kind: DiagKind,
    pub file_name: String,
    /// 1-based line/col.
    pub line: u32,
    pub col: u32,
    pub message: String,
    /// The source line text (without buffer access), if available.
    pub source_line: Option<String>,
    /// 0-based byte `[start, end)` columns of the highlighted range within the
    /// source line, if a range was provided and the location is known.
    /// `None` when no range is present or no source line is available.
    pub range_cols: Option<(u32, u32)>,
}

/// Sink for resolved diagnostics. Default impls print; the collecting impl
/// captures for tests. Replaces the hardcoded-stderr model.
pub trait DiagHandler {
    fn handle(&mut self, diag: &ResolvedDiagnostic);
    /// Return `self` as `&dyn Any` to allow downcasting to a concrete type.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Hook to translate coordinates (e.g. via a source map) before display.
/// Port of `ICoordTranslator`.
pub trait CoordTranslator {
    fn translate(&self, coords: &mut SourceCoords);
}

/// A `DiagHandler` that records diagnostics in memory for tests.
pub struct CollectingHandler {
    messages: Vec<ResolvedDiagnostic>,
}

impl CollectingHandler {
    pub fn new() -> CollectingHandler {
        CollectingHandler {
            messages: Vec::new(),
        }
    }

    pub fn messages(&self) -> &[ResolvedDiagnostic] {
        &self.messages
    }
}

impl Default for CollectingHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagHandler for CollectingHandler {
    fn handle(&mut self, diag: &ResolvedDiagnostic) {
        self.messages.push(diag.clone());
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A warning category. Ported verbatim from `hermes/Support/Warnings.def`.
/// `NoWarning` is special and must remain the first variant.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Warning {
    /// All warnings. Special; its description is only used in the context of
    /// -Werror and -Wno-error.
    NoWarning,
    /// Warning when an undefined variable is referenced. (-Wundefined-variable)
    UndefinedVariable,
    /// Warning when attempting a direct (local) eval. (-Wdirect-eval)
    DirectEval,
    /// Warning if invoking eval() when it is disabled. (-Weval-disabled)
    EvalDisabled,
    /// Warning when require calls cannot be resolved statically.
    /// (-Wunresolved-static-require)
    UnresolvedStaticRequire,
    /// Miscellaneous warnings. (hidden: -Wmisc)
    Misc,
}

impl Warning {
    /// Number of warning categories (the C++ `_NumWarnings` sentinel).
    pub const COUNT: usize = 6;

    /// The dense 0-based index of this category, for indexing status bitsets.
    pub fn index(self) -> usize {
        self as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collecting_handler_records() {
        let mut h = CollectingHandler::new();
        h.handle(&ResolvedDiagnostic {
            kind: DiagKind::Error,
            line: 3,
            col: 5,
            file_name: "a.js".into(),
            message: "boom".into(),
            source_line: Some("  let x".into()),
            range_cols: None,
        });
        assert_eq!(h.messages().len(), 1);
        assert_eq!(h.messages()[0].kind, DiagKind::Error);
        assert_eq!((h.messages()[0].line, h.messages()[0].col), (3, 5));
    }

    #[test]
    fn output_options_defaults() {
        let o = OutputOptions::default();
        assert!(o.show_colors);
        assert_eq!(OutputOptions::TAB_STOP, 8);
    }

    #[test]
    fn warning_index_within_count() {
        assert_eq!(Warning::NoWarning.index(), 0);
        assert!(Warning::Misc.index() < Warning::COUNT);
        assert_eq!(Warning::COUNT, 6);
    }
}
