//! Token kinds, ported from include/hermes/Parser/TokenKinds.def.
//!
//! Every variant appears in the EXACT order of TokenKinds.def so that
//! `ord(kind) == kind as u16` and the range-marker predicates stay plain
//! integer comparisons — identical to the C++ lexer.

/// JavaScript token kinds.
///
/// Ported from `include/hermes/Parser/TokenKinds.def`.  The discriminant
/// of each variant matches C++ `ord(TokenKind::name)`.
///
/// Each variant's doc quotes its `.def` entry verbatim: the macro name gives
/// the category (`RESWORD`, `PUNCTUATOR`, `BINOP` with its precedence,
/// `TEMPLATE`, `IDENT_OP`) and the string gives the source spelling.
/// `RANGE_MARKER` variants are sentinels bounding a run of real kinds; the
/// lexer never produces them.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum TokenKind {
    /// `TOK(none, "<none>")` — no token; a freshly constructed `Token`.
    none,
    /// `TOK(identifier, "identifier")` — an identifier or a contextual
    /// keyword the lexer did not reclassify.
    identifier,
    /// `TOK(private_identifier, "private identifier")` — a `#name` private
    /// class element name (the `#` is not part of the identifier text).
    private_identifier,

    /// Exclusive lower bound of the reserved-word range
    /// (`RANGE_MARKER(_first_resword)`); never produced by the lexer.
    _first_resword,
    /// `RESWORD(function)`
    rw_function,
    /// `RESWORD(for)`
    rw_for,
    /// `RESWORD(if)`
    rw_if,
    /// `RESWORD(in)`
    rw_in,
    /// `RESWORD(var)`
    rw_var,
    /// `RESWORD(break)`
    rw_break,
    /// `RESWORD(continue)`
    rw_continue,
    /// `RESWORD(return)`
    rw_return,
    /// `RESWORD(switch)`
    rw_switch,
    /// `RESWORD(this)`
    rw_this,

    /// `RESWORD(true)`
    rw_true,
    /// `RESWORD(false)`
    rw_false,
    /// `RESWORD(null)`
    rw_null,
    /// `RESWORD(case)`
    rw_case,
    /// `RESWORD(catch)`
    rw_catch,
    /// `RESWORD(const)`
    rw_const,
    /// `RESWORD(debugger)`
    rw_debugger,
    /// `RESWORD(default)`
    rw_default,
    /// `RESWORD(delete)`
    rw_delete,
    /// `RESWORD(do)`
    rw_do,
    /// `RESWORD(else)`
    rw_else,
    /// `RESWORD(finally)`
    rw_finally,
    /// `RESWORD(instanceof)`
    rw_instanceof,
    /// `RESWORD(new)`
    rw_new,
    /// `RESWORD(throw)`
    rw_throw,
    /// `RESWORD(try)`
    rw_try,
    /// `RESWORD(typeof)`
    rw_typeof,
    /// `RESWORD(void)`
    rw_void,
    /// `RESWORD(while)`
    rw_while,
    /// `RESWORD(with)`
    rw_with,

    /// `RESWORD(export)`
    rw_export,
    /// `RESWORD(import)`
    rw_import,

    /// `RESWORD(class)`
    rw_class,
    /// `RESWORD(static)`
    rw_static,
    /// `RESWORD(extends)`
    rw_extends,
    /// `RESWORD(super)`
    rw_super,

    // Future reserved words
    /// `RESWORD(enum)`
    rw_enum,

    // Strict mode future reserved words
    /// `RESWORD(implements)`
    rw_implements,
    /// `RESWORD(interface)`
    rw_interface,
    /// `RESWORD(package)`
    rw_package,
    /// `RESWORD(private)`
    rw_private,
    /// `RESWORD(protected)`
    rw_protected,
    /// `RESWORD(public)`
    rw_public,
    /// `RESWORD(yield)`
    rw_yield,
    /// Exclusive upper bound of the reserved-word range
    /// (`RANGE_MARKER(_last_resword)`); never produced by the lexer.
    _last_resword,

    /// `PUNCTUATOR(l_brace, "{")`
    l_brace,
    /// `PUNCTUATOR_FLOW(l_bracepipe, "{|")` — opens a Flow exact object
    /// type; only scanned as one token in `GrammarContext::Type`.
    l_bracepipe,
    /// `PUNCTUATOR(r_brace, "}")`
    r_brace,
    /// `PUNCTUATOR_FLOW(piper_brace, "|}")` — closes a Flow exact object
    /// type; only scanned as one token in `GrammarContext::Type`.
    piper_brace,
    /// `PUNCTUATOR(l_paren, "(")`
    l_paren,
    /// `PUNCTUATOR(r_paren, ")")`
    r_paren,
    /// `PUNCTUATOR(l_square, "[")`
    l_square,
    /// `PUNCTUATOR(r_square, "]")`
    r_square,
    /// `PUNCTUATOR(period, ".")`
    period,
    /// `PUNCTUATOR(questiondot, "?.")`
    questiondot,
    /// `PUNCTUATOR(dotdotdot, "...")`
    dotdotdot,
    /// `PUNCTUATOR(semi, ";")`
    semi,
    /// `PUNCTUATOR(comma, ",")`
    comma,
    /// `PUNCTUATOR(plusplus, "++")`
    plusplus,
    /// `PUNCTUATOR(minusminus, "--")`
    minusminus,
    /// Exclusive lower bound of the binary-operator range
    /// (`RANGE_MARKER(_first_binary)`); never produced by the lexer.
    _first_binary,
    /// `BINOP(starstar, "**", 12)`
    starstar,
    /// `BINOP(star, "*", 11)`
    star,
    /// `BINOP(percent, "%", 11)`
    percent,
    /// `BINOP(slash, "/", 11)`
    slash,
    /// `BINOP(plus, "+", 10)`
    plus,
    /// `BINOP(minus, "-", 10)`
    minus,
    /// `BINOP(lessless, "<<", 9)`
    lessless,
    /// `BINOP(greatergreater, ">>", 9)`
    greatergreater,
    /// `BINOP(greatergreatergreater, ">>>", 9)`
    greatergreatergreater,
    /// `BINOP(less, "<", 8)`
    less,
    /// `BINOP(greater, ">", 8)`
    greater,
    /// `BINOP(lessequal, "<=", 8)`
    lessequal,
    /// `BINOP(greaterequal, ">=", 8)`
    greaterequal,
    /// `BINOP(equalequal, "==", 7)`
    equalequal,
    /// `BINOP(exclaimequal, "!=", 7)`
    exclaimequal,
    /// `BINOP(equalequalequal, "===", 7)`
    equalequalequal,
    /// `BINOP(exclaimequalequal, "!==", 7)`
    exclaimequalequal,
    /// `BINOP(amp, "&", 6)`
    amp,
    /// `BINOP(caret, "^", 5)`
    caret,
    /// `BINOP(pipe, "|", 4)`
    pipe,
    /// `BINOP(ampamp, "&&", 3)`
    ampamp,
    /// `BINOP(pipepipe, "||", 2)`
    pipepipe,
    /// `BINOP(questionquestion, "??", 1)`
    questionquestion,
    /// Exclusive upper bound of the binary-operator range
    /// (`RANGE_MARKER(_last_binary)`); never produced by the lexer.
    _last_binary,
    /// `PUNCTUATOR(exclaim, "!")`
    exclaim,
    /// `PUNCTUATOR(tilde, "~")`
    tilde,
    /// `PUNCTUATOR(question, "?")`
    question,
    /// `PUNCTUATOR(colon, ":")`
    colon,
    /// `PUNCTUATOR(equal, "=")`
    equal,
    /// `PUNCTUATOR(plusequal, "+=")`
    plusequal,
    /// `PUNCTUATOR(minusequal, "-=")`
    minusequal,
    /// `PUNCTUATOR(starequal, "*=")`
    starequal,
    /// `PUNCTUATOR(starstarequal, "**=")`
    starstarequal,
    /// `PUNCTUATOR(percentequal, "%=")`
    percentequal,
    /// `PUNCTUATOR(slashequal, "/=")`
    slashequal,
    /// `PUNCTUATOR(lesslessequal, "<<=")`
    lesslessequal,
    /// `PUNCTUATOR(greatergreaterequal, ">>=")`
    greatergreaterequal,
    /// `PUNCTUATOR(greatergreatergreaterequal, ">>>=")`
    greatergreatergreaterequal,
    /// `PUNCTUATOR(ampequal, "&=")`
    ampequal,
    /// `PUNCTUATOR(pipeequal, "|=")`
    pipeequal,
    /// `PUNCTUATOR(ampampequal, "&&=")`
    ampampequal,
    /// `PUNCTUATOR(pipepipeequal, "||=")`
    pipepipeequal,
    /// `PUNCTUATOR(questionquestionequal, "\?\?=")`
    questionquestionequal,
    /// `PUNCTUATOR(caretequal, "^=")`
    caretequal,
    /// `PUNCTUATOR(equalgreater, "=>")`
    equalgreater,
    /// `PUNCTUATOR(at, "@")`
    at,

    /// `IDENT_OP(as_operator, "as", 8)` — the contextual `as` cast operator.
    /// The lexer scans `as` as an `identifier`; the parser reclassifies it
    /// with `convert_cur_token_to_ident_op` where a cast is allowed.
    as_operator,

    /// `TOK(numeric_literal, "number")` — the `f64` value is read back with
    /// `Token::get_numeric_literal`.
    numeric_literal,
    /// `TOK(string_literal, "string")` — `Token::get_string_literal` returns
    /// the cooked value, with escapes already decoded.
    string_literal,
    /// `TOK(regexp_literal, "regexp")` — `Token::get_regexp_literal` returns
    /// the body and flags; the pattern itself is not validated by the lexer.
    regexp_literal,
    /// `TOK(jsx_text, "JSX text")` — a run of literal JSX child text, only
    /// produced by `JSLexer::advance_in_jsx_child`.
    jsx_text,
    /// `TOK(bigint_literal, "bigint")` — a `123n` literal; the value is kept
    /// as text (`Token::get_bigint_literal`), never converted to `f64`.
    bigint_literal,

    /// `TEMPLATE(no_substitution_template, "template literal")` — a whole
    /// template with no substitutions.
    no_substitution_template,
    /// `TEMPLATE(template_head, "template literal start")` — the chunk up to
    /// the first `${`.
    template_head,
    /// `TEMPLATE(template_middle, "template literal resume")` — a chunk
    /// between two substitutions.
    template_middle,
    /// `TEMPLATE(template_tail, "template literal end")` — the chunk after
    /// the last substitution.
    template_tail,

    /// `TOK(eof, "<eof>")` — end of input; scanning again keeps returning it.
    eof,
    /// One past the last real token kind (`RANGE_MARKER(_last_token)`); its
    /// ordinal plus one is [`NUM_JS_TOKENS`], the size of the token tables.
    _last_token,
}

/// Number of token kinds = ord(_last_token) + 1.
pub const NUM_JS_TOKENS: usize = TokenKind::_last_token as usize + 1;

/// Human-readable names for each token kind, in `.def` order.
/// Mirrors C++ `tokenKindStr`.
const TOKEN_NAMES: [&str; NUM_JS_TOKENS] = [
    // TOK(none, "<none>")
    "<none>",
    // TOK(identifier, "identifier")
    "identifier",
    // TOK(private_identifier, "private identifier")
    "private identifier",

    // RANGE_MARKER(_first_resword)
    "<_first_resword>",
    // RESWORD(function)
    "function",
    // RESWORD(for)
    "for",
    // RESWORD(if)
    "if",
    // RESWORD(in)
    "in",
    // RESWORD(var)
    "var",
    // RESWORD(break)
    "break",
    // RESWORD(continue)
    "continue",
    // RESWORD(return)
    "return",
    // RESWORD(switch)
    "switch",
    // RESWORD(this)
    "this",

    // RESWORD(true)
    "true",
    // RESWORD(false)
    "false",
    // RESWORD(null)
    "null",
    // RESWORD(case)
    "case",
    // RESWORD(catch)
    "catch",
    // RESWORD(const)
    "const",
    // RESWORD(debugger)
    "debugger",
    // RESWORD(default)
    "default",
    // RESWORD(delete)
    "delete",
    // RESWORD(do)
    "do",
    // RESWORD(else)
    "else",
    // RESWORD(finally)
    "finally",
    // RESWORD(instanceof)
    "instanceof",
    // RESWORD(new)
    "new",
    // RESWORD(throw)
    "throw",
    // RESWORD(try)
    "try",
    // RESWORD(typeof)
    "typeof",
    // RESWORD(void)
    "void",
    // RESWORD(while)
    "while",
    // RESWORD(with)
    "with",

    // RESWORD(export)
    "export",
    // RESWORD(import)
    "import",

    // RESWORD(class)
    "class",
    // RESWORD(static)
    "static",
    // RESWORD(extends)
    "extends",
    // RESWORD(super)
    "super",

    // Future reserved words
    // RESWORD(enum)
    "enum",

    // Strict mode future reserved words
    // RESWORD(implements)
    "implements",
    // RESWORD(interface)
    "interface",
    // RESWORD(package)
    "package",
    // RESWORD(private)
    "private",
    // RESWORD(protected)
    "protected",
    // RESWORD(public)
    "public",
    // RESWORD(yield)
    "yield",
    // RANGE_MARKER(_last_resword)
    "<_last_resword>",

    // PUNCTUATOR(l_brace, "{")
    "{",
    // PUNCTUATOR_FLOW(l_bracepipe, "{|")
    "{|",
    // PUNCTUATOR(r_brace, "}")
    "}",
    // PUNCTUATOR_FLOW(piper_brace, "|}")
    "|}",
    // PUNCTUATOR(l_paren, "(")
    "(",
    // PUNCTUATOR(r_paren, ")")
    ")",
    // PUNCTUATOR(l_square, "[")
    "[",
    // PUNCTUATOR(r_square, "]")
    "]",
    // PUNCTUATOR(period, ".")
    ".",
    // PUNCTUATOR(questiondot, "?.")
    "?.",
    // PUNCTUATOR(dotdotdot, "...")
    "...",
    // PUNCTUATOR(semi, ";")
    ";",
    // PUNCTUATOR(comma, ",")
    ",",
    // PUNCTUATOR(plusplus, "++")
    "++",
    // PUNCTUATOR(minusminus, "--")
    "--",
    // RANGE_MARKER(_first_binary)
    "<_first_binary>",
    // BINOP(starstar, "**", 12)
    "**",
    // BINOP(star, "*", 11)
    "*",
    // BINOP(percent, "%", 11)
    "%",
    // BINOP(slash, "/", 11)
    "/",
    // BINOP(plus, "+", 10)
    "+",
    // BINOP(minus, "-", 10)
    "-",
    // BINOP(lessless, "<<", 9)
    "<<",
    // BINOP(greatergreater, ">>", 9)
    ">>",
    // BINOP(greatergreatergreater, ">>>", 9)
    ">>>",
    // BINOP(less, "<", 8)
    "<",
    // BINOP(greater, ">", 8)
    ">",
    // BINOP(lessequal, "<=", 8)
    "<=",
    // BINOP(greaterequal, ">=", 8)
    ">=",
    // BINOP(equalequal, "==", 7)
    "==",
    // BINOP(exclaimequal, "!=", 7)
    "!=",
    // BINOP(equalequalequal, "===", 7)
    "===",
    // BINOP(exclaimequalequal, "!==", 7)
    "!==",
    // BINOP(amp, "&", 6)
    "&",
    // BINOP(caret, "^", 5)
    "^",
    // BINOP(pipe, "|", 4)
    "|",
    // BINOP(ampamp, "&&", 3)
    "&&",
    // BINOP(pipepipe, "||", 2)
    "||",
    // BINOP(questionquestion, "??", 1)
    "??",
    // RANGE_MARKER(_last_binary)
    "<_last_binary>",
    // PUNCTUATOR(exclaim, "!")
    "!",
    // PUNCTUATOR(tilde, "~")
    "~",
    // PUNCTUATOR(question, "?")
    "?",
    // PUNCTUATOR(colon, ":")
    ":",
    // PUNCTUATOR(equal, "=")
    "=",
    // PUNCTUATOR(plusequal, "+=")
    "+=",
    // PUNCTUATOR(minusequal, "-=")
    "-=",
    // PUNCTUATOR(starequal, "*=")
    "*=",
    // PUNCTUATOR(starstarequal, "**=")
    "**=",
    // PUNCTUATOR(percentequal, "%=")
    "%=",
    // PUNCTUATOR(slashequal, "/=")
    "/=",
    // PUNCTUATOR(lesslessequal, "<<=")
    "<<=",
    // PUNCTUATOR(greatergreaterequal, ">>=")
    ">>=",
    // PUNCTUATOR(greatergreatergreaterequal, ">>>=")
    ">>>=",
    // PUNCTUATOR(ampequal, "&=")
    "&=",
    // PUNCTUATOR(pipeequal, "|=")
    "|=",
    // PUNCTUATOR(ampampequal, "&&=")
    "&&=",
    // PUNCTUATOR(pipepipeequal, "||=")
    "||=",
    // PUNCTUATOR(questionquestionequal, "\?\?=")
    "??=",
    // PUNCTUATOR(caretequal, "^=")
    "^=",
    // PUNCTUATOR(equalgreater, "=>")
    "=>",
    // PUNCTUATOR(at, "@")
    "@",

    // IDENT_OP(as_operator, "as", 8)
    "as",

    // TOK(numeric_literal, "number")
    "number",
    // TOK(string_literal, "string")
    "string",
    // TOK(regexp_literal, "regexp")
    "regexp",
    // TOK(jsx_text, "JSX text")
    "JSX text",
    // TOK(bigint_literal, "bigint")
    "bigint",

    // TEMPLATE(no_substitution_template, "template literal")
    "template literal",
    // TEMPLATE(template_head, "template literal start")
    "template literal start",
    // TEMPLATE(template_middle, "template literal resume")
    "template literal resume",
    // TEMPLATE(template_tail, "template literal end")
    "template literal end",

    // TOK(eof, "<eof>")
    "<eof>",
    // RANGE_MARKER(_last_token)
    "<_last_token>",
];

/// The `.def` variant name for each token kind, in `.def` order. This is the
/// `#name` string the C++ `tokenVariantName` switch returns (e.g. `l_brace`,
/// `starstar`, `rw_function`, `eof`), as used by the `js-lexer-dump` oracle —
/// distinct from `TOKEN_NAMES` (the human-readable spelling, e.g. "{").
const TOKEN_VARIANT_NAMES: [&str; NUM_JS_TOKENS] = [
    "none",
    "identifier",
    "private_identifier",
    "_first_resword",
    "rw_function",
    "rw_for",
    "rw_if",
    "rw_in",
    "rw_var",
    "rw_break",
    "rw_continue",
    "rw_return",
    "rw_switch",
    "rw_this",
    "rw_true",
    "rw_false",
    "rw_null",
    "rw_case",
    "rw_catch",
    "rw_const",
    "rw_debugger",
    "rw_default",
    "rw_delete",
    "rw_do",
    "rw_else",
    "rw_finally",
    "rw_instanceof",
    "rw_new",
    "rw_throw",
    "rw_try",
    "rw_typeof",
    "rw_void",
    "rw_while",
    "rw_with",
    "rw_export",
    "rw_import",
    "rw_class",
    "rw_static",
    "rw_extends",
    "rw_super",
    "rw_enum",
    "rw_implements",
    "rw_interface",
    "rw_package",
    "rw_private",
    "rw_protected",
    "rw_public",
    "rw_yield",
    "_last_resword",
    "l_brace",
    "l_bracepipe",
    "r_brace",
    "piper_brace",
    "l_paren",
    "r_paren",
    "l_square",
    "r_square",
    "period",
    "questiondot",
    "dotdotdot",
    "semi",
    "comma",
    "plusplus",
    "minusminus",
    "_first_binary",
    "starstar",
    "star",
    "percent",
    "slash",
    "plus",
    "minus",
    "lessless",
    "greatergreater",
    "greatergreatergreater",
    "less",
    "greater",
    "lessequal",
    "greaterequal",
    "equalequal",
    "exclaimequal",
    "equalequalequal",
    "exclaimequalequal",
    "amp",
    "caret",
    "pipe",
    "ampamp",
    "pipepipe",
    "questionquestion",
    "_last_binary",
    "exclaim",
    "tilde",
    "question",
    "colon",
    "equal",
    "plusequal",
    "minusequal",
    "starequal",
    "starstarequal",
    "percentequal",
    "slashequal",
    "lesslessequal",
    "greatergreaterequal",
    "greatergreatergreaterequal",
    "ampequal",
    "pipeequal",
    "ampampequal",
    "pipepipeequal",
    "questionquestionequal",
    "caretequal",
    "equalgreater",
    "at",
    "as_operator",
    "numeric_literal",
    "string_literal",
    "regexp_literal",
    "jsx_text",
    "bigint_literal",
    "no_substitution_template",
    "template_head",
    "template_middle",
    "template_tail",
    "eof",
    "_last_token",
];

/// \return the `.def` variant name of `kind` (e.g. `l_brace`, `eof`), matching
/// the C++ `tokenVariantName` switch used by `js-lexer-dump`.
#[inline]
pub fn variant_name(kind: TokenKind) -> &'static str {
    TOKEN_VARIANT_NAMES[kind as usize]
}

/// Binary-operator precedences in `.def` order (0 = not a binary operator).
/// Mirrors C++ `BINOP` precedence field.
///
/// IDENT_OP precedences (e.g. `as_operator`) are stored here for completeness
/// but are not exposed by `binop_precedence`, which is gated to the binary
/// marker range; the parser layer will read them directly from this table.
const TOKEN_PREC: [u8; NUM_JS_TOKENS] = [
    // none
    0,
    // identifier
    0,
    // private_identifier
    0,

    // _first_resword
    0,
    // rw_function .. rw_yield (44 entries)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // function..this
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // true..do
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // else..with
    0, 0, // export, import
    0, 0, 0, 0, // class, static, extends, super
    0,          // enum
    0, 0, 0, 0, 0, 0, 0, // implements..yield
    // _last_resword
    0,

    // l_brace .. minusminus (15 entries)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    // _first_binary
    0,
    // BINOP precedences (23 entries)
    12, // starstar
    11, // star
    11, // percent
    11, // slash
    10, // plus
    10, // minus
    9,  // lessless
    9,  // greatergreater
    9,  // greatergreatergreater
    8,  // less
    8,  // greater
    8,  // lessequal
    8,  // greaterequal
    7,  // equalequal
    7,  // exclaimequal
    7,  // equalequalequal
    7,  // exclaimequalequal
    6,  // amp
    5,  // caret
    4,  // pipe
    3,  // ampamp
    2,  // pipepipe
    1,  // questionquestion
    // _last_binary
    0,
    // exclaim .. at (22 entries)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,

    // IDENT_OP(as_operator, "as", 8)
    8,

    // numeric_literal .. bigint_literal (5 entries)
    0, 0, 0, 0, 0,
    // template tokens (4 entries)
    0, 0, 0, 0,
    // eof
    0,
    // _last_token
    0,
];

/// \return the integer ordinal of `kind` (matches C++ `ord`).
#[inline]
pub const fn ord(kind: TokenKind) -> i32 {
    kind as u16 as i32
}

/// \return the human-readable name of `kind` (matches C++ `tokenKindStr`).
#[inline]
pub fn token_kind_str(kind: TokenKind) -> &'static str {
    TOKEN_NAMES[kind as usize]
}

/// \return the human-readable name for the token kind with the given ordinal.
/// Used by the lexer to pre-intern reserved words by ordinal without needing to
/// reconstruct a `TokenKind` value (which would require an unsafe transmute).
#[inline]
pub fn token_kind_str_by_ord(ord: i32) -> &'static str {
    TOKEN_NAMES[ord as usize]
}

/// \return the binary-operator precedence of `kind`, or None if not a binary op.
#[inline]
pub fn binop_precedence(kind: TokenKind) -> Option<u8> {
    if kind.in_binary_range() {
        Some(TOKEN_PREC[kind as usize])
    } else {
        None
    }
}

impl TokenKind {
    /// True if this is a reserved word (strictly between the range markers).
    #[inline]
    pub fn is_res_word(self) -> bool {
        (self as u16) > (TokenKind::_first_resword as u16)
            && (self as u16) < (TokenKind::_last_resword as u16)
    }

    /// True if `self` is strictly inside the binary-operator range markers.
    #[inline]
    fn in_binary_range(self) -> bool {
        (self as u16) > (TokenKind::_first_binary as u16)
            && (self as u16) < (TokenKind::_last_binary as u16)
    }

    /// True if `self` is a punctuator (mirrors C++ `isPunctuatorDbg`).
    ///
    /// The contiguous punctuator run in TokenKinds.def spans `l_brace` through
    /// `at`; the `_first_binary` and `_last_binary` range markers sit inside
    /// that run but are NOT true punctuators, so we exclude them explicitly.
    #[inline]
    pub fn is_punctuator(self) -> bool {
        let v = self as u16;
        v >= (TokenKind::l_brace as u16)
            && v <= (TokenKind::at as u16)
            && v != (TokenKind::_first_binary as u16)
            && v != (TokenKind::_last_binary as u16)
    }
}

/// Recognise a reserved word by its bytes (mirrors `matchReservedWord` in
/// JSLexer.cpp).  Returns `TokenKind::identifier` if `bytes` is not a reserved
/// word.  Pure: no strict-mode filtering (that lives in lexer-core's
/// `scanReservedWord`).
pub fn match_reserved_word(bytes: &[u8]) -> TokenKind {
    match bytes {
        // RESWORD(function)
        b"function" => TokenKind::rw_function,
        // RESWORD(for)
        b"for" => TokenKind::rw_for,
        // RESWORD(if)
        b"if" => TokenKind::rw_if,
        // RESWORD(in)
        b"in" => TokenKind::rw_in,
        // RESWORD(var)
        b"var" => TokenKind::rw_var,
        // RESWORD(break)
        b"break" => TokenKind::rw_break,
        // RESWORD(continue)
        b"continue" => TokenKind::rw_continue,
        // RESWORD(return)
        b"return" => TokenKind::rw_return,
        // RESWORD(switch)
        b"switch" => TokenKind::rw_switch,
        // RESWORD(this)
        b"this" => TokenKind::rw_this,

        // RESWORD(true)
        b"true" => TokenKind::rw_true,
        // RESWORD(false)
        b"false" => TokenKind::rw_false,
        // RESWORD(null)
        b"null" => TokenKind::rw_null,
        // RESWORD(case)
        b"case" => TokenKind::rw_case,
        // RESWORD(catch)
        b"catch" => TokenKind::rw_catch,
        // RESWORD(const)
        b"const" => TokenKind::rw_const,
        // RESWORD(debugger)
        b"debugger" => TokenKind::rw_debugger,
        // RESWORD(default)
        b"default" => TokenKind::rw_default,
        // RESWORD(delete)
        b"delete" => TokenKind::rw_delete,
        // RESWORD(do)
        b"do" => TokenKind::rw_do,
        // RESWORD(else)
        b"else" => TokenKind::rw_else,
        // RESWORD(finally)
        b"finally" => TokenKind::rw_finally,
        // RESWORD(instanceof)
        b"instanceof" => TokenKind::rw_instanceof,
        // RESWORD(new)
        b"new" => TokenKind::rw_new,
        // RESWORD(throw)
        b"throw" => TokenKind::rw_throw,
        // RESWORD(try)
        b"try" => TokenKind::rw_try,
        // RESWORD(typeof)
        b"typeof" => TokenKind::rw_typeof,
        // RESWORD(void)
        b"void" => TokenKind::rw_void,
        // RESWORD(while)
        b"while" => TokenKind::rw_while,
        // RESWORD(with)
        b"with" => TokenKind::rw_with,

        // RESWORD(export)
        b"export" => TokenKind::rw_export,
        // RESWORD(import)
        b"import" => TokenKind::rw_import,

        // RESWORD(class)
        b"class" => TokenKind::rw_class,
        // RESWORD(static)
        b"static" => TokenKind::rw_static,
        // RESWORD(extends)
        b"extends" => TokenKind::rw_extends,
        // RESWORD(super)
        b"super" => TokenKind::rw_super,

        // Future reserved words
        // RESWORD(enum)
        b"enum" => TokenKind::rw_enum,

        // Strict mode future reserved words
        // RESWORD(implements)
        b"implements" => TokenKind::rw_implements,
        // RESWORD(interface)
        b"interface" => TokenKind::rw_interface,
        // RESWORD(package)
        b"package" => TokenKind::rw_package,
        // RESWORD(private)
        b"private" => TokenKind::rw_private,
        // RESWORD(protected)
        b"protected" => TokenKind::rw_protected,
        // RESWORD(public)
        b"public" => TokenKind::rw_public,
        // RESWORD(yield)
        b"yield" => TokenKind::rw_yield,

        _ => TokenKind::identifier,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinals_and_count() {
        assert_eq!(TokenKind::none as u16, 0);
        assert_eq!(TokenKind::identifier as u16, 1);
        assert_eq!(TokenKind::private_identifier as u16, 2);
        assert_eq!(TokenKind::_last_token as u16, 122);
        assert_eq!(NUM_JS_TOKENS, 123);
        assert_eq!(ord(TokenKind::eof), TokenKind::eof as u16 as i32);
    }

    #[test]
    fn resword_range() {
        assert!(TokenKind::rw_function.is_res_word());
        assert!(TokenKind::rw_yield.is_res_word());
        assert!(!TokenKind::identifier.is_res_word());
        assert!(!TokenKind::l_brace.is_res_word());
        assert!(!TokenKind::_first_resword.is_res_word());
        assert!(!TokenKind::_last_resword.is_res_word());
    }

    #[test]
    fn names_match_def() {
        assert_eq!(token_kind_str(TokenKind::none), "<none>");
        assert_eq!(token_kind_str(TokenKind::identifier), "identifier");
        assert_eq!(token_kind_str(TokenKind::private_identifier), "private identifier");
        assert_eq!(token_kind_str(TokenKind::rw_function), "function");
        assert_eq!(token_kind_str(TokenKind::l_brace), "{");
        assert_eq!(token_kind_str(TokenKind::starstar), "**");
        assert_eq!(token_kind_str(TokenKind::as_operator), "as");
        assert_eq!(token_kind_str(TokenKind::eof), "<eof>");
        assert_eq!(token_kind_str(TokenKind::_first_resword), "<_first_resword>");
    }

    #[test]
    fn token_kind_str_by_ord_keywords() {
        assert_eq!(token_kind_str_by_ord(ord(TokenKind::rw_function)), "function");
        assert_eq!(token_kind_str_by_ord(ord(TokenKind::rw_yield)), "yield");
        assert_eq!(
            token_kind_str_by_ord(ord(TokenKind::identifier)),
            "identifier"
        );
    }

    #[test]
    fn variant_names_match_def() {
        assert_eq!(variant_name(TokenKind::none), "none");
        assert_eq!(variant_name(TokenKind::l_brace), "l_brace");
        assert_eq!(variant_name(TokenKind::starstar), "starstar");
        assert_eq!(variant_name(TokenKind::rw_function), "rw_function");
        assert_eq!(variant_name(TokenKind::questionquestionequal), "questionquestionequal");
        assert_eq!(variant_name(TokenKind::eof), "eof");
        assert_eq!(variant_name(TokenKind::_last_token), "_last_token");
    }

    #[test]
    fn binop_precedence_table() {
        assert_eq!(binop_precedence(TokenKind::starstar), Some(12));
        assert_eq!(binop_precedence(TokenKind::star), Some(11));
        assert_eq!(binop_precedence(TokenKind::plus), Some(10));
        assert_eq!(binop_precedence(TokenKind::lessless), Some(9));
        assert_eq!(binop_precedence(TokenKind::less), Some(8));
        assert_eq!(binop_precedence(TokenKind::equalequal), Some(7));
        assert_eq!(binop_precedence(TokenKind::amp), Some(6));
        assert_eq!(binop_precedence(TokenKind::caret), Some(5));
        assert_eq!(binop_precedence(TokenKind::pipe), Some(4));
        assert_eq!(binop_precedence(TokenKind::ampamp), Some(3));
        assert_eq!(binop_precedence(TokenKind::pipepipe), Some(2));
        assert_eq!(binop_precedence(TokenKind::questionquestion), Some(1));
        assert_eq!(binop_precedence(TokenKind::l_brace), None);
        assert_eq!(binop_precedence(TokenKind::eof), None);
        // Range markers inside the binary span and the IDENT_OP `as` have no binop precedence.
        assert_eq!(binop_precedence(TokenKind::_first_binary), None);
        assert_eq!(binop_precedence(TokenKind::_last_binary), None);
        assert_eq!(binop_precedence(TokenKind::as_operator), None);
    }

    #[test]
    fn punctuator_predicate() {
        assert!(TokenKind::l_brace.is_punctuator());
        assert!(TokenKind::starstar.is_punctuator());
        assert!(TokenKind::at.is_punctuator());
        assert!(!TokenKind::identifier.is_punctuator());
        assert!(!TokenKind::rw_function.is_punctuator());
        assert!(!TokenKind::numeric_literal.is_punctuator());
        assert!(!TokenKind::as_operator.is_punctuator());
        // Range markers that fall inside the punctuator span must NOT be punctuators.
        assert!(!TokenKind::_first_binary.is_punctuator());
        assert!(!TokenKind::_last_binary.is_punctuator());
        // PUNCTUATOR_FLOW tokens are treated as punctuators here (C++ isPunctuatorDbg
        // returns false for them because it only defines PUNCTUATOR).
        assert!(TokenKind::l_bracepipe.is_punctuator());
        assert!(TokenKind::piper_brace.is_punctuator());
    }

    #[test]
    fn reserved_words() {
        assert_eq!(match_reserved_word(b"function"), TokenKind::rw_function);
        assert_eq!(match_reserved_word(b"yield"), TokenKind::rw_yield);
        assert_eq!(match_reserved_word(b"static"), TokenKind::rw_static);
        assert_eq!(match_reserved_word(b"extends"), TokenKind::rw_extends);
        assert_eq!(match_reserved_word(b"fora"), TokenKind::identifier);
        assert_eq!(match_reserved_word(b"Function"), TokenKind::identifier);
        assert_eq!(match_reserved_word(b""), TokenKind::identifier);
        assert_eq!(match_reserved_word(b"let"), TokenKind::identifier);
    }
}
