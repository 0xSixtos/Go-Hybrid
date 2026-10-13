//! hy-emit/src/shapes.rs — the foreign shape table (Batch E: example 06).
//!
//! The MVP-0 binding hints (BindingKind) answer "is this binding foreign?"
//! for ONE member level. Example 06 needs three: `r.url.query.get("a")` is a
//! FIELD (`r.URL`), a METHOD GO SPELLS WITHOUT ARGS IN HY (`r.URL.Query()`),
//! and a METHOD (`Get`) — and the closure converting to `http.handler` needs
//! its untyped params typed by the TARGET (`http.HandlerFunc`'s signature).
//!
//! This table is the curated-surface answer (SPEC M5: bindings are
//! per-package work; promising "any Go package" would be the dishonest
//! version). It is deliberately small: exactly what the ten pinned programs
//! need, grown by Decision Log entries. A real type checker supersedes it
//! post-MVP (the table's keys ARE the descriptor types that checker will
//! resolve), so nothing here is throwaway spelling.
//!
//! Discipline: every entry names the example that pins it. Members nobody
//! uses are not here — "unsupported" is the clean error, never a guess.

/// How a member is spelled in Go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    /// A Go field: `r.URL` — no parens, ever.
    Field,
    /// A Go method called with no arguments in this surface:
    /// hy spells `r.url.query`, Go requires `r.URL.Query()`.
    MethodZero,
    /// A Go method: args follow the call.
    Method,
}

/// One member of a foreign type.
#[derive(Debug, Clone, Copy)]
pub struct Member {
    /// Go spelling — EXACT (`"URL"`, not `to_go_name("url")` = `"Url"`).
    pub go: &'static str,
    pub kind: MemberKind,
    /// The descriptor type key of the member's result, for chain
    /// continuation (`url` on http.request → Some("url.url")).
    pub result: Option<&'static str>,
}

const fn field(go: &'static str, result: Option<&'static str>) -> Member {
    Member { go, kind: MemberKind::Field, result }
}

const fn method0(go: &'static str, result: Option<&'static str>) -> Member {
    Member { go, kind: MemberKind::MethodZero, result }
}

const fn method(go: &'static str, result: Option<&'static str>) -> Member {
    Member { go, kind: MemberKind::Method, result }
}

/// A foreign type's member table, keyed by the hy type name
/// (`http.request` — dotted, snake_case, exactly as the surface spells it).
pub struct TypeShape {
    pub members: &'static [(&'static str, Member)],
}

/// The curated table. Keys are hy type names; every entry names its pinning
/// example. `http.header` rides along (response_writer.header's result) so
/// the chain continues if an example needs it.
pub static SHAPES: &[(&str, TypeShape)] = &[
    (
        // 06: r.method, r.url (fields on *http.Request)
        "http.request",
        TypeShape {
            members: &[
                ("method", field("Method", None)),
                ("url", field("URL", Some("url.url"))),
                ("header", field("Header", Some("http.header"))),
                ("host", field("Host", None)),
                ("remote_addr", field("RemoteAddr", None)),
                ("body", field("Body", None)),
            ],
        },
    ),
    (
        // 06: r.url.path (field), r.url.query (zero-arg METHOD — Go parses
        // RawQuery on every Query() call; the hy surface hides that).
        "url.url",
        TypeShape {
            members: &[
                ("path", field("Path", None)),
                ("raw_path", field("RawPath", None)),
                ("raw_query", field("RawQuery", None)),
                ("host", field("Host", None)),
                ("query", method0("Query", Some("url.values"))),
            ],
        },
    ),
    (
        // 06: r.url.query.get("a") — url.Values.Get (first value for a key).
        "url.values",
        TypeShape {
            members: &[
                ("get", method("Get", None)),
                ("set", method("Set", None)),
                ("has", method("Has", None)),
                ("encode", method("Encode", None)),
            ],
        },
    ),
    (
        // 06: w as http.ResponseWriter — header()/write/writeHeader.
        "http.response_writer",
        TypeShape {
            members: &[
                ("header", method0("Header", Some("http.header"))),
                ("write", method("Write", None)),
                ("write_header", method("WriteHeader", None)),
            ],
        },
    ),
    (
        // http.Header — the result of w.header(); get/set/add.
        "http.header",
        TypeShape {
            members: &[
                ("get", method("Get", None)),
                ("set", method("Set", None)),
                ("add", method("Add", None)),
            ],
        },
    ),
    (
        // 06: next.serve_http(w, r) — the http.Handler interface's method.
        "http.handler",
        TypeShape {
            members: &[("serve_http", method("ServeHTTP", None))],
        },
    ),
    (
        // 06: mux.handle(path, handler) / mux.handle_func.
        "http.serve_mux",
        TypeShape {
            members: &[
                ("handle", method("Handle", None)),
                ("handle_func", method("HandleFunc", None)),
                ("serve_http", method("ServeHTTP", None)),
            ],
        },
    ),
    (
        // 10: dt.milliseconds — hy spells Duration's methods property-style;
        // Go requires the call parens (MethodZero).
        "time.duration",
        TypeShape {
            members: &[
                ("milliseconds", method0("Milliseconds", None)),
                ("microseconds", method0("Microseconds", None)),
                ("nanoseconds", method0("Nanoseconds", None)),
                ("seconds", method0("Seconds", None)),
                ("minutes", method0("Minutes", None)),
                ("hours", method0("Hours", None)),
            ],
        },
    ),
    (
        // 10: t0 from time.now() — typed so since(t0) chains cleanly; no
        // members the pinned surface uses yet.
        "time.time",
        TypeShape { members: &[] },
    ),
];

/// Member lookup: (descriptor type, hy member name) → Member.
pub fn lookup_member(ty: &str, name: &str) -> Option<&'static Member> {
    let (_, shape) = SHAPES.iter().find(|(k, _)| *k == ty)?;
    let (_, m) = shape.members.iter().find(|(k, _)| *k == name)?;
    Some(m)
}

// ---------------------------------------------------------------------------
// Closure-conversion targets (E11's carrier): a hy type whose VALUE is
// really a Go func needing a named conversion at the conversion site.
// ---------------------------------------------------------------------------

/// `http.handler(fn(w, r) { … })` → `http.HandlerFunc(func(w http.
/// ResponseWriter, r *http.Request) { … })` — an untyped fn literal takes
/// its param types from the TARGET, and `?` inside it propagates through
/// http.Error (the E11 handler contract), because the target signature has
/// no error result for E3's return to land in.
pub struct Conversion {
    /// Go type text of the CONVERSION constructor (`http.HandlerFunc`).
    pub go: &'static str,
    /// The underlying func's params. `ty_key` is the shape-table descriptor
    /// (chains resolve through it); `go` is the EXACT Go type text; `pointer`
    /// distinguishes `w ResponseWriter` (interface, by value) from
    /// `r *Request` (pointer) — Go's HandlerFunc signature mixes both.
    pub params: &'static [ParamDesc],
    /// The E11 propagation contract, if this target has one:
    /// the hy member name of the "writer" first param used by the
    /// propagation statement (`http.Error(w, err, 400)`).
    pub propagate: Option<&'static str>,
}

pub struct ParamDesc {
    #[allow(dead_code)] // documents the surface; the emitter keys on position
    pub name: &'static str,
    pub ty_key: &'static str,
    pub go: &'static str,
    pub pointer: bool,
}

pub static CONVERSIONS: &[(&str, Conversion)] = &[(
    // Pinned by example 06. http.Handler is an INTERFACE — a bare func
    // literal does not implement it; Go's conversion type is HandlerFunc.
    "http.handler",
    Conversion {
        go: "http.HandlerFunc",
        params: &[
            ParamDesc { name: "w", ty_key: "http.response_writer", go: "http.ResponseWriter", pointer: false },
            ParamDesc { name: "r", ty_key: "http.request", go: "http.Request", pointer: true },
        ],
        propagate: Some("w"),
    },
)];

pub fn lookup_conversion(ty: &str) -> Option<&'static Conversion> {
    CONVERSIONS.iter().find(|(k, _)| *k == ty).map(|(_, c)| c)
}

// ---------------------------------------------------------------------------
// Rust crate bindings (M6): `import "rust/X"` → a generated-shape Go binding
// package, one descriptor per pinned crate. The emitter lowers
// `X.f(args…)` to `bindingPkg.GoName(args…)` with named args placed by the
// descriptor's param order (Go has no named args — example 10's `seed=7`).
// ---------------------------------------------------------------------------

pub struct RustFnDesc {
    /// hy surface name (`sha3.digest256`'s `digest256`).
    pub hy: &'static str,
    /// Go name in the binding package.
    pub go: &'static str,
    /// Param names in order — named args land here; positional args fill
    /// free slots left-to-right.
    pub params: &'static [&'static str],
    /// Coerce string args to []byte at the call (`[]byte(arg)` is legal Go
    /// even when the arg is already a byte slice — a copy, which the
    /// copy-small/pin-large protocol allows).
    pub bytes_coerce: bool,
}

pub struct RustCrateDesc {
    /// hy package root (`sha3` in `import "rust/sha3"`).
    pub hy: &'static str,
    /// The binding Go package's ROOT (import path via import_path_of).
    pub binding_root: &'static str,
    pub fns: &'static [RustFnDesc],
}

pub static RUST_CRATES: &[RustCrateDesc] = &[
    // Pinned by example 10; the binding lives in runtime/go/sha3binding.
    RustCrateDesc {
        hy: "sha3",
        binding_root: "sha3binding",
        fns: &[
            RustFnDesc {
                hy: "digest256",
                go: "Digest256",
                params: &["data"],
                bytes_coerce: true,
            },
            RustFnDesc {
                hy: "stream_checksum",
                go: "StreamChecksum",
                params: &["iters", "seed"],
                bytes_coerce: false,
            },
        ],
    },
];

pub fn lookup_rust_crate(root: &str) -> Option<&'static RustCrateDesc> {
    RUST_CRATES.iter().find(|c| c.hy == root)
}

// ---------------------------------------------------------------------------
// Import-path mapping (SPEC M5's curated allowlist, mechanical part).
// ---------------------------------------------------------------------------

/// hy package root → Go import path. Most roots are 1:1; net/* differ.
/// The emitted CODE always refers to the root spelling (`http.HandlerFunc`)
/// — Go's package name for `net/http` IS `http`, so the code is unchanged;
/// only the import line's path differs.
///
/// `hyb build` assembles every program as module `hybrid.program` with the
/// runtime packages copied inside — the binding imports are
/// `hybrid.program/<binding_root>`.
pub fn import_path_of(root: &str) -> String {
    match root {
        "http" => "net/http".to_string(),
        "httptest" => "net/http/httptest".to_string(),
        "hex" => "encoding/hex".to_string(),
        "sha3binding" => "hybrid.program/sha3binding".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_chain_resolves_three_levels() {
        // r.url.query.get — the exact example-06 chain.
        let url_m = lookup_member("http.request", "url").unwrap();
        assert_eq!(url_m.go, "URL");
        assert_eq!(url_m.kind, MemberKind::Field);
        assert_eq!(url_m.result, Some("url.url"));

        let q_m = lookup_member("url.url", "query").unwrap();
        assert_eq!(q_m.go, "Query");
        assert_eq!(q_m.kind, MemberKind::MethodZero);
        assert_eq!(q_m.result, Some("url.values"));

        let g_m = lookup_member("url.values", "get").unwrap();
        assert_eq!(g_m.go, "Get");
        assert_eq!(g_m.kind, MemberKind::Method);
    }

    #[test]
    fn unknown_members_are_none() {
        assert!(lookup_member("http.request", "nope").is_none());
        assert!(lookup_member("made.up.type", "url").is_none());
    }

    #[test]
    fn handler_conversion_shape() {
        let c = lookup_conversion("http.handler").unwrap();
        assert_eq!(c.go, "http.HandlerFunc");
        assert_eq!(c.params.len(), 2);
        assert_eq!(c.params[0].name, "w");
        assert_eq!(c.params[0].ty_key, "http.response_writer");
        assert!(!c.params[0].pointer, "the writer is an interface value");
        assert!(c.params[1].pointer, "the request is a pointer");
        assert_eq!(c.propagate, Some("w"));
    }

    #[test]
    fn import_paths_map_net_packages() {
        assert_eq!(import_path_of("http"), "net/http");
        assert_eq!(import_path_of("httptest"), "net/http/httptest");
        assert_eq!(import_path_of("fmt"), "fmt");
        assert_eq!(import_path_of("strconv"), "strconv");
    }
}
