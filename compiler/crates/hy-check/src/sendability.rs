//! sendability.rs — rule A1c (O4 classes) at spawn/send check-sites, plus the
//! O6 `managed` suppression and lint (HYB-E8001).
//!
//! Outcome per value (task brief, VERDICTS.md sketches):
//!   * OK classes: primitives; string/const/immutable; chan-typed values;
//!     `move x`; fresh literals whose reference-typed parts are all sendable
//!     (O4.6 deep sendability); atomic values and `&atomic_binding`;
//!     managed-tainted values (class 6).
//!   * REJECT: a named mutable GC binding (map/slice/struct/pointer-to-
//!     mutable), `&mutable_struct_binding`, borrow-handles, closures.

use crate::model::{
    as_addr_of, move_operand_ident, Binding, Checker, FnState, MoveKind, TypeShape,
};
use hy_diag::{Diagnostic, Span};
use hy_syntax::Expr;

/// The classification of one value at a check-site.
pub(crate) enum Outcome {
    /// Sendable via classes 1-5 (or already-reported use-after-move).
    Ok,
    /// Resolved via class 6 (managed taint / suppression) — sendable.
    /// `risky` marks values that are NOT also safe-by-type (mutable GC
    /// state): only those carry the audit lint and the class-6 site count —
    /// a tainted channel or primitive crossing a site is not the trust
    /// surface an auditor needs named (O6.3: the lint carries WHAT is
    /// shared and WHY it is believed safe).
    Managed { desc: String, risky: bool },
    /// Not sendable (A1c).
    Rejected(RejectInfo),
}

pub(crate) struct RejectInfo {
    /// The value, as written ("counts", "&stats", "shared_map").
    pub value: String,
    /// The value's type, for the message ("map[string]int", "*pool_stats").
    pub ty: String,
    pub notes: Vec<String>,
}

impl<'a> Checker<'a> {
    // -----------------------------------------------------------------------
    // Classification
    // -----------------------------------------------------------------------

    pub(crate) fn classify_value(&self, st: &FnState, expr: &Expr) -> Outcome {
        // `move x` — class 3; the transfer marking happens at the site.
        if matches!(expr, Expr::Move { .. }) {
            return Outcome::Ok;
        }
        // literals and interpolated strings
        if matches!(expr, Expr::Lit { .. } | Expr::Interp { .. }) {
            return Outcome::Ok;
        }
        // closures capture their environment — not sendable in Tier 0
        if matches!(expr, Expr::FnLit { .. }) {
            return Outcome::Rejected(RejectInfo {
                value: expr.render(),
                ty: "closure".to_string(),
                notes: vec![
                    "closures capture their environment; they are not sendable at spawn/send sites in Tier 0 (closure escapes are the A5 perimeter)".to_string(),
                ],
            });
        }
        // `&binding` — the pointer shape
        if let Some(operand) = as_addr_of(expr) {
            return self.classify_addr_of(st, expr, operand);
        }
        match expr {
            Expr::Ident { name, .. } => self.classify_ident(st, name),
            Expr::StructLit { fields, .. } => {
                self.classify_composite(st, expr, fields.iter().map(|f| &f.value))
            }
            Expr::MapLit { entries, .. } => {
                let mut parts = Vec::new();
                for (k, v) in entries {
                    parts.push(k);
                    parts.push(v);
                }
                self.classify_composite(st, expr, parts.into_iter())
            }
            Expr::SliceLit { items, .. } => self.classify_composite(st, expr, items.iter()),
            // field/index/slice reads: classify the read's shape, but honor
            // immutable roots (views of const values stay immutable, OQ5)
            Expr::Field { .. } | Expr::Index { .. } | Expr::Slice { .. } => {
                if let Some(root) = expr.root_ident() {
                    if let Some(b) = self.lookup_binding(st, root) {
                        if b.moved.is_some() {
                            return Outcome::Ok; // use-after-move already reported
                        }
                        if b.managed_taint {
                            return Outcome::Managed {
                                desc: self.managed_desc(root, b),
                                risky: !b.shape.is_sendable_by_type(),
                            };
                        }
                        if b.immutable {
                            return Outcome::Ok;
                        }
                    }
                }
                let shape = self.shape_of_expr(st, expr);
                self.outcome_for_shape(&shape, &expr.render())
            }
            // everything else (calls, receives, arithmetic, try, paren):
            // classify the result shape
            _ => {
                let shape = self.shape_of_expr(st, expr);
                self.outcome_for_shape(&shape, &expr.render())
            }
        }
    }

    fn classify_ident(&self, st: &FnState, name: &str) -> Outcome {
        let Some(b) = self.lookup_binding(st, name) else {
            // Unknown name (perimeter/global) — permissive, documented.
            return Outcome::Ok;
        };
        if b.moved.is_some() {
            // already reported at the use site — do not double-report A1c
            return Outcome::Ok;
        }
        if b.managed_taint {
            return Outcome::Managed {
                desc: self.managed_desc(name, b),
                risky: !b.shape.is_sendable_by_type(),
            };
        }
        if b.immutable {
            return Outcome::Ok; // class 4 (let/const/const-decl)
        }
        if b.shape.is_sendable_by_type() {
            return Outcome::Ok; // classes 1/2 (+ permissive Unknown)
        }
        let ty = self.binding_ty_str(b);
        if b.shape.is_handle() {
            return Outcome::Rejected(RejectInfo {
                value: name.to_string(),
                ty,
                notes: vec![format!(
                    "`{}` is a boundary handle — handles cross tasks only via an owned `move` (O11/O13)",
                    name
                )],
            });
        }
        let mut notes = vec![format!(
            "this {} is shared across tasks: mutable shared state must cross a boundary the checker can see (Tier-0 rule O4)",
            b.shape.noun()
        )];
        if matches!(b.shape, TypeShape::Map(_, _)) {
            notes.push(
                "as written this also panics in Go — concurrent map writes are a runtime fatal; the checker tells you earlier".to_string(),
            );
        }
        Outcome::Rejected(RejectInfo {
            value: name.to_string(),
            ty,
            notes,
        })
    }

    fn classify_addr_of(&self, st: &FnState, expr: &Expr, operand: &Expr) -> Outcome {
        let Some(root) = operand.root_ident() else {
            // `&m[k]`-shaped or otherwise unrooted: classify the pointee shape
            let shape = self.shape_of_expr(st, operand);
            return self.outcome_for_shape(&shape, &expr.render());
        };
        let Some(b) = self.lookup_binding(st, root) else {
            return Outcome::Ok;
        };
        if b.moved.is_some() {
            return Outcome::Ok;
        }
        if b.managed_taint {
            return Outcome::Managed {
                desc: self.managed_desc(root, b),
                risky: !b.shape.is_sendable_by_type(),
            };
        }
        if b.immutable {
            return Outcome::Ok;
        }
        match &b.shape {
            // `&hits` where hits is atomic-typed — the mediated route (OQ6)
            TypeShape::Atomic => Outcome::Ok,
            // `&ch` — channels are share-safe by type
            TypeShape::Chan(_) => Outcome::Ok,
            TypeShape::Unknown => Outcome::Ok,
            TypeShape::Struct(Some(sname)) => {
                let contested = self.struct_has_mutex_field(sname);
                let mut notes = vec![format!(
                    "`{}` is a mutable struct shared as a live borrow — mutation through it from several tasks is a data race",
                    root
                )];
                if contested {
                    notes.push(
                        "this struct is guarded by sync.mutex — race-free idiomatic Go, but Tier 0 does not credit locks at spawn sites (QUESTIONS.md Q1, reading (c))".to_string(),
                    );
                    notes.push(
                        "an \"externally synchronized\" amendment (checker-verified exclusive access behind a lock) is under discussion for a later tier — this rejection is expected to be revisited; it is tracked, not silent (OQ2)".to_string(),
                    );
                }
                Outcome::Rejected(RejectInfo {
                    value: expr.render(),
                    ty: format!("*{}", self.binding_ty_str(b)),
                    notes,
                })
            }
            _ => {
                Outcome::Rejected(RejectInfo {
                    value: expr.render(),
                    ty: format!("*{}", self.binding_ty_str(b)),
                    notes: vec![format!(
                        "`&{}` lends the whole mutable {} — a live borrow of mutable shared state, which is not one of the sendable classes (O4)",
                        root,
                        b.shape.noun()
                    )],
                })
            }
        }
    }

    /// O4.6 deep sendability: a fresh literal is sendable iff every
    /// reference-typed part is sendable; the first bad part is named.
    fn classify_composite<'e>(
        &self,
        st: &FnState,
        expr: &Expr,
        parts: impl Iterator<Item = &'e Expr>,
    ) -> Outcome {
        let mut managed: Option<(String, bool)> = None;
        for part in parts {
            match self.classify_value(st, part) {
                Outcome::Ok => {}
                Outcome::Managed { desc, risky } => {
                    // O6.2: taint propagates through composites; a managed
                    // mutable part makes the composite the trust surface.
                    let r = managed.map(|(_, r)| r).unwrap_or(false) || risky;
                    managed = Some((desc, r));
                }
                Outcome::Rejected(mut info) => {
                    info.notes.push(format!(
                        "`{}` is a live mutable value referenced by the fresh literal `{}` — a fresh literal is sendable only when every reference-typed part is (O4.6 deep sendability)",
                        info.value,
                        expr.render()
                    ));
                    return Outcome::Rejected(info);
                }
            }
        }
        match managed {
            // a composite holding a managed part is managed (O6.2 taint
            // propagates through composites)
            Some((desc, risky)) => Outcome::Managed { desc, risky },
            None => Outcome::Ok,
        }
    }

    fn outcome_for_shape(&self, shape: &TypeShape, value: &str) -> Outcome {
        if shape.is_sendable_by_type() {
            return Outcome::Ok;
        }
        let ty = shape.render();
        if shape.is_handle() {
            return Outcome::Rejected(RejectInfo {
                value: value.to_string(),
                ty: ty.clone(),
                notes: vec![format!(
                    "`{}` is a boundary handle — handles cross tasks only via an owned `move` (O11/O13)",
                    value
                )],
            });
        }
        Outcome::Rejected(RejectInfo {
            value: value.to_string(),
            ty: ty.clone(),
            notes: vec![format!(
                "this {} is shared across tasks: mutable shared state must cross a boundary the checker can see (Tier-0 rule O4)",
                shape.noun()
            )],
        })
    }

    fn managed_desc(&self, name: &str, b: &Binding) -> String {
        format!("`{}` ({})", name, self.binding_ty_str(b))
    }

    // -----------------------------------------------------------------------
    // Check-site processing (A1c / O6 suppression + lint)
    // -----------------------------------------------------------------------

    /// Process one spawn/send check-site: classify every value, emit
    /// HYB-E3001/E3002 (A1c) or the HYB-E8001 managed lint, count for
    /// --report-managed, and mark `move` transfers.
    ///
    /// `code` is "HYB-E3001" (spawn) or "HYB-E3002" (send); `site_desc` is a
    /// self-describing phrase used in move notes ("the move-send
    /// `results <- move agg`"). `implicit_move` covers parsers that flag
    /// `Send{is_move: true}` with a plain Ident value.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn check_site(
        &mut self,
        st: &mut FnState,
        span: Span,
        code: &str,
        values: &[&Expr],
        managed: bool,
        site_desc: &str,
        implicit_move: bool,
    ) {
        self.total_sites += 1;

        let mut rejects: Vec<RejectInfo> = Vec::new();
        let mut managed_descs: Vec<String> = Vec::new();
        let mut moves: Vec<(String, Span)> = Vec::new();

        for v in values {
            // class-3 owned moves first: `move x` (Expr form) and the
            // parser's flagged-Ident form (`Send{is_move: true, value: x}`).
            // Either way the value is sendable AND the binding is moved-from
            // at this site — never also classified (that would reject the
            // legal move itself).
            if let Some(name) = move_operand_ident(v) {
                moves.push((name, v.span()));
                continue;
            }
            if implicit_move {
                if let Expr::Ident { name, span: ispan } = v {
                    moves.push((name.clone(), *ispan));
                    continue;
                }
            }
            match self.classify_value(st, v) {
                Outcome::Ok => {}
                Outcome::Managed { desc, risky } => {
                    // only values not safe-by-type are the trust surface
                    if risky {
                        managed_descs.push(desc);
                    }
                }
                Outcome::Rejected(info) => rejects.push(info),
            }
        }

        if !rejects.is_empty() {
            if managed {
                // O6.1: sites inside a managed block are permitted + linted
                self.sites_via_managed += 1;
                let shared: Vec<String> = rejects
                    .iter()
                    .map(|r| format!("`{}` ({})", r.value, r.ty))
                    .collect();
                let msg = format!(
                    "spawn/send site inside a `managed` block: {} shared across tasks (unchecked)",
                    shared.join(", ")
                );
                let notes = vec![
                    "the `managed` hatch suppresses the sendability check (O6.1): the contract is Go's DRF-SC discipline with `-race` as the backstop".to_string(),
                    "managed does not remove Go's own runtime fatals (concurrent map writes are still a runtime fatal) — prefer the channel redesign or add a lock".to_string(),
                    "counted by --report-managed — the hatch is greppable, auditable, countable (O6.3)".to_string(),
                ];
                self.diags.push(
                    Diagnostic::warning("HYB-E8001", self.path.clone(), span, msg)
                        .with_notes(notes),
                );
            } else {
                let site_word = if code == "HYB-E3001" {
                    "across tasks"
                } else {
                    "at this channel send"
                };
                for r in rejects {
                    let msg = format!("`{}` ({}) is not sendable {}", r.value, r.ty, site_word);
                    self.diags.push(
                        Diagnostic::error(code, self.path.clone(), span, msg).with_notes(r.notes),
                    );
                }
            }
        } else if !managed_descs.is_empty() {
            // class 6 resolution (managed-tainted value), even outside a
            // block; only non-safe-by-type values are the trust surface
            self.sites_via_managed += 1;
            let msg = format!(
                "managed value crosses this spawn/send site (class 6 — outside the safety guarantee): {}",
                managed_descs.join(", ")
            );
            let notes = vec![
                "a value born inside a `managed` block is sendable everywhere but stays outside the §9.2 guarantee for its whole lifetime (O6.2)".to_string(),
                "counted by --report-managed (O6.3)".to_string(),
            ];
            self.diags.push(
                Diagnostic::warning("HYB-E8001", self.path.clone(), span, msg).with_notes(notes),
            );
        }

        // O5.1: mark moved bindings (+ freeze live aliases)
        for (name, mspan) in moves {
            self.mark_move(st, &name, mspan, site_desc.to_string(), MoveKind::Moved);
        }
    }
}
