//! const_overflow.rs — regression for issue #148: a constant initializer
//! outside a declared fixed-width integer type used to pass `hyb check`
//! (0 diagnostics) and then fail `go vet` / `go build` with
//! `cannot use N as T value (overflows)`.

use hy_check::{check_file, Verdict};
use hy_diag::Severity;
use hy_syntax::parse_file;

fn check(src: &str) -> hy_check::CheckResult {
    let path = "inline.hy";
    let ast = parse_file(path, src)
        .unwrap_or_else(|diags| panic!("must parse (parser contract): {:?}", diags));
    check_file(&ast)
}

fn error_codes(r: &hy_check::CheckResult) -> Vec<String> {
    r.diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.code.clone())
        .collect()
}

const OVERFLOW_INT8: &str = "\
fn main() {
    var n int8 = 300
    fmt.println(\"{n}\")
}
";

const OVERFLOW_UINT8: &str = "\
fn main() {
    var n uint8 = 256
    fmt.println(\"{n}\")
}
";

const OVERFLOW_BYTE_HEX: &str = "\
fn main() {
    var n byte = 0x100
    fmt.println(\"{n}\")
}
";

const OVERFLOW_NEG_INT8: &str = "\
fn main() {
    var n int8 = -129
    fmt.println(\"{n}\")
}
";

const OVERFLOW_FILE_CONST: &str = "\
const n int8 = 300

fn main() {
    fmt.println(\"{n}\")
}
";

const FITS_INT8: &str = "\
fn main() {
    var n int8 = 127
    var m int8 = -128
    fmt.println(\"{n} {m}\")
}
";

const FITS_BYTE: &str = "\
fn main() {
    var n byte = 255
    fmt.println(\"{n}\")
}
";

const VARIABLE_OPERAND_OK: &str = "\
fn main() {
    x := 300
    y := byte(x)
    fmt.println(\"{y}\")
}
";

const PAREN_OVERFLOW: &str = "\
fn main() {
    var n int8 = (300)
    fmt.println(\"{n}\")
}
";

#[test]
fn int8_constant_300_is_rejected() {
    let r = check(OVERFLOW_INT8);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for var n int8 = 300, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn uint8_constant_256_is_rejected() {
    let r = check(OVERFLOW_UINT8);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn byte_hex_0x100_is_rejected() {
    let r = check(OVERFLOW_BYTE_HEX);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for byte = 0x100, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn int8_neg_129_is_rejected() {
    let r = check(OVERFLOW_NEG_INT8);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for -129, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn file_level_const_overflow_is_rejected() {
    let r = check(OVERFLOW_FILE_CONST);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for file-level const, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn paren_wrapped_overflow_is_rejected() {
    let r = check(PAREN_OVERFLOW);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for (300), got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn int8_boundary_values_accept() {
    let r = check(FITS_INT8);
    assert!(
        error_codes(&r).is_empty(),
        "127 / -128 must fit int8, got {:?}",
        error_codes(&r)
    );
    assert_eq!(r.verdict, Verdict::Accept);
}

#[test]
fn byte_255_accepts() {
    let r = check(FITS_BYTE);
    assert!(
        error_codes(&r).is_empty(),
        "255 must fit byte, got {:?}",
        error_codes(&r)
    );
    assert_eq!(r.verdict, Verdict::Accept);
}

#[test]
fn variable_conversion_truncation_still_accepts() {
    // Issue #148 control: conversion from a *variable* is the runtime
    // truncation path (SPEC X4) and must stay accepted.
    let r = check(VARIABLE_OPERAND_OK);
    assert!(
        error_codes(&r).is_empty(),
        "variable conversion must stay clean, got {:?}",
        error_codes(&r)
    );
    assert_eq!(r.verdict, Verdict::Accept);
}

const OVERFLOW_STRUCT_LITERAL: &str = "\
struct user {
    age int8
}

fn main() {
    u := user{age: 300}
    fmt.println(\"{u.age}\")
}
";

const OVERFLOW_SLICE_LITERAL: &str = "\
fn main() {
    xs := []int8{300}
    fmt.println(\"{xs[0]}\")
}
";

const OVERFLOW_MAP_VALUE: &str = "\
fn main() {
    m := map[string]int8{\"a\": 300}
    v := m[\"a\"]
    fmt.println(\"{v}\")
}
";

const OVERFLOW_MAP_KEY: &str = "\
fn main() {
    m := map[int8]string{300: \"a\"}
    v := m[300]
    fmt.println(\"{v}\")
}
";

const OVERFLOW_TYPE_ALIAS_STRUCT_LITERAL: &str = "\
struct config {
    limit uint8
}

type cfg = config

fn main() {
    c := cfg{limit: 300}
    fmt.println(\"{c.limit}\")
}
";

const OVERFLOW_NESTED_COMPOSITE: &str = "\
struct item {
    code int8
}

fn main() {
    items := []item{
        item{code: 300},
    }
    fmt.println(\"{items[0].code}\")
}
";

const FITS_COMPOSITE_LITERALS: &str = "\
struct user {
    age int8
}

fn main() {
    u := user{age: 127}
    xs := []int8{127, -128}
    m := map[string]int8{\"a\": 127}
    v := m[\"a\"]
    fmt.println(\"{u.age} {xs[0]} {v}\")
}
";

#[test]
fn struct_literal_field_overflow_is_rejected() {
    let r = check(OVERFLOW_STRUCT_LITERAL);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for user{{age: 300}}, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn slice_literal_element_overflow_is_rejected() {
    let r = check(OVERFLOW_SLICE_LITERAL);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for []int8{{300}}, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn map_literal_value_overflow_is_rejected() {
    let r = check(OVERFLOW_MAP_VALUE);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for map[string]int8{{\"a\": 300}}, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn map_literal_key_overflow_is_rejected() {
    let r = check(OVERFLOW_MAP_KEY);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for map[int8]string{{300: \"a\"}}, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn type_alias_struct_literal_overflow_is_rejected() {
    let r = check(OVERFLOW_TYPE_ALIAS_STRUCT_LITERAL);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for aliased struct literal limit: 300, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn nested_composite_literal_overflow_is_rejected() {
    let r = check(OVERFLOW_NESTED_COMPOSITE);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for nested composite literal code: 300, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn composite_literals_valid_range_is_accepted() {
    let r = check(FITS_COMPOSITE_LITERALS);
    assert!(
        error_codes(&r).is_empty(),
        "valid composite literals must accept, got {:?}",
        error_codes(&r)
    );
    assert_eq!(r.verdict, Verdict::Accept);
}
