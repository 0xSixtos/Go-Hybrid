//! hy-check — the Tier-0 checker (SPEC-LANGUAGE.md §9).
//!
//! Public API (consumed by the `hyb` CLI and the corpus harness):
//!   * `check_file(&File) -> CheckResult` — diagnostics + verdict + managed report
//!
//! Implementation status: stub (agent 5-c fills `rules/`).

use hy_diag::{Diagnostic, ManagedReport};
use hy_syntax::File;

/// The result of checking one file.
#[derive(Debug, Default)]
pub struct CheckResult {
    pub diagnostics: Vec<Diagnostic>,
    /// `accept` when no errors remain (warnings allowed), else `reject`.
    pub verdict: Verdict,
    pub managed: ManagedReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verdict {
    #[default]
    Accept,
    Reject,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Accept => "accept",
            Verdict::Reject => "reject",
        }
    }
}

/// Check one parsed file against the Tier-0 rules (O2/O4/O5/O6/O7/O8/O12...).
///
/// STUB: accepts everything until the rules engine lands. The checker agent
/// replaces this — the signature is the contract.
pub fn check_file(file: &File) -> CheckResult {
    let _ = file;
    CheckResult {
        diagnostics: Vec::new(),
        verdict: Verdict::Accept,
        managed: ManagedReport::default(),
    }
}
