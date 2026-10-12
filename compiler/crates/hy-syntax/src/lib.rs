//! hy-syntax — lexer, parser, and AST for the Hybrid language.
//!
//! Public API (the walking-skeleton surface):
//!   * `parse_file(path, src) -> Result<File, Vec<Diagnostic>>`
//!   * `File`, `Decl`, `Stmt`, `Expr`, `TypeExpr`, ... (re-exported from ast)
//!
//! Implementation status:
//!   * `ast` — the contract (complete; owned by the lead)
//!   * `lexer` / `parser` — being implemented (agent 5-b)

mod ast;

pub use ast::*;

use hy_diag::{Diagnostic, Span};

/// Parse a whole source file. On success returns the AST; on failure returns
/// every parse diagnostic collected (parse errors are `HYB-E0xxx` codes).
///
/// STUB: returns an empty file until the parser lands. The parser agent
/// replaces this — the signature is the contract.
pub fn parse_file(path: &str, src: &str) -> Result<File, Vec<Diagnostic>> {
    let _ = (path, src);
    Err(vec![Diagnostic::error(
        "HYB-E0001",
        path.to_string(),
        Span::point(hy_diag::Pos::new(1, 1)),
        "parser not yet implemented (hy-syntax stub)",
    )])
}
