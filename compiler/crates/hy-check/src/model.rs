//! model.rs — the Tier-0 checker's data model (SPEC-LANGUAGE.md §9).
//!
//! This is a deliberately LIGHTWEIGHT classification (per the task brief:
//! "a lightweight classification — NOT full type inference"): every binding
//! gets a `TypeShape` good enough to decide the §9 rules. There is no type
//! variable, no unification, no coercion — just enough structure to answer
//! "is this value sendable?" (O4) and "which loan rules apply?" (O7/O8/O12).

use hy_diag::{Diagnostic, Span};
use hy_syntax::{Expr, File, FnDecl, TypeExpr};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Type shapes
// ---------------------------------------------------------------------------

/// A coarse classification of a binding's type. `Unknown` is the permissive
/// default: Tier 0 never rejects on shapes it cannot see (the corpus only
/// exercises the classified forms; unknown shapes are foreign-perimeter or
/// parser-variance territory — see the TECH DEBT note in foreign.rs).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TypeShape {
    /// `int`, `bool`, `float64`, ... — value copies (class 1).
    Primitive,
    /// `string` — immutable (class 4).
    Str,
    /// `atomic.*` — internally synchronized (class 2).
    Atomic,
    /// any `chan` direction (class 2); carries the element shape for
    /// loop-variable and receive inference.
    Chan(Box<TypeShape>),
    /// `map[K]V` — mutable GC composite (NOT sendable).
    Map(Box<TypeShape>, Box<TypeShape>),
    /// `[]T` — mutable GC composite (NOT sendable).
    Slice(Box<TypeShape>),
    /// `[N]T` — mutable GC composite (NOT sendable as a binding).
    Array(Box<TypeShape>),
    /// a struct (local decl name if known) — mutable GC composite.
    Struct(Option<String>),
    /// `*T` — pointer; sendable only when the pointee is atomic (OQ6).
    Pointer(Box<TypeShape>),
    /// an L3 own handle, e.g. `serde_json.value`, `flate2.writer` (O13).
    OwnHandle(String),
    /// an L3 borrow handle / element view, e.g. `serde_json.view` (O7).
    BorrowHandle(String),
    /// a Go-side opaque foreign type (`sync.mutex`, `http.*`, ...) — the A5
    /// perimeter; no loan discipline, not sendable at spawn/send.
    ForeignOpaque(String),
    /// unresolved / not understood — permissive.
    Unknown,
}

impl TypeShape {
    /// Class 1/2/4-by-type: primitives, strings, atomics, channels (a channel
    /// is safe to share BY TYPE regardless of its element type — the value
    /// crossing the site is the channel itself), borrows of atomics, and the
    /// permissive `Unknown`.
    pub(crate) fn is_sendable_by_type(&self) -> bool {
        match self {
            TypeShape::Primitive
            | TypeShape::Str
            | TypeShape::Atomic
            | TypeShape::Chan(_)
            | TypeShape::Unknown => true,
            // `&atomic_binding` is the mediated share route (OQ6); a pointer
            // to anything else is a mutable pointee — strict reject.
            TypeShape::Pointer(inner) => matches!(**inner, TypeShape::Atomic),
            _ => false,
        }
    }

    /// An L3 boundary handle (loan rules O7/O11 apply).
    pub(crate) fn is_handle(&self) -> bool {
        matches!(self, TypeShape::OwnHandle(_) | TypeShape::BorrowHandle(_))
    }

    /// Go reference semantics: assigning `b := a` makes `b` an ALIAS of `a`
    /// (maps and slices are reference types; pointers alias the pointee).
    /// Structs and arrays are copied by value in Go, so they are NOT
    /// binding-level aliases. Used by the freeze rule (O5.2).
    pub(crate) fn is_reference_alias(&self) -> bool {
        matches!(
            self,
            TypeShape::Map(_, _) | TypeShape::Slice(_) | TypeShape::Pointer(_)
        )
    }

    /// Short human name for diagnostics.
    pub(crate) fn noun(&self) -> &'static str {
        match self {
            TypeShape::Map(_, _) => "map",
            TypeShape::Slice(_) => "slice",
            TypeShape::Array(_) => "array",
            TypeShape::Struct(_) => "struct",
            TypeShape::Pointer(_) => "pointer",
            TypeShape::Chan(_) => "channel",
            TypeShape::Atomic => "atomic value",
            TypeShape::Str => "string",
            TypeShape::Primitive => "primitive value",
            TypeShape::OwnHandle(_) | TypeShape::BorrowHandle(_) => "boundary handle",
            TypeShape::ForeignOpaque(_) => "foreign value",
            TypeShape::Unknown => "value",
        }
    }

    /// Fallback rendering for diagnostics when no source `TypeExpr` is
    /// available (never panics).
    pub(crate) fn render(&self) -> String {
        match self {
            TypeShape::Primitive => "primitive".to_string(),
            TypeShape::Str => "string".to_string(),
            TypeShape::Atomic => "atomic".to_string(),
            TypeShape::Chan(e) => format!("chan {}", e.render()),
            TypeShape::Map(k, v) => format!("map[{}]{}", k.render(), v.render()),
            TypeShape::Slice(e) => format!("[]{}", e.render()),
            TypeShape::Array(e) => format!("[...]{}", e.render()),
            TypeShape::Struct(Some(n)) => n.clone(),
            TypeShape::Struct(None) => "struct".to_string(),
            TypeShape::Pointer(t) => format!("*{}", t.render()),
            TypeShape::OwnHandle(t) | TypeShape::BorrowHandle(t) | TypeShape::ForeignOpaque(t) => {
                t.clone()
            }
            TypeShape::Unknown => "unknown value".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Bindings, moves, loans
// ---------------------------------------------------------------------------

/// Everything the checker knows about one local binding.
#[derive(Debug, Clone)]
pub(crate) struct Binding {
    pub shape: TypeShape,
    /// The source type as written (for diagnostics: "map[string]int").
    pub ty: Option<TypeExpr>,
    /// `let`/`const` binding or a file-level `const` (class 4).
    pub immutable: bool,
    /// Born inside a `managed` block (O6.2 taint): sendable everywhere,
    /// outside the §9.2 guarantee for its whole lifetime.
    pub managed_taint: bool,
    /// Moved-from / consumed-from state (O5.1 / O13).
    pub moved: Option<MoveInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveKind {
    /// `move x` at a spawn/send/assignment/return (O5.1) — A1b.
    Moved,
    /// A consuming API call took ownership (O13) — A7.
    Consumed,
}

#[derive(Debug, Clone)]
pub(crate) struct MoveInfo {
    /// The transfer site (where ownership left).
    pub span: Span,
    pub kind: MoveKind,
    /// Self-describing site description, e.g.
    /// "the move-send `results <- move agg`" or "the consuming call `enc.finish()`".
    pub desc: String,
}

/// A live loan. `last` is the inclusive end of the non-lexical window, in
/// statement indices (see walk.rs for the indexing scheme).
#[derive(Debug, Clone)]
pub(crate) enum LoanTarget {
    /// O7/A2: borrow-mode loan on a boundary handle binding.
    Handle { handle: String },
    /// O8/A3: unique loan on one map slot; `binding` is the defined name
    /// (for last-use computation), `path` is the base map path ("e.services").
    MapSlot { binding: String, path: String },
}

#[derive(Debug, Clone)]
pub(crate) struct Loan {
    pub target: LoanTarget,
    pub created: usize,
    pub last: usize,
    pub span: Span,
    pub desc: String,
}

/// Per-function mutable state (environment, aliases, live loans).
#[derive(Debug, Default)]
pub(crate) struct FnState {
    pub env: HashMap<String, Binding>,
    /// alias name -> target binding name (binding-level only — enough for the
    /// 20-program corpus; documented limitation).
    pub aliases: HashMap<String, String>,
    pub loans: Vec<Loan>,
    /// name -> last statement index that references it (Pass A result).
    pub last_use: HashMap<String, usize>,
}

// ---------------------------------------------------------------------------
// The checker (file-level tables + accumulators)
// ---------------------------------------------------------------------------

pub(crate) struct Checker<'a> {
    pub file: &'a File,
    /// Source path, cloned onto every diagnostic.
    pub path: String,
    /// struct name -> (field name, field type) — file-level struct decls.
    pub structs: HashMap<String, Vec<(String, TypeExpr)>>,
    /// fn name -> decl (return-type inference for single-value defines).
    pub fns: HashMap<String, &'a FnDecl>,
    /// file-level consts (immutable, visible in every fn).
    pub consts: HashMap<String, Binding>,
    /// Known foreign package prefixes (imports ∪ the builtin table).
    pub packages: HashSet<String>,
    /// Emitted diagnostics (unsorted; check_file sorts by span).
    pub diags: Vec<Diagnostic>,
    // --report-managed counters (O6.3)
    pub managed_blocks: usize,
    pub sites_via_managed: usize,
    pub total_sites: usize,
}

impl<'a> Checker<'a> {
    pub(crate) fn new(file: &'a File) -> Self {
        Checker {
            file,
            path: file.path.clone(),
            structs: HashMap::new(),
            fns: HashMap::new(),
            consts: HashMap::new(),
            packages: HashSet::new(),
            diags: Vec::new(),
            managed_blocks: 0,
            sites_via_managed: 0,
            total_sites: 0,
        }
    }

    /// Binding lookup: locals first, then file-level consts.
    pub(crate) fn lookup_binding<'b>(&'b self, st: &'b FnState, name: &str) -> Option<&'b Binding> {
        st.env.get(name).or_else(|| self.consts.get(name))
    }

    /// The type string for diagnostics: the source type when we have it,
    /// else the shape fallback.
    pub(crate) fn binding_ty_str(&self, b: &Binding) -> String {
        b.ty.as_ref()
            .map(|t| t.render())
            .unwrap_or_else(|| b.shape.render())
    }
}

// ---------------------------------------------------------------------------
// Small shared expression helpers
// ---------------------------------------------------------------------------

/// `&x` — both legal AST spellings (`Expr::AddrOf` and `Expr::Unary{Addr}`).
pub(crate) fn as_addr_of(e: &Expr) -> Option<&Expr> {
    match e {
        Expr::AddrOf { operand, .. } => Some(operand),
        Expr::Unary {
            op: hy_syntax::UnOp::Addr,
            operand,
            ..
        } => Some(operand),
        _ => None,
    }
}

/// The direct operand of a `move x` when it is a plain binding.
pub(crate) fn move_operand_ident(e: &Expr) -> Option<String> {
    match e {
        Expr::Move { operand, .. } => match &**operand {
            Expr::Ident { name, .. } => Some(name.clone()),
            // `move s.conn` (own-handle field moves, O5.4) are not tracked in
            // Tier 0's binding-level analysis — the corpus never needs them.
            _ => None,
        },
        _ => None,
    }
}
