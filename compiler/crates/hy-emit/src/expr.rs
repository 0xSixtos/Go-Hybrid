//! Expression emission — `Emitter` impl part 3.
//!
//! Precedence-aware minimal parenthesization (users read the emitted Go),
//! the special-member table (`os.args()` → `os.Args`, `fmt.eprintln` →
//! `fmt.Fprintln(os.Stderr, …)`, duration constructors), and the bug-#2
//! fix: `make`'s first argument renders as a BARE TYPE — the parser encodes
//! type-as-value as an empty `StructLit{ty: Some(t)}` / `SliceLit` /
//! `MapLit`, and rendering those as value literals inside `make(...)` is
//! invalid Go.

use hy_syntax::*;

use super::{go_quote, is_package_root, special_member, to_go_name, BindingKind, Emitter, EmitError, MemberSpecial};

/// Operator precedence (Go tiers; higher binds tighter).
fn bin_prec(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 3,
        BinOp::Add | BinOp::Sub => 4,
        BinOp::Mul | BinOp::Div | BinOp::Mod => 5,
    }
}

const ATOM: u8 = 100;

impl Emitter {
    /// The full expression emitter. `min_prec` = the precedence the CALLER
    /// requires; lower-precedence children parenthesize themselves.
    pub fn emit_expr(&mut self, e: &Expr, min_prec: u8) -> Result<String, EmitError> {
        let text = self.emit_expr_inner(e)?;
        let my = self.top_prec(e);
        if my < min_prec {
            Ok(format!("({})", text))
        } else {
            Ok(text)
        }
    }

    fn top_prec(&self, e: &Expr) -> u8 {
        match e {
            Expr::Binary { op, .. } => bin_prec(*op),
            _ => ATOM,
        }
    }

    fn emit_expr_inner(&mut self, e: &Expr) -> Result<String, EmitError> {
        match e {
            Expr::Ident { name, .. } => {
                if name == "none" {
                    return Err(EmitError::unsupported(
                        "`none` outside a return (MVP-0 lowers `return none` against the declared option result)",
                    ));
                }
                Ok(name.clone())
            }

            Expr::Lit { kind, .. } => Ok(self.lit_text(kind)),

            Expr::Interp { parts, .. } => self.emit_interp(parts),

            Expr::Binary { op, lhs, rhs, .. } => {
                let p = bin_prec(*op);
                let l = self.emit_expr(lhs, p)?;
                let r = self.emit_expr(rhs, p + 1)?;
                Ok(format!("{} {} {}", l, op.render(), r))
            }

            Expr::Unary { op, operand, .. } => {
                let o = self.emit_expr(operand, ATOM)?;
                Ok(format!("{}{}", op.render(), o))
            }

            Expr::AddrOf { operand, .. } => {
                let o = self.emit_expr(operand, ATOM)?;
                Ok(format!("&{}", o))
            }

            // The move marker is ownership bookkeeping (O5) — the checker
            // verified it; Go receives the plain value.
            Expr::Move { operand, .. } => self.emit_expr_inner(operand),

            Expr::Try { operand, .. } => Err(EmitError::unsupported(format!(
                "`?` outside a binding position (on `{}`) — E3 desugars binding-position `?` only",
                operand.render()
            ))),

            Expr::Receive { chan, .. } => {
                let c = self.emit_expr(chan, ATOM)?;
                Ok(format!("<-{}", c))
            }

            Expr::Index { base, index, .. } => {
                let b = self.emit_expr(base, ATOM)?;
                let i = self.emit_expr(index, 0)?;
                Ok(format!("{}[{}]", b, i))
            }

            Expr::Slice { base, lo, hi, .. } => {
                let b = self.emit_expr(base, ATOM)?;
                let lo_t = match lo {
                    Some(x) => self.emit_expr(x, 0)?,
                    None => String::new(),
                };
                let hi_t = match hi {
                    Some(x) => self.emit_expr(x, 0)?,
                    None => String::new(),
                };
                Ok(format!("{}[{}:{}]", b, lo_t, hi_t))
            }

            Expr::Field { base, name, .. } => {
                let b = self.emit_expr(base, ATOM)?;
                // Package-qualified members register the import.
                if let Expr::Ident { name: root, .. } = base.as_ref() {
                    if is_package_root(root) {
                        self.use_package(root);
                    }
                }
                Ok(format!("{}.{}", b, self.member_name(base, name)))
            }

            Expr::Call { callee, args, .. } => self.emit_call(callee, args),

            Expr::StructLit { ty, fields, .. } => {
                if let Some(t) = ty {
                    let head = self.type_text(t);
                    let body = self.field_inits(fields)?;
                    Ok(format!("{}{{{}}}", head, body))
                } else {
                    Err(EmitError::unsupported(
                        "type-elided struct literal — spell the type (MVP-0 emission surface)",
                    ))
                }
            }

            Expr::SliceLit { elem, items, .. } => {
                let head = match elem {
                    Some(t) => format!("[]{}", self.type_text(t)),
                    None => self.infer_slice_head(items)?,
                };
                let mut parts = Vec::new();
                for i in items {
                    parts.push(self.emit_expr(i, 0)?);
                }
                Ok(format!("{}{{{}}}", head, parts.join(", ")))
            }

            Expr::MapLit { key, value, entries, .. } => {
                let head = format!("map[{}]{}", self.type_text(key), self.type_text(value));
                let mut parts = Vec::new();
                for (k, v) in entries {
                    let kt = self.emit_expr(k, 0)?;
                    let vt = self.emit_expr(v, 0)?;
                    parts.push(format!("{}: {}", kt, vt));
                }
                Ok(format!("{}{{{}}}", head, parts.join(", ")))
            }

            Expr::FnLit { params, body, .. } => {
                let rendered_params = params
                    .iter()
                    .map(|p| format!("{} {}", p.name, self.type_text(&p.ty)))
                    .collect::<Vec<_>>()
                    .join(", ");
                // Hints from closure param types (example-06's w, r) —
                // scoped: removed again when the literal ends.
                let known: Vec<String> = self.bindings.keys().cloned().collect();
                for p in params {
                    if self.type_is_foreign(&p.ty) {
                        self.bindings.insert(p.name.clone(), BindingKind::Foreign);
                    }
                }
                self.fnlit_depth += 1;
                let mut out = String::from("func(");
                out.push_str(&rendered_params);
                out.push_str(") {\n");
                // Redirect statement output into a scratch buffer: the
                // literal renders inline, its body keeps normal indents.
                let main_body = std::mem::take(&mut self.body);
                self.indent += 1;
                for st in &body.stmts {
                    self.emit_stmt(st)?;
                }
                self.indent -= 1;
                let closure_body = std::mem::take(&mut self.body);
                self.body = main_body;
                self.fnlit_depth -= 1;
                self.bindings.retain(|k, _| known.contains(k));
                out.push_str(&closure_body);
                for _ in 0..self.indent {
                    out.push('\t');
                }
                out.push('}');
                Ok(out)
            }

            Expr::Paren { inner, .. } => {
                let i = self.emit_expr(inner, 0)?;
                Ok(format!("({})", i))
            }
        }
    }

    fn lit_text(&self, k: &LitKind) -> String {
        match k {
            LitKind::Int(s) | LitKind::Float(s) => s.clone(),
            LitKind::Bool(b) => b.to_string(),
            LitKind::Str(s) => go_quote(s),
            LitKind::RawStr(s) => format!("`{}`", s),
            LitKind::Char(c) => format!("{:?}", c),
            LitKind::Nil => "nil".to_string(),
        }
    }

    /// `"total={total}"` → `fmt.Sprintf("total=%v", total)`.
    /// All-literal interps stay plain strings (no Sprintf, no allocation).
    pub fn emit_interp(&mut self, parts: &[InterpPart]) -> Result<String, EmitError> {
        let has_expr = parts
            .iter()
            .any(|p| matches!(p, InterpPart::Expr(_)));
        if !has_expr {
            let mut s = String::new();
            for p in parts {
                if let InterpPart::Lit(l) = p {
                    s.push_str(l);
                }
            }
            return Ok(go_quote(&s));
        }
        let mut format_str = String::new();
        let mut args = Vec::new();
        for p in parts {
            match p {
                InterpPart::Lit(l) => {
                    format_str.push_str(&l.replace('%', "%%"));
                }
                InterpPart::Expr(x) => {
                    format_str.push_str("%v");
                    args.push(self.emit_expr(x, 0)?);
                }
            }
        }
        self.use_package("fmt");
        Ok(format!(
            "fmt.Sprintf({}, {})",
            go_quote(&format_str),
            args.join(", ")
        ))
    }

    fn field_inits(&mut self, fields: &[FieldInit]) -> Result<String, EmitError> {
        let mut parts = Vec::new();
        for f in fields {
            let v = self.emit_expr(&f.value, 0)?;
            parts.push(format!("{}: {}", f.name, v));
        }
        Ok(parts.join(", "))
    }

    /// `["a","b"]` with no element type: infer from the first item. Unknown
    /// shapes error cleanly — never a wrong emission.
    fn infer_slice_head(&mut self, items: &[Expr]) -> Result<String, EmitError> {
        let Some(first) = items.first() else {
            return Err(EmitError::unsupported(
                "empty `[]` literal — write `[]T{}` with the element type",
            ));
        };
        let elem = match first {
            Expr::Lit { kind, .. } => match kind {
                LitKind::Int(_) => "int",
                LitKind::Float(_) => "float64",
                LitKind::Str(_) | LitKind::RawStr(_) => "string",
                LitKind::Bool(_) => "bool",
                LitKind::Char(_) => "rune",
                LitKind::Nil => {
                    return Err(EmitError::unsupported("nil in an untyped slice literal"))
                }
            },
            Expr::StructLit { ty: Some(t), .. } => {
                return Ok(format!("[]{}", self.type_text(t)));
            }
            _ => {
                return Err(EmitError::unsupported(format!(
                    "untyped slice literal of `{}` — spell the element type",
                    first.render()
                )));
            }
        };
        Ok(format!("[]{}", elem))
    }

    // -----------------------------------------------------------------------
    // Calls — builtins, package specials, foreign methods
    // -----------------------------------------------------------------------

    fn emit_call(&mut self, callee: &Expr, args: &[CallArg]) -> Result<String, EmitError> {
        // --- make(...): the first argument is a TYPE (bug-#2 fix) ---
        if matches!(callee, Expr::Ident { name, .. } if name == "make") {
            return self.emit_make(args);
        }

        // --- package-qualified calls ---
        if let Expr::Field { base, name, .. } = callee {
            if let Expr::Ident { name: pkg, .. } = base.as_ref() {
                if is_package_root(pkg) {
                    return self.emit_pkg_call(pkg, name, args);
                }
            }
        }

        // --- method calls on foreign bindings normalize (M4) ---
        if let Expr::Field { base, name, .. } = callee {
            if let Expr::Ident { name: root, .. } = base.as_ref() {
                if let Some(BindingKind::Foreign) = self.bindings.get(root) {
                    let b = self.emit_expr(base, ATOM)?;
                    let mut parts = Vec::new();
                    for a in args {
                        parts.push(self.emit_call_arg(a)?);
                    }
                    return Ok(format!("{}.{}({})", b, to_go_name(name), parts.join(", ")));
                }
            }
        }

        // --- slice `.pop()` → prelude helper (example 08's stack shape) ---
        if let Expr::Field { base, name, .. } = callee {
            if name == "pop" {
                if let Expr::Ident { .. } = base.as_ref() {
                    let b = self.emit_expr(base, ATOM)?;
                    return Ok(format!("hy_pop(&{})", b));
                }
                return Err(EmitError::unsupported(
                    "`.pop()` on a non-local base (MVP-0 supports plain slice locals)",
                ));
            }
        }

        // --- associated-fn calls: `lru_cache.new[string, int](2)` →
        //     `lru_cache_new[string, int](2)` (Go has no dotted idents) ---
        if let Expr::Field { base, name, .. } = callee {
            if let Expr::Ident { name: root, .. } = base.as_ref() {
                if !is_package_root(root) && !self.bindings.contains_key(root) {
                    let (head, type_args) = split_type_args(name);
                    let full = format!("{}.{}", root, head);
                    if self.assoc_fns.contains(&full) {
                        let mut parts = Vec::new();
                        for a in args {
                            parts.push(self.emit_call_arg(a)?);
                        }
                        return Ok(format!(
                            "{}_{}{}({})",
                            root,
                            head,
                            type_args.unwrap_or_default(),
                            parts.join(", ")
                        ));
                    }
                }
            }
        }

        // --- everything else: callee(args...) ---
        let c = self.emit_expr(callee, ATOM)?;
        let mut parts = Vec::new();
        for a in args {
            parts.push(self.emit_call_arg(a)?);
        }
        Ok(format!("{}({})", c, parts.join(", ")))
    }

    fn emit_call_arg(&mut self, a: &CallArg) -> Result<String, EmitError> {
        let t = self.emit_expr(&a.value, 0)?;
        if a.spread {
            Ok(format!("{}...", t))
        } else {
            Ok(t)
        }
    }
    /// `make(chan job, 16)` / `make([]byte, 64*1024)` / `make(map[string]int)`.
    /// The parser encodes the type argument as an EMPTY composite literal of
    /// that type — rendering it as a value (`job{}`) was MVP-0 bug #2.
    fn emit_make(&mut self, args: &[CallArg]) -> Result<String, EmitError> {
        let Some(first) = args.first() else {
            return Err(EmitError::unsupported("`make` with no type argument"));
        };
        let ty_text = match &first.value {
            Expr::StructLit { ty: Some(t), fields, .. } if fields.is_empty() => self.type_text(t),
            Expr::SliceLit { elem: Some(t), items, .. } if items.is_empty() => {
                format!("[]{}", self.type_text(t))
            }
            Expr::MapLit { key, value, entries, .. } if entries.is_empty() => {
                format!("map[{}]{}", self.type_text(key), self.type_text(value))
            }
            other => {
                return Err(EmitError::unsupported(format!(
                    "`make` first argument must be a type; got value `{}`",
                    other.render()
                )));
            }
        };
        let mut parts = vec![ty_text];
        for a in &args[1..] {
            parts.push(self.emit_call_arg(a)?);
        }
        Ok(format!("make({})", parts.join(", ")))
    }

    fn emit_pkg_call(
        &mut self,
        pkg: &str,
        name: &str,
        args: &[CallArg],
    ) -> Result<String, EmitError> {
        self.use_package(pkg);

        if let Some(special) = special_member(pkg, name) {
            return match special {
                MemberSpecial::Var(text) => {
                    if args.is_empty() {
                        Ok(text.to_string())
                    } else {
                        Err(EmitError::unsupported(format!(
                            "{}.{} takes no arguments",
                            pkg, name
                        )))
                    }
                }
                MemberSpecial::StderrCall(go_name) => {
                    self.use_package("os");
                    let mut parts = vec!["os.Stderr".to_string()];
                    for a in args {
                        parts.push(self.emit_call_arg(a)?);
                    }
                    Ok(format!("{}.{}({})", pkg, go_name, parts.join(", ")))
                }
                MemberSpecial::Duration(unit) => {
                    self.use_package("time");
                    if args.len() != 1 {
                        return Err(EmitError::unsupported(format!(
                            "time.{} takes exactly one argument",
                            name
                        )));
                    }
                    let n = self.emit_call_arg(&args[0])?;
                    Ok(format!("time.Duration({}) * time.{}", n, unit))
                }
            };
        }

        // fmt.print on a byte buffer → os.Stdout.Write (a byte-slice dump
        // prints `[104 105]`, not the bytes — the reason the hint exists).
        if pkg == "fmt" && name == "print" && args.len() == 1 {
            if let Expr::Ident { name: b, .. } = &args[0].value {
                if let Some(BindingKind::Bytes) = self.bindings.get(b) {
                    self.use_package("os");
                    let t = self.emit_call_arg(&args[0])?;
                    return Ok(format!("os.Stdout.Write({})", t));
                }
            }
        }

        let mut parts = Vec::new();
        for a in args {
            parts.push(self.emit_call_arg(a)?);
        }
        Ok(format!("{}.{}({})", pkg, to_go_name(name), parts.join(", ")))
    }
}

/// `new[string, int]` → ("new", Some("[string, int]")).
fn split_type_args(name: &str) -> (&str, Option<&str>) {
    match name.find('[') {
        Some(i) => (&name[..i], Some(&name[i..])),
        None => (name, None),
    }
}

// ---------------------------------------------------------------------------
// Total-rendering helper for type positions (never fail, never panic)
// ---------------------------------------------------------------------------

impl Emitter {
    /// Expression text for contexts that cannot hold a `?`/match (array
    /// lengths, etc.). Falls back to the AST's lossy `render()` — a
    /// description, never a wrong emission.
    pub fn expr_text_static(&mut self, e: &Expr) -> String {
        match self.emit_expr(e, 0) {
            Ok(t) => t,
            Err(err) => format!("{} /* unsupported: {} */", e.render(), err.message),
        }
    }
}
