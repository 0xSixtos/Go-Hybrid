//! const_write.rs — regression tests for HYB-E3006 (T22 deep immutability
//! on `const`), from soundness issue #37: a file-level `const` slice took
//! element writes and whole-binding rebinds with 0 diagnostics, the emitter
//! lowered it to a package-level `var`, and two spawned tasks raced
//! (`hyb check` OK / `go run -race` DATA RACE). Verdict: `const` is frozen —
//! binding, elements, and fields; reads stay legal; shadowing copies stay
//! writable.

use hy_check::{check_file, Verdict};
use hy_diag::Severity;
use hy_syntax::parse_file;

fn check(src: &str) -> hy_check::CheckResult {
    let path = "inline.hy";
    let ast = parse_file(path, src)
        .unwrap_or_else(|diags| panic!("must parse (parser contract): {:?}", diags));
    check_file(&ast)
}

fn codes(r: &hy_check::CheckResult) -> Vec<String> {
    r.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.code.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Rejects — every write form through a `const` root
// ---------------------------------------------------------------------------

#[test]
fn file_const_element_write_is_e3006() {
    // Issue #37's exact shape: the index write inside the task body.
    let r = check("const cells = [0]\n\nfn main() {\n    cells[0] = cells[0] + 1\n}\n");
    let cs = codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E3006"),
        "expected HYB-E3006 for the element write through a file-level const, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn file_const_rebind_is_e3006() {
    // The whole-binding form from the issue's acceptance text (`cells = [5]`).
    let r = check("const cells = [0]\n\nfn main() {\n    cells = [5]\n}\n");
    let cs = codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E3006"),
        "expected HYB-E3006 for the whole-binding rebind of a const, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn file_const_field_write_is_e3006() {
    // Deep, not shallow: a field store under a const root is frozen too.
    let r = check(
        "struct point {\n    x int\n    y int\n}\n\nconst origin = point{x: 0, y: 0}\n\nfn main() {\n    origin.x = 1\n}\n",
    );
    let cs = codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E3006"),
        "expected HYB-E3006 for the field write through a const root, got {:?}",
        cs
    );
}

#[test]
fn fn_local_const_rebind_is_e3006() {
    // Local const: `Binding::immutable` is set iff LocalKind::Const.
    let r = check("fn main() {\n    const n = 5\n    n = 6\n}\n");
    let cs = codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E3006"),
        "expected HYB-E3006 for rebinding a fn-local const, got {:?}",
        cs
    );
}

#[test]
fn fn_local_const_element_write_is_e3006() {
    let r = check("fn main() {\n    const acc = [0]\n    acc[0] = 1\n}\n");
    let cs = codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E3006"),
        "expected HYB-E3006 for an element write through a fn-local const, got {:?}",
        cs
    );
}

#[test]
fn compound_assign_through_const_is_e3006() {
    // `+=` is a write: the compound op must not slip past the root check.
    let r = check("const total = 0\n\nfn main() {\n    total += 1\n}\n");
    let cs = codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E3006"),
        "expected HYB-E3006 for `+=` through a const, got {:?}",
        cs
    );
}

// ---------------------------------------------------------------------------
// Accepts — reads, shadowing copies, and plain mutable bindings
// ---------------------------------------------------------------------------

#[test]
fn reads_of_const_stay_legal() {
    // Deep immutability bans the write, not the value (E3006 must not fire).
    let r = check(
        "const budget = 3\n\nfn work(x int) int {\n    return x + budget\n}\n\nfn main() {\n    y := work(1)\n    _ = y\n}\n",
    );
    assert!(
        codes(&r).is_empty(),
        "reading a const must stay legal, got {:?}",
        codes(&r)
    );
    assert_eq!(r.verdict, Verdict::Accept);
}

#[test]
fn shadowing_local_copy_of_const_name_stays_writable() {
    // lookup_binding is locals-first: a `let`/`var` copy that shadows the
    // const name is a fresh mutable binding — the write goes to the copy.
    let r = check("const cells = [0]\n\nfn main() {\n    cells := [1]\n    cells[0] = 2\n}\n");
    assert!(
        codes(&r).is_empty(),
        "a shadowing local copy must stay writable, got {:?}",
        codes(&r)
    );
    assert_eq!(r.verdict, Verdict::Accept);
}

#[test]
fn mutable_bindings_stay_writable() {
    // The control: `:=` locals and `var` bindings are the legal home for
    // updating state — E3006 fires only on `const` roots.
    let r = check(
        "fn main() {\n    cells := [0]\n    cells[0] = cells[0] + 1\n    var n int\n    n = n + 1\n    _ = cells\n    _ = n\n}\n",
    );
    assert!(
        codes(&r).is_empty(),
        "writes through short-declared/var bindings must stay legal, got {:?}",
        codes(&r)
    );
    assert_eq!(r.verdict, Verdict::Accept);
}

#[test]
fn let_interior_mutation_stays_legal() {
    // T22: `let` is SHALLOW immutability — the binding cannot be rebound,
    // but contents stay mutable, so an element write is not E3006's business.
    let r = check("fn main() {\n    let m = map[string]int{}\n    m[\"k\"] = 1\n    _ = m\n}\n");
    assert!(
        codes(&r).is_empty(),
        "interior mutation through a `let` is shallow-legal, got {:?}",
        codes(&r)
    );
}
