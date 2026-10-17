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

#[test]
fn chan_send_int8_overflow_is_rejected() {
    let src = "
fn worker(ch chan int8) {
    ch <- 300
}
";
    let r = check(src);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for ch <- 300 under chan int8, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn chan_send_uint8_negative_is_rejected() {
    let src = "
fn worker(ch chan uint8) {
    ch <- -1
}
";
    let r = check(src);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for ch <- -1 under chan uint8, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn chan_send_float_truncation_is_rejected() {
    let src = "
fn worker(ch chan int) {
    ch <- 2.5
}
";
    let r = check(src);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for ch <- 2.5 under chan int, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn chan_send_select_overflow_is_rejected() {
    let src = "
fn worker(ch chan int8) {
    select {
    ch <- 300:
        _ = 1
    default:
    }
}
";
    let r = check(src);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for select arm ch <- 300, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn chan_send_closure_overflow_is_rejected() {
    let src = "
fn main() {
    ch := make(chan int8, 1)
    f := fn() {
        ch <- 300
    }
    f()
}
";
    let r = check(src);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 in closure body, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn chan_send_aliased_type_overflow_is_rejected() {
    let src = "
type my_byte = uint8

fn worker(ch chan my_byte) {
    ch <- 300
}
";
    let r = check(src);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "expected HYB-E6002 for aliased type send, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn chan_send_fitting_value_accepts() {
    let src = "
fn worker(ch chan int8) {
    ch <- 100
}
";
    let r = check(src);
    assert!(
        error_codes(&r).is_empty(),
        "expected 100 to fit chan int8 cleanly, got {:?}",
        error_codes(&r)
    );
    assert_eq!(r.verdict, Verdict::Accept);
}
