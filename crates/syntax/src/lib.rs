//! Language-neutral design facts extracted from source files.
//!
//! [`parse`] turns one TypeScript, JavaScript, Vue, Python, PHP or Rust file into a
//! [`FileFacts`]: functions, types, fields, calls, comments, ignored errors, local
//! names and duplicate-detection fragments. Everything a design rule needs is here,
//! so rule code never sees a syntax tree or a grammar.

mod comment;
mod fragment;
mod lang;
mod walk;

pub use lang::Lang;

/// An inclusive range of 1-based line numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    /// True when `line` falls inside this span.
    #[must_use]
    pub fn contains(self, line: u32) -> bool {
        self.start <= line && line <= self.end
    }
}

/// Everything extracted from one file. Indices in `doc` fields and `Local::function`
/// point into the vectors of the same `FileFacts`.
#[derive(Debug, Default)]
pub struct FileFacts {
    pub functions: Vec<Function>,
    pub types: Vec<TypeDecl>,
    pub fields: Vec<Field>,
    pub calls: Vec<Call>,
    pub comments: Vec<Comment>,
    pub ignored_errors: Vec<IgnoredError>,
    pub locals: Vec<Local>,
    /// Raw text of every import, `use` or `from … import` target.
    pub imports: Vec<String>,
    pub fragments: Vec<Fragment>,
}

/// A function, method or named arrow function with a body.
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    /// Enclosing class, impl target, trait or struct name.
    pub owner: Option<String>,
    pub span: Span,
    /// Declaration text before the body: the signature as a caller sees it.
    pub header: String,
    pub header_span: Span,
    /// Visible to other modules: exported, `pub`, `public`, or not `_`-prefixed.
    pub public: bool,
    /// May implement an interface or override a parent: the method lives in a class
    /// with `extends`/`implements`/base classes, or in `impl Trait for T`.
    pub overrides: bool,
    /// Parameter names in order, without `self`, `this` or `$` sigils.
    pub params: Vec<String>,
    /// Declares a non-void return type or returns a value somewhere in its body.
    pub returns_value: bool,
    /// Declares or returns a tuple (`[A, B]`, `(A, B)`, `tuple[...]`, `return a, b`).
    pub returns_tuple: bool,
    /// Number of top-level statements in the body, excluding comments and docstrings.
    pub statements: u32,
    pub body: Body,
    /// Index into [`FileFacts::comments`] of the interface comment.
    pub doc: Option<usize>,
}

/// The shape of a one-statement body. Anything else is [`Body::Other`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// The only statement calls another function, optionally returning its result.
    Forward(Call),
    /// The only statement returns a field of `this`/`self`.
    ReturnsField(String),
    Other,
}

/// A class, interface, trait, struct or enum declaration.
#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub name: String,
    pub span: Span,
    pub header: String,
    pub public: bool,
    pub doc: Option<usize>,
}

/// A field or property declared on a type.
#[derive(Debug, Clone)]
pub struct Field {
    pub owner: String,
    pub name: String,
    pub public: bool,
    /// Holds a mutable collection: list, array, map, set, `Vec`, `HashMap`...
    pub collection: bool,
    pub line: u32,
}

/// One call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// Index into [`FileFacts::functions`] of the innermost enclosing function.
    pub caller: Option<usize>,
    /// Last path segment of the called name: `insert` for `this.area.insert`.
    pub callee: String,
    /// Text before the callee: `this.area` for `this.area.insert`, empty for `f()`.
    pub receiver: String,
    pub args: Vec<Arg>,
    pub line: u32,
}

/// One call argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arg {
    /// The prop name of a JSX attribute; `None` for a positional argument.
    pub name: Option<String>,
    /// Source text, without `$` sigils.
    pub text: String,
    /// A string, number, boolean or null literal.
    pub literal: bool,
}

/// A comment block: consecutive line comments are merged into one.
#[derive(Debug, Clone)]
pub struct Comment {
    pub span: Span,
    /// Text without comment markers.
    pub text: String,
    /// An interface comment: `/** */`, `///`, `//!` or a docstring.
    pub doc: bool,
    /// The code this comment describes: the declaration header for interface
    /// comments, the rest of the line for trailing comments, otherwise up to three
    /// lines of the statements that directly follow it.
    pub code: String,
}

impl Comment {
    /// The first paragraph, before any blank line or tag section (`@param`,
    /// `Args:`, `# Errors`). This is the description of the thing documented.
    #[must_use]
    pub fn summary(&self) -> &str {
        comment::summary(&self.text)
    }
}

/// An error handler that discards the error: an empty `catch`, `except: pass`,
/// `let _ = f()` or `f().ok();`.
#[derive(Debug, Clone)]
pub struct IgnoredError {
    pub line: u32,
    /// The single call whose error is discarded, when the guarded code is one call.
    pub callee: Option<String>,
}

/// A parameter or local variable and how far its uses reach.
#[derive(Debug, Clone)]
pub struct Local {
    /// Index into [`FileFacts::functions`].
    pub function: usize,
    pub name: String,
    pub declared: u32,
    pub last_use: u32,
}

/// A statement of three or more lines with code, hashed with identifiers and
/// literals erased, so copies that differ only in names hash the same.
#[derive(Debug, Clone)]
pub struct Fragment {
    pub hash: u64,
    pub span: Span,
    /// Lines that contain code.
    pub lines: u32,
    /// Distinct identifiers used: the parameter count an extracted function could need.
    pub identifiers: u32,
}

/// Extracts the facts of one file. Returns `None` when the extension is not a
/// supported language. `path` only selects the language; it is not read.
#[must_use]
pub fn parse(path: &str, source: &str) -> Option<FileFacts> {
    let lang = Lang::for_path(path)?;
    let source = match lang::vue_script(path, source) {
        Some(script) => script,
        None => source.to_owned(),
    };
    Some(walk::extract(lang, &source))
}
