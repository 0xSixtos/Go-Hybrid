//! Statement emission — `Emitter` impl part 2 (see lib.rs for the context).

use hy_syntax::*;

use super::{is_package_root, to_go_name, BindingKind, EmitError, Emitter};

impl Emitter {
    pub fn emit_stmt(&mut self, s: &Stmt) -> Result<(), EmitError> {
        match s {
            Stmt::Local(l) => self.emit_local(l),
            Stmt::Assign { targets, op, values, .. } => self.emit_assign(targets, *op, values),
            Stmt::Expr { expr, .. } => {
                let text = self.emit_expr(expr, 0)?;
                self.push(&text);
                Ok(())
            }
            Stmt::Send { chan, value, is_move: _, .. } => {
                // `move` markers drop here: ownership transfer is a CHECKER
                // concept; Go sends the value either way.
                let ch = self.emit_expr(chan, 0)?;
                let v = self.emit_expr(value, 0)?;
                self.push(&format!("{} <- {}", ch, v));
                Ok(())
            }
            Stmt::If { init, cond, then, otherwise, .. } => {
                self.emit_if(init.as_deref(), cond, then, otherwise.as_deref())
            }
            Stmt::For { kind, body, .. } => self.emit_for(kind, body),
            Stmt::Return { values, .. } => self.emit_return(values),
            Stmt::Task { callee, args, .. } => {
                // task → go (spawn sites were checked by Tier-0; emission is
                // a plain goroutine).
                let c = self.emit_expr(callee, 0)?;
                let mut parts = Vec::new();
                for a in args {
                    parts.push(self.emit_expr(&a.value, 0)?);
                }
                self.push(&format!("go {}({})", c, parts.join(", ")));
                Ok(())
            }
            Stmt::Managed { body, .. } => {
                // The escape hatch emits a bare block. The audit marker stays
                // greppable in the generated source.
                self.push("{ // managed (unchecked share — audited via hyb check --report-managed)");
                self.indent += 1;
                for st in &body.stmts {
                    self.emit_stmt(st)?;
                }
                self.indent -= 1;
                self.push("}");
                Ok(())
            }
            Stmt::Defer { call, .. } => {
                let c = self.emit_expr(call, 0)?;
                self.push(&format!("defer {}", c));
                Ok(())
            }
            Stmt::Break { label, .. } => {
                // `break loop` → Go label `hyLoop` (`loop` is a Go keyword).
                // The enclosing loop labels itself when it owns a labeled
                // break — see emit_for.
                match label {
                    Some(_) => self.push("break hyLoop"),
                    None => self.push("break"),
                }
                Ok(())
            }
            Stmt::Continue { label, .. } => {
                match label {
                    Some(_) => self.push("continue hyLoop"),
                    None => self.push("continue"),
                }
                Ok(())
            }
            Stmt::Block { block, .. } => {
                self.push("{");
                self.indent += 1;
                for st in &block.stmts {
                    self.emit_stmt(st)?;
                }
                self.indent -= 1;
                self.push("}");
                Ok(())
            }
            Stmt::Match { subject, .. } => Err(EmitError::unsupported(format!(
                "match (over `{}`) — the enum/match surface lands with the generics batch",
                subject.render()
            ))),
            Stmt::Select { arms, .. } => {
                // Go select, 1:1 — Hybrid's arm spelling
                // (`v, ok := <-sq:` / `<-deadline:` / `ch <- v:`) lowers to
                // Go's case clauses.
                self.push("select {");
                self.indent += 1;
                for arm in arms {
                    match &arm.kind {
                        SelectArmKind::Recv { names, chan } => {
                            let c = self.emit_expr(chan, 0)?;
                            let head = match names.len() {
                                0 => format!("case <-{}:", c),
                                1 => format!("case {} := <-{}:", names[0], c),
                                2 => format!("case {}, {} := <-{}:", names[0], names[1], c),
                                n => {
                                    return Err(EmitError::unsupported(format!(
                                        "select receive with {} names",
                                        n
                                    )));
                                }
                            };
                            self.push(&head);
                        }
                        SelectArmKind::Send { chan, value } => {
                            let c = self.emit_expr(chan, 0)?;
                            let v = self.emit_expr(value, 0)?;
                            self.push(&format!("case {} <- {}:", c, v));
                        }
                        SelectArmKind::Default => self.push("default:"),
                    }
                    self.indent += 1;
                    for st in &arm.body.stmts {
                        self.emit_stmt(st)?;
                    }
                    self.indent -= 1;
                }
                self.indent -= 1;
                self.push("}");
                Ok(())
            }
        }
    }

    // -----------------------------------------------------------------------
    // Local bindings (Define / Let / Var / Const) + the `?` desugar (E3)
    // -----------------------------------------------------------------------

    fn emit_local(&mut self, l: &LocalDecl) -> Result<(), EmitError> {
        match l.kind {
            LocalKind::Var => {
                let ty = l.ty.as_ref().map(|t| self.type_text(t));
                match (&ty, l.values.first()) {
                    (Some(t), None) => self.push(&format!("var {} {}", l.names.join(", "), t)),
                    (Some(t), Some(v)) => {
                        let e = self.emit_expr(v, 0)?;
                        self.push(&format!("var {} {} = {}", l.names.join(", "), t, e));
                    }
                    (None, Some(v)) => {
                        let e = self.emit_expr(v, 0)?;
                        self.push(&format!("var {} = {}", l.names.join(", "), e));
                    }
                    (None, None) => {
                        return Err(EmitError::unsupported(
                            "`var x` without a type or initializer",
                        ));
                    }
                }
                self.hint_from_values(&l.names, &l.values);
                return Ok(());
            }
            LocalKind::Const => {
                let value = self.emit_expr(l.values.first().ok_or_else(|| {
                    EmitError::unsupported("`const` without an initializer")
                })?, 0)?;
                let names = l.names.join(", ");
                match &l.ty {
                    Some(t) => {
                        let ty = self.type_text(t);
                        self.push(&format!("const {} {} = {}", names, ty, value));
                    }
                    None => self.push(&format!("const {} = {}", names, value)),
                }
                return Ok(());
            }
            // `:=` and `let` both render as Go's short declaration —
            // immutability is a checker-level contract (D14 folded
            // `let x := e` into this shape).
            LocalKind::Define | LocalKind::Let => {}
        }

        // `x := f()?` — the try desugar. Values.len() == 1, a Try node.
        if l.values.len() == 1 {
            if let Expr::Try { operand, .. } = &l.values[0] {
                return self.emit_try_define(l, operand);
            }
        }

        let values = l
            .values
            .iter()
            .map(|v| self.emit_expr(v, 0))
            .collect::<Result<Vec<_>, _>>()?;
        let names = l.names.join(", ");
        match (l.ty.as_ref(), values.len()) {
            (Some(_), _) => {
                return Err(EmitError::unsupported(
                    "type-ascribed `:=` — write `var x T = e` or `x := e`",
                ));
            }
            (None, _) => self.push(&format!("{} := {}", names, values.join(", "))),
        }
        self.hint_from_values(&l.names, &l.values);
        Ok(())
    }

    /// E3 desugaring, wrapper-aware:
    ///
    /// ```text
    /// x := f()?
    /// ```
    /// becomes (inside the main wrapper closure — `hyErr` is its named
    /// error result; unique temps keep `:=` always-fresh at any depth):
    ///
    /// ```go
    /// x, hyErr0 := f()
    /// if hyErr0 != nil {
    ///     hyErr = hyErr0
    ///     return
    /// }
    /// ```
    fn emit_try_define(&mut self, l: &LocalDecl, call: &Expr) -> Result<(), EmitError> {
        let err_name = if self.in_main_closure {
            "hyErr".to_string()
        } else {
            return Err(EmitError::unsupported(
                "`?` outside main (functions-with-error-results land after the walking skeleton)",
            ));
        };
        let temp = self.next_temp("hyErr");
        let call_text = self.emit_expr(call, 0)?;
        let mut names = l.names.clone();
        names.push(temp.clone());
        self.push(&format!("{} := {}", names.join(", "), call_text));
        self.push(&format!("if {} != nil {{", temp));
        self.indent += 1;
        self.push(&format!("{} = {}", err_name, temp));
        self.push("return");
        self.indent -= 1;
        self.push("}");
        self.hint_from_values(&l.names, std::slice::from_ref(call));
        Ok(())
    }

    /// Binding hints from the defining values (see `BindingKind`).
    fn hint_from_values(&mut self, names: &[String], values: &[Expr]) {
        if names.len() != 1 || values.len() != 1 {
            return;
        }
        let kind = self.binding_kind_of(&values[0]);
        if kind != BindingKind::Plain {
            self.bindings.insert(names[0].clone(), kind);
        }
    }

    fn binding_kind_of(&self, e: &Expr) -> BindingKind {
        let call = match e {
            Expr::Try { operand, .. } => match operand.as_ref() {
                Expr::Call { callee, .. } => callee.as_ref(),
                _ => return BindingKind::Plain,
            },
            Expr::Call { callee, .. } => callee.as_ref(),
            _ => return BindingKind::Plain,
        };
        if let Expr::Field { base, name, .. } = call {
            if let Expr::Ident { name: pkg, .. } = base.as_ref() {
                if is_package_root(pkg) {
                    if pkg == "io" && name == "read_all" {
                        return BindingKind::Bytes;
                    }
                    return BindingKind::Foreign;
                }
            }
        }
        BindingKind::Plain
    }

    // -----------------------------------------------------------------------
    // Assignment
    // -----------------------------------------------------------------------

    fn emit_assign(
        &mut self,
        targets: &[Place],
        op: AssignOp,
        values: &[Expr],
    ) -> Result<(), EmitError> {
        let op_text = match op {
            AssignOp::Assign => "=",
            AssignOp::AddAssign => "+=",
            AssignOp::SubAssign => "-=",
            AssignOp::MulAssign => "*=",
            AssignOp::DivAssign => "/=",
            AssignOp::ModAssign => "%=",
            AssignOp::Inc => {
                let t = self.emit_place(&targets[0])?;
                self.push(&format!("{}++", t));
                return Ok(());
            }
            AssignOp::Dec => {
                let t = self.emit_place(&targets[0])?;
                self.push(&format!("{}--", t));
                return Ok(());
            }
        };
        let ts = targets
            .iter()
            .map(|t| self.emit_place(t))
            .collect::<Result<Vec<_>, _>>()?;
        let vs = values
            .iter()
            .map(|v| self.emit_expr(v, 0))
            .collect::<Result<Vec<_>, _>>()?;
        self.push(&format!("{} {} {}", ts.join(", "), op_text, vs.join(", ")));
        Ok(())
    }

    /// A place in Go spelling. Field members on Foreign bindings normalize
    /// (M4); our own struct members stay verbatim.
    pub fn emit_place(&mut self, p: &Place) -> Result<String, EmitError> {
        match &p.kind {
            PlaceKind::Ident(n) => Ok(n.clone()),
            PlaceKind::Index { base, index } => {
                let b = self.emit_place(base)?;
                let i = self.emit_expr(index, 0)?;
                Ok(format!("{}[{}]", b, i))
            }
            PlaceKind::Field { base, name } => {
                let b = self.emit_place(base)?;
                Ok(format!("{}.{}", b, self.place_member_name(base, name)))
            }
            PlaceKind::AddrOf { base } => {
                let b = self.emit_place(base)?;
                Ok(format!("&{}", b))
            }
        }
    }

    /// Member-name normalization for a PLACE (assignment target). Same rule
    /// as the expression side: foreign package roots and Foreign bindings
    /// normalize; our own members stay verbatim.
    pub fn place_member_name(&self, base: &Place, name: &str) -> String {
        let root = base.root();
        if is_package_root(root) || matches!(self.bindings.get(root), Some(BindingKind::Foreign)) {
            return super::to_go_name(name);
        }
        name.to_string()
    }

    /// Member-name normalization decision for a field/method access whose
    /// base's ROOT binding is `base` (an Ident chain root).
    pub fn member_name(&self, base: &Expr, name: &str) -> String {
        if let Expr::Ident { name: root, .. } = base {
            if is_package_root(root) {
                return to_go_name(name);
            }
            if let Some(BindingKind::Foreign) = self.bindings.get(root) {
                return to_go_name(name);
            }
        }
        name.to_string()
    }

    // -----------------------------------------------------------------------
    // Control flow
    // -----------------------------------------------------------------------

    fn emit_if(
        &mut self,
        init: Option<&Stmt>,
        cond: &Expr,
        then: &Block,
        otherwise: Option<&Stmt>,
    ) -> Result<(), EmitError> {
        self.emit_if_at(init, cond, then, otherwise, true)
    }

    /// `at_line_start=false` renders the head as a `} else if …` CONTINUATION
    /// on the current line (else-if chains), never a fresh indented line.
    fn emit_if_at(
        &mut self,
        init: Option<&Stmt>,
        cond: &Expr,
        then: &Block,
        otherwise: Option<&Stmt>,
        at_line_start: bool,
    ) -> Result<(), EmitError> {
        let mut head = String::new();
        if at_line_start {
            for _ in 0..self.indent {
                head.push('\t');
            }
            head.push_str("if ");
        } else {
            head.push_str("if ");
        }
        if let Some(i) = init {
            // Go: `if <stmt>; <cond> {` — the init renders without newline.
            let init_text = self.stmt_inline(i)?;
            head.push_str(&init_text);
            head.push_str("; ");
        }
        let c = self.emit_expr(cond, 0)?;
        head.push_str(&c);
        head.push_str(" {");
        self.push_raw(&head);
        self.push_raw("\n");
        self.indent += 1;
        for st in &then.stmts {
            self.emit_stmt(st)?;
        }
        self.indent -= 1;
        match otherwise {
            None => self.push("}"),
            Some(Stmt::If { init, cond, then, otherwise, .. }) => {
                self.push_raw("} else ");
                self.emit_if_at(init.as_deref(), cond, then, otherwise.as_deref(), false)?;
            }
            Some(Stmt::Block { block, .. }) => {
                // `else { … }` — emit the block's statements directly (a
                // bare-block emit here would double the braces).
                self.push("} else {");
                self.indent += 1;
                for st in &block.stmts {
                    self.emit_stmt(st)?;
                }
                self.indent -= 1;
                self.push("}");
            }
            Some(other) => {
                self.push("} else {");
                self.indent += 1;
                self.emit_stmt(other)?;
                self.indent -= 1;
                self.push("}");
            }
        }
        Ok(())
    }

    /// Inline statement rendering for if-init (`v, ok := m[k]`).
    fn stmt_inline(&mut self, s: &Stmt) -> Result<String, EmitError> {
        match s {
            Stmt::Local(l) => {
                // try-desugar cannot appear in an if-init (the init cond is
                // checked by the checker) — render plainly.
                let values = l
                    .values
                    .iter()
                    .map(|v| self.emit_expr(v, 0))
                    .collect::<Result<Vec<_>, _>>()?;
                let names = l.names.join(", ");
                Ok(match (&l.ty, values.len()) {
                    (Some(t), 1) => format!("{} := {} {}", names, self.type_text(t), values[0]),
                    _ => format!("{} := {}", names, values.join(", ")),
                })
            }
            Stmt::Expr { expr, .. } => self.emit_expr(expr, 0),
            _ => Err(EmitError::unsupported(
                "that statement form in an if-init (MVP-0 supports defines)",
            )),
        }
    }

    fn emit_for(&mut self, kind: &ForKind, body: &Block) -> Result<(), EmitError> {
        // A labeled `break loop` / `continue loop` inside this loop (NOT
        // inside a nested loop — that one labels itself) makes this loop the
        // Go label target: `hyLoop: for { … break hyLoop … }`.
        let needs_label = self.block_has_loop_break(body);
        let label_prefix = if needs_label { "hyLoop: " } else { "" };
        let head = match kind {
            ForKind::Infinite => format!("{}for {{", label_prefix),
            ForKind::While { cond } => format!("{}for {} {{", label_prefix, self.emit_expr(cond, 0)?),
            ForKind::Range { var, lo, hi, inclusive } => {
                let lo_t = self.emit_expr(lo, 0)?;
                let hi_t = self.emit_expr(hi, 0)?;
                let op = if *inclusive { "<=" } else { "<" };
                format!(
                    "{}for {} := {}; {} {} {}; {}++ {{",
                    label_prefix, var, lo_t, var, op, hi_t, var
                )
            }
            ForKind::Iterate { vars, expr } => {
                let e = self.emit_expr(expr, 0)?;
                let core = match vars.len() {
                    1 => format!("for {} := range {} {{", vars[0], e),
                    2 => format!("for {}, {} := range {} {{", vars[0], vars[1], e),
                    n => {
                        return Err(EmitError::unsupported(format!(
                            "for-in with {} loop variables",
                            n
                        )))
                    }
                };
                format!("{}{}", label_prefix, core)
            }
        };
        self.push(&head);
        self.indent += 1;
        for st in &body.stmts {
            self.emit_stmt(st)?;
        }
        self.indent -= 1;
        self.push("}");
        Ok(())
    }

    /// Does this block contain a labeled break/continue whose nearest
    /// enclosing loop is THIS one? Does not descend into nested `for`
    /// statements (a nested loop that owns a labeled break labels itself).
    fn block_has_loop_break(&self, b: &Block) -> bool {
        b.stmts.iter().any(|s| self.stmt_has_loop_break(s))
    }

    fn stmt_has_loop_break(&self, s: &Stmt) -> bool {
        match s {
            Stmt::Break { label: Some(_), .. } | Stmt::Continue { label: Some(_), .. } => true,
            Stmt::If { init, then, otherwise, .. } => {
                init.as_ref().map(|i| self.stmt_has_loop_break(i)).unwrap_or(false)
                    || self.block_has_loop_break(then)
                    || otherwise.as_ref().map(|o| self.stmt_has_loop_break(o)).unwrap_or(false)
            }
            Stmt::For { .. } => false, // nested loop labels itself
            Stmt::Block { block, .. } | Stmt::Managed { body: block, .. } => {
                self.block_has_loop_break(block)
            }
            Stmt::Select { arms, .. } => arms.iter().any(|a| self.block_has_loop_break(&a.body)),
            _ => false,
        }
    }

    // -----------------------------------------------------------------------
    // Return (wrapper-aware)
    // -----------------------------------------------------------------------

    fn emit_return(&mut self, values: &[Expr]) -> Result<(), EmitError> {
        // Inside a function literal the closure owns the return — plain.
        if self.fnlit_depth > 0 {
            let vs = values
                .iter()
                .map(|v| self.emit_expr(v, 0))
                .collect::<Result<Vec<_>, _>>()?;
            self.push(&format!("return {}", vs.join(", ")));
            return Ok(());
        }
        // Inside the main wrapper closure: explicit returns carry `nil` for
        // the hidden error result; bare `return` is legal (named results)
        // and returns the current hyErr — exactly the E3 desugar's shape.
        if self.in_main_closure {
            if values.is_empty() {
                self.push("return");
                return Ok(());
            }
            let vs = values
                .iter()
                .map(|v| self.emit_expr(v, 0))
                .collect::<Result<Vec<_>, _>>()?;
            self.push(&format!("return {}, nil", vs.join(", ")));
            return Ok(());
        }
        let vs = values
            .iter()
            .map(|v| self.emit_expr(v, 0))
            .collect::<Result<Vec<_>, _>>()?;
        self.push(&format!("return {}", vs.join(", ")));
        Ok(())
    }
}
