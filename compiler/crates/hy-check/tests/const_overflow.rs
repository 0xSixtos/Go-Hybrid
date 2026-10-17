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

const OVERFLOW_CALL_ARG: &str = "\
fn take_b(b uint8) {
    fmt.println(\"{b}\")
}
fn main() {
    take_b(300)
}
";

const OVERFLOW_CALL_ARG_NEG: &str = "\
fn take_b(b uint8) {
    fmt.println(\"{b}\")
}
fn main() {
    take_b(-1)
}
";

const FITS_CALL_ARG: &str = "\
fn take_b(b uint8) {
    fmt.println(\"{b}\")
}
fn main() {
    take_b(255)
}
";

const OVERFLOW_METHOD_ARG: &str = "\
struct user {
    age int8
}
fn (u *user) set_age(age int8) {
    u.age = age
}
fn main() {
    var u user
    u.set_age(300)
}
";

const OVERFLOW_IIFE_ARG: &str = "\
fn main() {
    fn(b uint8) {
        fmt.println(\"{b}\")
    }(300)
}
";

const OVERFLOW_APPEND_ARG: &str = "\
fn main() {
    var xs []int8
    xs = append(xs, 300)
}
";

const FITS_APPEND_ARG: &str = "\
fn main() {
    var xs []int8
    xs = append(xs, 127)
}
";

const OVERFLOW_TASK_ARG: &str = "\
fn worker(b uint8) {
    fmt.println(\"{b}\")
}
fn main() {
    task worker(300)
}
";

const OVERFLOW_ALIASED_PARAM_ARG: &str = "\
type my_byte = uint8
fn take_b(b my_byte) {
    fmt.println(\"{b}\")
}
fn main() {
    take_b(300)
}
";

#[test]
fn call_argument_overflow_is_rejected() {
    let r = check(OVERFLOW_CALL_ARG);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "take_b(300) must emit HYB-E6002, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn call_argument_negative_overflow_is_rejected() {
    let r = check(OVERFLOW_CALL_ARG_NEG);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "take_b(-1) for uint8 must emit HYB-E6002, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn call_argument_fitting_accepts() {
    let r = check(FITS_CALL_ARG);
    assert!(
        error_codes(&r).is_empty(),
        "take_b(255) must fit uint8, got {:?}",
        error_codes(&r)
    );
    assert_eq!(r.verdict, Verdict::Accept);
}

#[test]
fn method_argument_overflow_is_rejected() {
    let r = check(OVERFLOW_METHOD_ARG);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "u.set_age(300) must emit HYB-E6002, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn iife_argument_overflow_is_rejected() {
    let r = check(OVERFLOW_IIFE_ARG);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "iife must emit HYB-E6002, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn append_argument_overflow_is_rejected() {
    let r = check(OVERFLOW_APPEND_ARG);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "append(xs, 300) for []int8 must emit HYB-E6002, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn append_argument_fitting_accepts() {
    let r = check(FITS_APPEND_ARG);
    assert!(
        error_codes(&r).is_empty(),
        "append(xs, 127) must fit int8, got {:?}",
        error_codes(&r)
    );
    assert_eq!(r.verdict, Verdict::Accept);
}

#[test]
fn task_argument_overflow_is_rejected() {
    let r = check(OVERFLOW_TASK_ARG);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "task worker(300) must emit HYB-E6002, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}

#[test]
fn aliased_parameter_argument_overflow_is_rejected() {
    let r = check(OVERFLOW_ALIASED_PARAM_ARG);
    let cs = error_codes(&r);
    assert!(
        cs.iter().any(|c| c == "HYB-E6002"),
        "take_b(300) for my_byte must emit HYB-E6002, got {:?}",
        cs
    );
    assert_eq!(r.verdict, Verdict::Reject);
}
