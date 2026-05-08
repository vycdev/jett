use std::collections::{HashMap, HashSet};

use jett_common::{FileId, Span};
use jett_diagnostics::{Diagnostic, DiagnosticSink};
use jett_parser::ast::{
    ActorDef, AssertStmt, AssignStmt, Block, BreakpointStmt, CallArg, ComptimeTypeBindStmt, Expr,
    ExprStmt, ForStmt, FunctionDecl, FunctionDef, IfStmt, Item, MatchStmt, Module, Pattern,
    RespondStmt, ReturnStmt, Stmt, StringPart, TraceStmt, TypeAlias, TypeExpr, UseDecl, VarDecl,
    WhileStmt,
};

use crate::errors;
use crate::scope::{DefId, DefKind, ScopeId, ScopeTable};

/// The result of name resolution.
#[derive(Debug)]
pub struct ResolveResult {
    /// The complete scope and definition table.
    pub scope_table: ScopeTable,
    /// Map from AST node spans to the `DefId` they resolved to.
    pub resolutions: HashMap<Span, DefId>,
    /// Diagnostics emitted during resolution.
    pub diagnostics: Vec<Diagnostic>,
}

/// Resolve all names in a parsed `Module`.
pub fn resolve(module: &Module) -> ResolveResult {
    let mut resolver = Resolver::new();
    resolver.resolve_module(module);
    resolver.check_unused();
    ResolveResult {
        scope_table: resolver.scope_table,
        resolutions: resolver.resolutions,
        diagnostics: resolver.sink.into_diagnostics(),
    }
}

// ---------------------------------------------------------------------------
// Internal resolver state
// ---------------------------------------------------------------------------

struct Resolver {
    scope_table: ScopeTable,
    resolutions: HashMap<Span, DefId>,
    sink: DiagnosticSink,
    /// The current scope during the walk.
    current_scope: ScopeId,
    /// Namespace introduced by the latest `namespace` item in the current file.
    current_namespace: Option<String>,
    /// File for the last top-level item walked; namespaces do not leak across files.
    current_file: Option<FileId>,
    /// Set of DefIds that have been referenced (for unused detection).
    used_defs: HashSet<DefId>,
    /// Track which definitions are `use` imports (for unused-import warnings).
    use_defs: HashSet<DefId>,
    /// Track which definitions are variables/params (for unused-variable warnings).
    var_defs: HashSet<DefId>,
    /// During pass-1 of top-level items we record each name and the *index*
    /// at which it was declared so that pass-2 can detect forward references.
    /// The map goes name -> (DefId, declaration order index).
    top_level_order: HashMap<String, (DefId, usize)>,
    /// Functions predeclared by a `mutual` block: name -> declaration span.
    mutual_declarations: HashMap<String, Span>,
    /// Functions from `mutual` blocks that already have a real body definition.
    mutual_definitions: HashMap<String, Span>,
    /// Type parameter names that are currently in scope (e.g. `T`, `U` in a generic struct).
    /// Names in this set are suppressed from "undefined name" errors.
    active_type_params: HashSet<String>,
}

impl Resolver {
    fn new() -> Self {
        let mut scope_table = ScopeTable::new();
        let root = scope_table.new_scope(None);

        let dummy_span = Span::new(jett_common::FileId::new(0), 0, 0);

        // Pre-register built-in type names so they don't trigger "undefined" errors.
        let builtins = [
            // Primitive types
            "int8",
            "int16",
            "int32",
            "int64",
            "uint8",
            "uint16",
            "uint32",
            "uint64",
            "float32",
            "float64",
            "string",
            "bool",
            "bytes",
            "nothing",
            "JsonValue",
            "TypeConstruction",
            // Built-in generic types (used as identifiers in type annotations)
            "list",
            "map",
            "set",
            "optional",
            "result",
            "secret",
            "TypeInfo",
            "TypeKind",
            "TypePrimitive",
            "TypeField",
            "TypeBitfield",
            "TypeBitfieldField",
            "TypeBitfieldFieldShape",
            "TypeVariant",
            "type",
            // Capability types
            "Stdout",
            "Stderr",
            "Stdin",
            "Filesystem",
            "Network",
            "Clock",
            "Random",
            "Process",
            "Environment",
            // Common built-in functions/values
            "true",
            "false",
            "none",
            "range",
            "print",
            "println",
        ];
        for name in builtins {
            let def = scope_table.new_def(name.to_string(), DefKind::Constant, dummy_span);
            scope_table.bind(root, name.to_string(), def);
        }

        Self {
            scope_table,
            resolutions: HashMap::new(),
            sink: DiagnosticSink::new(),
            current_scope: root,
            current_namespace: None,
            current_file: None,
            used_defs: HashSet::new(),
            use_defs: HashSet::new(),
            var_defs: HashSet::new(),
            top_level_order: HashMap::new(),
            mutual_declarations: HashMap::new(),
            mutual_definitions: HashMap::new(),
            active_type_params: HashSet::new(),
        }
    }

    // ------------------------------------------------------------------
    // Module
    // ------------------------------------------------------------------

    fn resolve_module(&mut self, module: &Module) {
        // Pass 1: register top-level declarations in order.
        self.reset_namespace_tracking();
        self.pass1_declarations(&module.items);

        // Pass 2: walk bodies and resolve references.
        self.reset_namespace_tracking();
        self.pass2_references(&module.items);
    }

    // ------------------------------------------------------------------
    // Pass 1 — top-level declarations (strict top-to-bottom order)
    // ------------------------------------------------------------------

    fn pass1_declarations(&mut self, items: &[Item]) {
        for (index, item) in items.iter().enumerate() {
            self.update_current_namespace(item);
            match item {
                Item::Namespace(ns) => {
                    self.declare_top_level(&ns.name.name, DefKind::Namespace, ns.span, index);
                }
                Item::Function(func) => {
                    self.declare_function_top_level(func, index);
                }
                Item::Mutual(block) => {
                    for decl in &block.declarations {
                        if let Some(def_id) = self.declare_top_level(
                            &decl.name.name,
                            DefKind::Function,
                            decl.name.span,
                            index,
                        ) {
                            self.resolutions.insert(decl.name.span, def_id);
                            self.mutual_declarations
                                .insert(decl.name.name.clone(), decl.name.span);
                        }
                    }
                }
                Item::Interface(interface) => {
                    self.declare_namespaced_top_level(
                        &interface.name.name,
                        DefKind::Interface,
                        interface.name.span,
                        index,
                    );
                }
                Item::Struct(s) => {
                    self.declare_namespaced_top_level(
                        &s.name.name,
                        DefKind::Struct,
                        s.name.span,
                        index,
                    );
                }
                Item::Bitfield(b) => {
                    self.declare_namespaced_top_level(
                        &b.name.name,
                        DefKind::Bitfield,
                        b.name.span,
                        index,
                    );
                }
                Item::Enum(e) => {
                    self.declare_namespaced_top_level(
                        &e.name.name,
                        DefKind::Enum,
                        e.name.span,
                        index,
                    );
                }
                Item::VarDecl(v) => {
                    let def_id = self.declare_namespaced_top_level(
                        &v.name.name,
                        DefKind::Variable,
                        v.name.span,
                        index,
                    );
                    if let Some(id) = def_id {
                        self.var_defs.insert(id);
                    }
                }
                Item::Machine(m) => {
                    self.declare_namespaced_top_level(
                        &m.name.name,
                        DefKind::Machine,
                        m.name.span,
                        index,
                    );
                }
                Item::Actor(a) => {
                    self.declare_namespaced_top_level(
                        &a.name.name,
                        DefKind::Actor,
                        a.name.span,
                        index,
                    );
                }
                Item::TypeAlias(ta) => {
                    self.declare_namespaced_top_level(
                        &ta.name.name,
                        DefKind::Struct, // treat type aliases like types for now
                        ta.name.span,
                        index,
                    );
                }
                // Verify, property, and implement blocks don't declare new names in the module scope.
                Item::Verify(_) | Item::Property(_) | Item::Implement(_) => {}
            }
        }
    }

    fn declare_function_top_level(&mut self, func: &FunctionDef, order: usize) {
        if self.mutual_declarations.contains_key(&func.name.name) {
            if let Some(prev_span) = self
                .mutual_definitions
                .insert(func.name.name.clone(), func.name.span)
            {
                self.sink.emit(errors::duplicate_definition(
                    &func.name.name,
                    func.name.span,
                    prev_span,
                ));
                return;
            }

            let def_id = self
                .scope_table
                .lookup_local(self.current_scope, &func.name.name)
                .expect("mutual declaration must already be in the current scope");
            self.resolutions.insert(func.name.span, def_id);
            return;
        }

        self.declare_namespaced_top_level(
            &func.name.name,
            DefKind::Function,
            func.name.span,
            order,
        );
    }

    /// Register a top-level name. Returns `Some(DefId)` on success, `None` if
    /// a duplicate was detected.
    fn declare_top_level(
        &mut self,
        name: &str,
        kind: DefKind,
        span: Span,
        order: usize,
    ) -> Option<DefId> {
        // Check for duplicate in the current scope.
        if let Some(prev_id) = self.scope_table.lookup_local(self.current_scope, name) {
            let prev_span = self.scope_table.def(prev_id).span;
            self.sink
                .emit(errors::duplicate_definition(name, span, prev_span));
            return None;
        }
        let def_id = self.scope_table.new_def(name.to_string(), kind, span);
        self.scope_table
            .bind(self.current_scope, name.to_string(), def_id);
        self.top_level_order
            .insert(name.to_string(), (def_id, order));
        Some(def_id)
    }

    fn current_qualified_name(&self, name: &str) -> Option<String> {
        if name.contains('.') {
            return None;
        }
        self.current_namespace
            .as_ref()
            .map(|namespace| format!("{namespace}.{name}"))
    }

    fn reset_namespace_tracking(&mut self) {
        self.current_namespace = None;
        self.current_file = None;
    }

    fn item_file(item: &Item) -> FileId {
        match item {
            Item::Namespace(ns) => ns.span.file,
            Item::Function(func) => func.span.file,
            Item::Mutual(block) => block.span.file,
            Item::Interface(interface) => interface.span.file,
            Item::Implement(block) => block.span.file,
            Item::Struct(strukt) => strukt.span.file,
            Item::Bitfield(bitfield) => bitfield.span.file,
            Item::Enum(enm) => enm.span.file,
            Item::Machine(machine) => machine.span.file,
            Item::Actor(actor) => actor.span.file,
            Item::VarDecl(decl) => decl.span.file,
            Item::Verify(verify) => verify.span.file,
            Item::Property(prop) => prop.span.file,
            Item::TypeAlias(alias) => alias.span.file,
        }
    }

    fn update_current_namespace(&mut self, item: &Item) {
        let item_file = Self::item_file(item);
        if self.current_file.is_some_and(|file| file != item_file) {
            self.current_namespace = None;
        }
        self.current_file = Some(item_file);

        if let Item::Namespace(ns) = item {
            self.current_namespace = Some(ns.name.name.clone());
        }
    }

    fn declare_namespaced_top_level(
        &mut self,
        name: &str,
        kind: DefKind,
        span: Span,
        order: usize,
    ) -> Option<DefId> {
        let Some(qualified) = self.current_qualified_name(name) else {
            return self.declare_top_level(name, kind, span, order);
        };

        let def_id = self.declare_top_level(&qualified, kind, span, order)?;

        if self
            .scope_table
            .lookup_local(self.current_scope, name)
            .is_none()
        {
            self.scope_table
                .bind(self.current_scope, name.to_string(), def_id);
            self.top_level_order
                .insert(name.to_string(), (def_id, order));
        }

        Some(def_id)
    }

    // ------------------------------------------------------------------
    // Pass 2 — resolve references inside bodies
    // ------------------------------------------------------------------

    fn pass2_references(&mut self, items: &[Item]) {
        for (index, item) in items.iter().enumerate() {
            self.update_current_namespace(item);
            match item {
                Item::Mutual(block) => {
                    for decl in &block.declarations {
                        self.resolve_function_decl(decl, index);
                    }
                }
                Item::Interface(interface) => {
                    for method in &interface.methods {
                        self.resolve_function_decl(method, index);
                    }
                }
                Item::Implement(block) => {
                    self.resolve_name(&block.interface_name.name, block.interface_name.span, index);
                    self.resolve_type_expr(&block.for_type, index);
                    for method in &block.methods {
                        self.resolve_function(method, index);
                    }
                }
                Item::TypeAlias(alias) => {
                    self.resolve_type_alias(alias, index);
                }
                Item::Function(func) => {
                    self.resolve_function(func, index);
                }
                Item::Struct(s) => {
                    // For generic structs, add type params to active set so
                    // uses of T/U etc. in field types are not reported as errors.
                    let added_params: Vec<String> = s
                        .type_params
                        .iter()
                        .filter(|p| self.active_type_params.insert(p.name.clone()))
                        .map(|p| p.name.clone())
                        .collect();

                    for field in &s.fields {
                        self.resolve_type_expr(&field.ty, index);
                    }
                    // Resolve method bodies.
                    for method in &s.methods {
                        self.resolve_function(method, index);
                    }

                    for param in added_params {
                        self.active_type_params.remove(&param);
                    }
                }
                Item::Bitfield(bitfield) => {
                    self.resolve_bitfield(bitfield, index);
                }
                Item::VarDecl(v) => {
                    self.resolve_expr(&v.value, index);
                }
                Item::Actor(actor) => {
                    self.resolve_actor(actor, index);
                }
                // Namespace and Enum declarations have no bodies to walk.
                _ => {}
            }
        }
    }

    fn resolve_actor(&mut self, actor: &ActorDef, item_index: usize) {
        let actor_scope = self.push_scope();

        // Capability parameters are in scope for the whole actor.
        for param in &actor.capability_params {
            self.resolve_type_expr(&param.ty, item_index);
            self.declare_local(&param.name.name, DefKind::Param, param.name.span);
        }

        // State fields are in scope for handlers.
        for field in &actor.state_fields {
            self.resolve_expr(&field.value, item_index);
            self.declare_local(&field.name.name, DefKind::Variable, field.name.span);
        }

        // Each receive handler gets its own inner scope for message params.
        for handler in &actor.handlers {
            let handler_scope = self.push_scope();
            for param in &handler.params {
                self.resolve_type_expr(&param.ty, item_index);
                self.declare_local(&param.name.name, DefKind::Param, param.name.span);
            }
            if let Some(responds) = &handler.responds {
                self.resolve_type_expr(responds, item_index);
            }
            self.resolve_block(&handler.body, item_index);
            self.pop_scope(handler_scope);
        }

        self.pop_scope(actor_scope);
    }

    // ------------------------------------------------------------------
    // Functions
    // ------------------------------------------------------------------

    fn resolve_function_decl(&mut self, decl: &FunctionDecl, item_index: usize) {
        for param in &decl.params {
            self.resolve_type_expr(&param.ty, item_index);
        }
        if let Some(return_type) = &decl.return_type {
            self.resolve_type_expr(return_type, item_index);
        }
    }

    fn resolve_type_alias(&mut self, alias: &TypeAlias, item_index: usize) {
        self.resolve_type_expr(&alias.base_type, item_index);

        if let Some(constraint) = &alias.constraint {
            let scope = self.push_scope();
            self.declare_local_without_unused_warning(
                "value",
                DefKind::Variable,
                constraint.span(),
            );
            self.resolve_expr(constraint, item_index);
            self.pop_scope(scope);
        }
    }

    fn resolve_bitfield(&mut self, bitfield: &jett_parser::ast::BitfieldDef, item_index: usize) {
        for field in &bitfield.fields {
            match &field.kind {
                jett_parser::ast::BitfieldFieldKind::Bits { as_type, .. } => {
                    if let Some(ty) = as_type {
                        self.resolve_type_expr(ty, item_index);
                    }
                }
                jett_parser::ast::BitfieldFieldKind::Payload(ty) => {
                    self.resolve_type_expr(ty, item_index);
                }
            }
        }
    }

    fn resolve_function(&mut self, func: &FunctionDef, item_index: usize) {
        let func_scope = self.push_scope();

        // Add generic type parameters to the active set so uses of T/U etc.
        // in param types, return type, and the body are not reported as errors.
        let added_params: Vec<String> = func
            .type_params
            .iter()
            .filter(|p| self.active_type_params.insert(p.name.clone()))
            .map(|p| p.name.clone())
            .collect();

        // Bind parameters.
        for param in &func.params {
            self.declare_local(&param.name.name, DefKind::Param, param.name.span);
        }

        // Resolve the function body, tracking `use` placement.
        self.resolve_block_with_use_check(&func.body, item_index);

        for param in added_params {
            self.active_type_params.remove(&param);
        }

        self.pop_scope(func_scope);
    }

    // ------------------------------------------------------------------
    // Blocks & statements
    // ------------------------------------------------------------------

    fn resolve_block_with_use_check(&mut self, block: &Block, item_index: usize) {
        let mut seen_non_use = false;
        for stmt in &block.stmts {
            if let Stmt::Use(_) = stmt {
                if seen_non_use {
                    self.sink.emit(errors::use_not_at_top(stmt_span(stmt)));
                }
            } else {
                seen_non_use = true;
            }
            self.resolve_stmt(stmt, item_index);
        }
    }

    fn resolve_block(&mut self, block: &Block, item_index: usize) {
        let scope = self.push_scope();
        for stmt in &block.stmts {
            self.resolve_stmt(stmt, item_index);
        }
        self.pop_scope(scope);
    }

    fn resolve_stmt(&mut self, stmt: &Stmt, item_index: usize) {
        match stmt {
            Stmt::VarDecl(v) => self.resolve_var_decl(v, item_index),
            Stmt::Assign(a) => self.resolve_assign(a, item_index),
            Stmt::Return(r) => self.resolve_return(r, item_index),
            Stmt::ComptimeTypeBind(b) => self.resolve_comptime_type_bind(b, item_index),
            Stmt::If(i) => self.resolve_if(i, item_index),
            Stmt::For(f) => self.resolve_for(f, item_index),
            Stmt::While(w) => self.resolve_while(w, item_index),
            Stmt::Expr(e) => self.resolve_expr_stmt(e, item_index),
            Stmt::Use(u) => self.resolve_use(u),
            Stmt::Assert(a) => self.resolve_assert(a, item_index),
            Stmt::Trace(t) => self.resolve_trace(t, item_index),
            Stmt::Breakpoint(b) => self.resolve_breakpoint(b, item_index),
            Stmt::Match(m) => self.resolve_match(m, item_index),
            Stmt::Respond(r) => self.resolve_respond(r, item_index),
            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn resolve_respond(&mut self, r: &RespondStmt, item_index: usize) {
        self.resolve_expr(&r.value, item_index);
    }

    fn resolve_var_decl(&mut self, v: &VarDecl, item_index: usize) {
        // Resolve the initialiser first (the name is not yet in scope).
        self.resolve_expr(&v.value, item_index);
        self.resolve_type_expr(&v.ty, item_index);

        // Now declare the variable.
        self.declare_local(&v.name.name, DefKind::Variable, v.name.span);
    }

    fn resolve_comptime_type_bind(&mut self, b: &ComptimeTypeBindStmt, item_index: usize) {
        self.resolve_expr(&b.value, item_index);

        let scope = self.push_scope();
        self.declare_local(&b.name.name, DefKind::Type, b.name.span);
        for stmt in &b.body.stmts {
            self.resolve_stmt(stmt, item_index);
        }
        self.pop_scope(scope);
    }

    fn resolve_assign(&mut self, a: &AssignStmt, item_index: usize) {
        self.resolve_expr(&a.target, item_index);
        self.resolve_expr(&a.value, item_index);
    }

    fn resolve_return(&mut self, r: &ReturnStmt, item_index: usize) {
        if let Some(ref val) = r.value {
            self.resolve_expr(val, item_index);
        }
    }

    fn resolve_if(&mut self, i: &IfStmt, item_index: usize) {
        self.resolve_expr(&i.condition, item_index);
        self.resolve_block(&i.then_block, item_index);

        for (cond, block) in &i.else_ifs {
            self.resolve_expr(cond, item_index);
            self.resolve_block(block, item_index);
        }

        if let Some(ref else_block) = i.else_block {
            self.resolve_block(else_block, item_index);
        }
    }

    fn resolve_for(&mut self, f: &ForStmt, item_index: usize) {
        // Resolve the iterable in the outer scope.
        self.resolve_expr(&f.iterable, item_index);

        // Create a scope for the loop variable(s) + body.
        let scope = self.push_scope();
        self.declare_local(&f.variable.name, DefKind::Variable, f.variable.span);
        if let Some(ref val_var) = f.value_variable {
            self.declare_local(&val_var.name, DefKind::Variable, val_var.span);
        }
        for stmt in &f.body.stmts {
            self.resolve_stmt(stmt, item_index);
        }
        self.pop_scope(scope);
    }

    fn resolve_while(&mut self, w: &WhileStmt, item_index: usize) {
        self.resolve_expr(&w.condition, item_index);
        self.resolve_block(&w.body, item_index);
    }

    fn resolve_expr_stmt(&mut self, e: &ExprStmt, item_index: usize) {
        self.resolve_expr(&e.expr, item_index);
    }

    fn resolve_use(&mut self, u: &UseDecl) {
        // Determine the bound name: either the alias or the last segment of the path.
        let bound_name = if let Some(ref alias) = u.alias {
            alias.name.clone()
        } else {
            // For dotted paths like "net.http", bind the last segment.
            // The ast::UseDecl currently stores path as a single Ident, so
            // we split on '.' to get the last segment.
            last_segment(&u.path.name)
        };

        let def_id = self.declare_local_use(&bound_name, DefKind::Namespace, u.span);
        if let Some(id) = def_id {
            self.use_defs.insert(id);
        }
    }

    fn resolve_assert(&mut self, a: &AssertStmt, item_index: usize) {
        self.resolve_expr(&a.condition, item_index);
        if let Some(ref msg) = a.message {
            self.resolve_expr(msg, item_index);
        }
    }

    fn resolve_trace(&mut self, t: &TraceStmt, item_index: usize) {
        self.resolve_name(&t.name.name, t.name.span, item_index);
    }

    fn resolve_breakpoint(&mut self, b: &BreakpointStmt, item_index: usize) {
        if let Some(condition) = &b.condition {
            self.resolve_expr(condition, item_index);
        }
    }

    fn resolve_match(&mut self, m: &MatchStmt, item_index: usize) {
        self.resolve_expr(&m.expr, item_index);
        for arm in &m.arms {
            let scope = self.push_scope();
            // Declare bindings introduced by destructuring patterns.
            match &arm.pattern {
                Pattern::Variant(_, bindings) => {
                    for binding in bindings {
                        self.declare_local(&binding.name, DefKind::Variable, binding.span);
                    }
                }
                Pattern::Ident(_) | Pattern::Other(_) => {}
            }
            for stmt in &arm.body.stmts {
                self.resolve_stmt(stmt, item_index);
            }
            self.pop_scope(scope);
        }
    }

    // ------------------------------------------------------------------
    // Type expressions
    // ------------------------------------------------------------------

    fn resolve_type_expr(&mut self, ty: &TypeExpr, item_index: usize) {
        match ty {
            TypeExpr::Named(ident) => {
                // Builtin types are not resolved against the scope table.
                if !is_builtin_type(&ident.name) {
                    self.resolve_type_name(&ident.name, ident.span, item_index);
                }
            }
            TypeExpr::Generic(ident, args, _) => {
                if !is_builtin_type(&ident.name) {
                    self.resolve_type_name(&ident.name, ident.span, item_index);
                }
                for arg in args {
                    self.resolve_type_expr(arg, item_index);
                }
            }
            TypeExpr::View(inner, _) => {
                self.resolve_type_expr(inner, item_index);
            }
            TypeExpr::Function(param_types, return_type, _) => {
                for pt in param_types {
                    self.resolve_type_expr(pt, item_index);
                }
                self.resolve_type_expr(return_type, item_index);
            }
        }
    }

    fn resolve_type_name(&mut self, name: &str, span: Span, item_index: usize) {
        if let Some((namespace, _type_name)) = name.rsplit_once('.') {
            if self.scope_table.lookup(self.current_scope, name).is_some() {
                self.resolve_name(name, span, item_index);
            } else if !is_builtin_module(namespace) {
                self.resolve_name(namespace, span, item_index);
            }
        } else {
            self.resolve_name(name, span, item_index);
        }
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    fn resolve_expr(&mut self, expr: &Expr, item_index: usize) {
        match expr {
            Expr::Ident(ident) => {
                self.resolve_name(&ident.name, ident.span, item_index);
            }
            Expr::Binary(lhs, _, rhs, _) => {
                self.resolve_expr(lhs, item_index);
                self.resolve_expr(rhs, item_index);
            }
            Expr::Unary(_, operand, _) => {
                self.resolve_expr(operand, item_index);
            }
            Expr::FieldAccess(object, _, _) => {
                if let Some(path) = dotted_expr_name(expr) {
                    if self.resolve_namespace_prefix(&path, expr.span(), item_index) {
                        return;
                    }
                }

                // Only resolve the object; the field is resolved during type checking.
                self.resolve_expr(object, item_index);
            }
            Expr::Call(callee, args, _) => {
                self.resolve_expr(callee, item_index);
                for arg in args {
                    self.resolve_call_arg(arg, item_index);
                }
            }
            Expr::GenericCall(callee, type_args, args, _) => {
                self.resolve_expr(callee, item_index);
                for ty in type_args {
                    self.resolve_type_expr(ty, item_index);
                }
                for arg in args {
                    self.resolve_call_arg(arg, item_index);
                }
            }
            Expr::Paren(inner, _) => {
                self.resolve_expr(inner, item_index);
            }
            Expr::View(inner, _) => {
                self.resolve_expr(inner, item_index);
            }
            Expr::ListConstruct(elems, _) => {
                for elem in elems {
                    self.resolve_expr(elem, item_index);
                }
            }
            Expr::MapConstruct(entries, _) => {
                for (key, val) in entries {
                    self.resolve_expr(key, item_index);
                    self.resolve_expr(val, item_index);
                }
            }
            Expr::Handle(expr, error_binding, block, _) => {
                self.resolve_expr(expr, item_index);
                let scope = self.push_scope();
                if let Some(binding) = error_binding {
                    self.declare_local_without_unused_warning(
                        &binding.name,
                        DefKind::Variable,
                        binding.span,
                    );
                }
                for stmt in &block.stmts {
                    self.resolve_stmt(stmt, item_index);
                }
                self.pop_scope(scope);
            }
            Expr::Ok(inner, _) | Expr::Fail(inner, _) | Expr::Some(inner, _) => {
                self.resolve_expr(inner, item_index);
            }
            Expr::Default(inner, _) => {
                self.resolve_expr(inner, item_index);
            }
            Expr::EnumVariant(type_name, _, _) => {
                // Resolve the type name; variant name is checked during type checking.
                self.resolve_type_name(&type_name.name, type_name.span, item_index);
            }
            Expr::StringInterpolation(parts, _) => {
                for part in parts {
                    if let StringPart::Expr(expr) = part {
                        self.resolve_expr(expr, item_index);
                    }
                }
            }
            Expr::Declassify(inner, _) => {
                self.resolve_expr(inner, item_index);
            }
            Expr::Coarsen(inner, _) => {
                self.resolve_expr(inner, item_index);
            }
            Expr::Pipeline(initial, steps, _) => {
                self.resolve_expr(initial, item_index);
                for step in steps {
                    self.resolve_expr(&step.function, item_index);
                    for arg in &step.extra_args {
                        self.resolve_call_arg(arg, item_index);
                    }
                }
            }
            Expr::At(expr, _state_name, _) => {
                self.resolve_expr(expr, item_index);
            }
            Expr::Spawn(inner, _)
            | Expr::Send(inner, _)
            | Expr::Ask(inner, _)
            | Expr::Clone(inner, _)
            | Expr::Run(inner, _)
            | Expr::Join(inner, _)
            | Expr::Cancel(inner, _) => {
                self.resolve_expr(inner, item_index);
            }
            Expr::InlineFn(params, _return_type, body, _) => {
                let scope = self.push_scope();
                for param in params {
                    self.declare_local(&param.name.name, DefKind::Param, param.name.span);
                }
                self.resolve_block(body, item_index);
                self.pop_scope(scope);
            }
            // Literals and nothing — no names to resolve.
            Expr::IntLiteral(_, _)
            | Expr::FloatLiteral(_, _)
            | Expr::StringLiteral(_, _)
            | Expr::BoolLiteral(_, _)
            | Expr::Nothing(_)
            | Expr::None(_)
            | Expr::Error(_) => {}
        }
    }

    fn resolve_call_arg(&mut self, arg: &CallArg, item_index: usize) {
        // Named argument labels are not resolved — they match parameter names
        // during type checking, not name resolution.
        self.resolve_expr(&arg.value, item_index);
    }

    // ------------------------------------------------------------------
    // Name resolution core
    // ------------------------------------------------------------------

    /// Resolve a name at the given span. Records the resolution or emits an
    /// error. `item_index` is the index of the current top-level item being
    /// processed (used for forward-reference checking).
    fn resolve_name(&mut self, name: &str, span: Span, item_index: usize) {
        // Type parameters introduced by a generic struct are always in scope.
        if self.active_type_params.contains(name) {
            return;
        }

        // Builtin module names (e.g., `math`) are always in scope as callee prefixes.
        if is_builtin_module(name) {
            return;
        }

        if let Some(def_id) = self.lookup_local_non_root(name) {
            self.record_resolution(name, span, item_index, def_id);
            return;
        }

        if !name.contains('.') {
            if let Some(qualified) = self.current_qualified_name(name) {
                if let Some(def_id) = self.scope_table.lookup(self.current_scope, &qualified) {
                    self.record_resolution(&qualified, span, item_index, def_id);
                    return;
                }
            }
        }

        if let Some(def_id) = self.scope_table.lookup(self.current_scope, name) {
            self.record_resolution(name, span, item_index, def_id);
            return;
        }

        self.sink.emit(errors::undefined_name(name, span));
    }

    fn record_resolution(&mut self, name: &str, span: Span, item_index: usize, def_id: DefId) {
        // Check for forward reference to a top-level item.
        if let Some(&(top_def_id, decl_index)) = self.top_level_order.get(name) {
            if def_id == top_def_id && decl_index > item_index {
                let def_span = self.scope_table.def(def_id).span;
                self.sink
                    .emit(errors::forward_reference(name, span, def_span));
                return;
            }
        }
        self.resolutions.insert(span, def_id);
        self.used_defs.insert(def_id);
    }

    fn lookup_local_non_root(&self, name: &str) -> Option<DefId> {
        let mut scope = Some(self.current_scope);
        while let Some(scope_id) = scope {
            if scope_id.index() == 0 {
                return None;
            }
            if let Some(def_id) = self.scope_table.lookup_local(scope_id, name) {
                return Some(def_id);
            }
            scope = self.scope_table.scopes[scope_id.index() as usize].parent;
        }
        None
    }

    fn resolve_namespace_prefix(&mut self, path: &str, span: Span, item_index: usize) -> bool {
        for (index, _) in path.match_indices('.').rev() {
            let prefix = &path[..index];
            if is_builtin_module(prefix) {
                return true;
            }

            let Some(def_id) = self.scope_table.lookup(self.current_scope, prefix) else {
                continue;
            };
            if self.scope_table.def(def_id).kind != DefKind::Namespace {
                continue;
            }

            self.resolve_name(prefix, span, item_index);
            return true;
        }

        false
    }

    // ------------------------------------------------------------------
    // Scope management
    // ------------------------------------------------------------------

    fn push_scope(&mut self) -> ScopeId {
        let parent = self.current_scope;
        let new_scope = self.scope_table.new_scope(Some(parent));
        self.current_scope = new_scope;
        new_scope
    }

    fn pop_scope(&mut self, expected: ScopeId) {
        debug_assert_eq!(self.current_scope, expected);
        self.current_scope = self.scope_table.scopes[expected.index() as usize]
            .parent
            .expect("cannot pop root scope");
    }

    /// Declare a local binding (variable, parameter). Checks for shadowing.
    fn declare_local(&mut self, name: &str, kind: DefKind, span: Span) -> Option<DefId> {
        self.declare_local_impl(name, kind, span, true)
    }

    /// Declare a local binding that should not participate in unused-variable
    /// warnings. This is used for mandatory contextual bindings like
    /// `handle error:`, where the syntax requires the binding even if the body
    /// does not need to reference it.
    fn declare_local_without_unused_warning(
        &mut self,
        name: &str,
        kind: DefKind,
        span: Span,
    ) -> Option<DefId> {
        self.declare_local_impl(name, kind, span, false)
    }

    fn declare_local_impl(
        &mut self,
        name: &str,
        kind: DefKind,
        span: Span,
        track_unused: bool,
    ) -> Option<DefId> {
        // Check for shadowing in ancestor scopes.
        if let Some(prev_id) = self.scope_table.lookup_ancestor(self.current_scope, name) {
            let prev_span = self.scope_table.def(prev_id).span;
            self.sink
                .emit(errors::variable_shadowing(name, span, prev_span));
            return None;
        }
        // Check for duplicate in the current scope.
        if let Some(prev_id) = self.scope_table.lookup_local(self.current_scope, name) {
            let prev_span = self.scope_table.def(prev_id).span;
            self.sink
                .emit(errors::duplicate_definition(name, span, prev_span));
            return None;
        }
        let def_id = self.scope_table.new_def(name.to_string(), kind, span);
        self.scope_table
            .bind(self.current_scope, name.to_string(), def_id);
        if track_unused && (kind == DefKind::Variable || kind == DefKind::Param) {
            self.var_defs.insert(def_id);
        }
        Some(def_id)
    }

    /// Declare a use-import binding. Use declarations intentionally bring a
    /// namespace name into the local scope, so they do not trigger shadowing
    /// errors against ancestor scopes. Duplicate imports in the same scope are
    /// still rejected.
    fn declare_local_use(&mut self, name: &str, kind: DefKind, span: Span) -> Option<DefId> {
        // Check for duplicate in the current scope.
        if let Some(prev_id) = self.scope_table.lookup_local(self.current_scope, name) {
            let prev_span = self.scope_table.def(prev_id).span;
            self.sink
                .emit(errors::duplicate_definition(name, span, prev_span));
            return None;
        }
        let def_id = self.scope_table.new_def(name.to_string(), kind, span);
        self.scope_table
            .bind(self.current_scope, name.to_string(), def_id);
        Some(def_id)
    }

    // ------------------------------------------------------------------
    // Unused detection
    // ------------------------------------------------------------------

    fn check_unused(&mut self) {
        for &def_id in &self.var_defs {
            if !self.used_defs.contains(&def_id) {
                let info = self.scope_table.def(def_id);
                // Skip the loop variable `_` convention if adopted, but
                // for now we warn on everything.
                if info.name.starts_with('_') {
                    continue;
                }
                self.sink
                    .emit(errors::unused_variable(&info.name, info.span));
            }
        }
        for &def_id in &self.use_defs {
            if !self.used_defs.contains(&def_id) {
                let info = self.scope_table.def(def_id);
                self.sink.emit(errors::unused_import(&info.name, info.span));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn last_segment(path: &str) -> String {
    path.rsplit('.').next().unwrap_or(path).to_string()
}

fn dotted_expr_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(ident) => Some(ident.name.clone()),
        Expr::FieldAccess(base, field, _) => {
            let base_name = dotted_expr_name(base)?;
            Some(format!("{base_name}.{}", field.name))
        }
        _ => None,
    }
}

fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "int64"
            | "float64"
            | "string"
            | "bool"
            | "nothing"
            | "list"
            | "map"
            | "result"
            | "optional"
            | "secret"
            | "set"
            | "bytes"
            | "JsonValue"
            | "TypeConstruction"
            | "TypeInfo"
            | "TypeKind"
            | "TypePrimitive"
            | "TypeField"
            | "TypeBitfield"
            | "TypeBitfieldField"
            | "TypeBitfieldFieldShape"
            | "TypeVariant"
    )
}

/// Returns true for stdlib module names that can appear as callee prefixes
/// (e.g., `math.abs(x)`). These are not user-definable and should not
/// generate "undefined name" errors when used in expression position.
fn is_builtin_module(name: &str) -> bool {
    matches!(
        name,
        "math"
            | "json"
            | "random"
            | "encoding"
            | "crypto"
            | "time"
            | "os"
            | "log"
            | "format"
            | "validate"
            | "regex"
            | "csv"
            | "uuid"
            | "string"
            | "list"
            | "map"
            | "set"
            | "net"
            | "bitfield"
            | "type"
    )
}

fn stmt_span(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::VarDecl(v) => v.span,
        Stmt::Assign(a) => a.span,
        Stmt::Return(r) => r.span,
        Stmt::ComptimeTypeBind(b) => b.span,
        Stmt::If(i) => i.span,
        Stmt::For(f) => f.span,
        Stmt::While(w) => w.span,
        Stmt::Match(m) => m.span,
        Stmt::Expr(e) => e.span,
        Stmt::Use(u) => u.span,
        Stmt::Assert(a) => a.span,
        Stmt::Trace(t) => t.span,
        Stmt::Breakpoint(b) => b.span,
        Stmt::Break(s) | Stmt::Continue(s) => *s,
        Stmt::Respond(r) => r.span,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::{FileId, Span};
    use jett_diagnostics::Severity;
    use jett_parser::ast::*;

    /// Helper to create a span — all tests use file 0.
    fn sp(start: u32, end: u32) -> Span {
        Span::new(FileId::new(0), start, end)
    }

    fn ident(name: &str, start: u32) -> Ident {
        Ident {
            name: name.to_string(),
            span: sp(start, start + name.len() as u32),
        }
    }

    fn named_type(name: &str, start: u32) -> TypeExpr {
        TypeExpr::Named(ident(name, start))
    }

    fn int_literal(val: i64, start: u32) -> Expr {
        Expr::IntLiteral(val, sp(start, start + 2))
    }

    fn ident_expr(name: &str, start: u32) -> Expr {
        Expr::Ident(ident(name, start))
    }

    fn empty_block(start: u32) -> Block {
        Block {
            stmts: Vec::new(),
            span: sp(start, start + 1),
        }
    }

    // ---- Test: resolve a simple function call ----

    #[test]
    fn resolve_simple_function_call() {
        // function greet() returns nothing: ...
        // function main() returns nothing: greet()
        let module = Module {
            items: vec![
                Item::Function(FunctionDef {
                    name: ident("greet", 0),
                    type_params: vec![],
                    params: vec![],
                    return_type: Some(named_type("nothing", 20)),
                    body: empty_block(30),
                    span: sp(0, 35),
                }),
                Item::Function(FunctionDef {
                    name: ident("main", 40),
                    type_params: vec![],
                    params: vec![],
                    return_type: Some(named_type("nothing", 60)),
                    body: Block {
                        stmts: vec![Stmt::Expr(ExprStmt {
                            expr: Expr::Call(Box::new(ident_expr("greet", 80)), vec![], sp(80, 87)),
                            span: sp(80, 87),
                        })],
                        span: sp(70, 90),
                    },
                    span: sp(40, 90),
                }),
            ],
            span: sp(0, 90),
        };

        let result = resolve(&module);
        assert!(
            !result
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error),
            "expected no errors, got: {:#?}",
            result.diagnostics
        );
        // The call to `greet` at span (80,85) should resolve to the DefId for greet.
        let greet_span = sp(80, 85);
        assert!(
            result.resolutions.contains_key(&greet_span),
            "greet call should be resolved"
        );
    }

    // ---- Test: variable declaration and usage ----

    #[test]
    fn resolve_variable_declaration_and_usage() {
        // function main() returns nothing:
        //     int64 x = 42
        //     x
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("main", 0),
                type_params: vec![],
                params: vec![],
                return_type: Some(named_type("nothing", 20)),
                body: Block {
                    stmts: vec![
                        Stmt::VarDecl(VarDecl {
                            mutable: false,
                            ty: named_type("int64", 50),
                            name: ident("x", 56),
                            value: int_literal(42, 60),
                            span: sp(50, 64),
                        }),
                        Stmt::Expr(ExprStmt {
                            expr: ident_expr("x", 70),
                            span: sp(70, 71),
                        }),
                    ],
                    span: sp(40, 80),
                },
                span: sp(0, 80),
            })],
            span: sp(0, 80),
        };

        let result = resolve(&module);
        assert!(
            !result
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error),
            "expected no errors, got: {:#?}",
            result.diagnostics
        );
        // `x` at span (70,71) should resolve.
        let x_span = sp(70, 71);
        assert!(
            result.resolutions.contains_key(&x_span),
            "usage of x should be resolved"
        );
    }

    // ---- Test: no forward reference enforcement ----

    #[test]
    fn reject_forward_reference() {
        // function main() returns nothing: greet()
        // function greet() returns nothing: ...
        let module = Module {
            items: vec![
                Item::Function(FunctionDef {
                    name: ident("main", 0),
                    type_params: vec![],
                    params: vec![],
                    return_type: Some(named_type("nothing", 20)),
                    body: Block {
                        stmts: vec![Stmt::Expr(ExprStmt {
                            expr: Expr::Call(Box::new(ident_expr("greet", 50)), vec![], sp(50, 57)),
                            span: sp(50, 57),
                        })],
                        span: sp(30, 60),
                    },
                    span: sp(0, 60),
                }),
                Item::Function(FunctionDef {
                    name: ident("greet", 70),
                    type_params: vec![],
                    params: vec![],
                    return_type: Some(named_type("nothing", 90)),
                    body: empty_block(100),
                    span: sp(70, 105),
                }),
            ],
            span: sp(0, 105),
        };

        let result = resolve(&module);
        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 forward-reference error");
        assert_eq!(errors[0].code.code(), 205);
        assert!(errors[0].message.contains("greet"));
    }

    #[test]
    fn mutual_block_allows_forward_references_between_declared_functions() {
        let module = Module {
            items: vec![
                Item::Mutual(MutualBlock {
                    declarations: vec![
                        FunctionDecl {
                            name: ident("is_even", 0),
                            type_params: vec![],
                            params: vec![Param {
                                view: false,
                                mutable: false,
                                name: ident("n", 8),
                                ty: named_type("int64", 11),
                                span: sp(8, 16),
                            }],
                            return_type: Some(named_type("bool", 25)),
                            span: sp(0, 29),
                        },
                        FunctionDecl {
                            name: ident("is_odd", 35),
                            type_params: vec![],
                            params: vec![Param {
                                view: false,
                                mutable: false,
                                name: ident("n", 42),
                                ty: named_type("int64", 45),
                                span: sp(42, 50),
                            }],
                            return_type: Some(named_type("bool", 59)),
                            span: sp(35, 63),
                        },
                    ],
                    span: sp(0, 63),
                }),
                Item::Function(FunctionDef {
                    name: ident("is_even", 70),
                    type_params: vec![],
                    params: vec![Param {
                        view: false,
                        mutable: false,
                        name: ident("n", 78),
                        ty: named_type("int64", 81),
                        span: sp(78, 86),
                    }],
                    return_type: Some(named_type("bool", 95)),
                    body: Block {
                        stmts: vec![Stmt::Return(ReturnStmt {
                            value: Some(Expr::Call(
                                Box::new(ident_expr("is_odd", 110)),
                                vec![CallArg {
                                    name: None,
                                    value: ident_expr("n", 117),
                                    span: sp(117, 118),
                                }],
                                sp(110, 119),
                            )),
                            span: sp(103, 119),
                        })],
                        span: sp(100, 121),
                    },
                    span: sp(70, 121),
                }),
                Item::Function(FunctionDef {
                    name: ident("is_odd", 130),
                    type_params: vec![],
                    params: vec![Param {
                        view: false,
                        mutable: false,
                        name: ident("n", 137),
                        ty: named_type("int64", 140),
                        span: sp(137, 145),
                    }],
                    return_type: Some(named_type("bool", 154)),
                    body: Block {
                        stmts: vec![Stmt::Return(ReturnStmt {
                            value: Some(Expr::Call(
                                Box::new(ident_expr("is_even", 169)),
                                vec![CallArg {
                                    name: None,
                                    value: ident_expr("n", 177),
                                    span: sp(177, 178),
                                }],
                                sp(169, 179),
                            )),
                            span: sp(162, 179),
                        })],
                        span: sp(159, 181),
                    },
                    span: sp(130, 181),
                }),
            ],
            span: sp(0, 181),
        };

        let result = resolve(&module);
        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errors.is_empty(), "expected no errors, got: {errors:#?}");
        assert!(result.resolutions.contains_key(&sp(110, 116)));
        assert!(result.resolutions.contains_key(&sp(169, 176)));
    }

    // ---- Test: variable shadowing rejection ----

    #[test]
    fn reject_variable_shadowing() {
        // function main(x: int64) returns nothing:
        //     if true:
        //         int64 x = 10     # shadows parameter x
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("main", 0),
                type_params: vec![],
                params: vec![Param {
                    view: false,
                    mutable: false,
                    name: ident("x", 10),
                    ty: named_type("int64", 13),
                    span: sp(10, 19),
                }],
                return_type: Some(named_type("nothing", 30)),
                body: Block {
                    stmts: vec![Stmt::If(IfStmt {
                        condition: Expr::BoolLiteral(true, sp(50, 54)),
                        then_block: Block {
                            stmts: vec![Stmt::VarDecl(VarDecl {
                                mutable: false,
                                ty: named_type("int64", 60),
                                name: ident("x", 66),
                                value: int_literal(10, 70),
                                span: sp(60, 74),
                            })],
                            span: sp(55, 80),
                        },
                        else_ifs: vec![],
                        else_block: None,
                        span: sp(50, 80),
                    })],
                    span: sp(40, 85),
                },
                span: sp(0, 85),
            })],
            span: sp(0, 85),
        };

        let result = resolve(&module);
        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 shadowing error");
        assert_eq!(errors[0].code.code(), 201);
        assert!(errors[0].message.contains("shadows"));
    }

    // ---- Test: unused variable detection ----

    #[test]
    fn detect_unused_variable() {
        // function main() returns nothing:
        //     int64 x = 42
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("main", 0),
                type_params: vec![],
                params: vec![],
                return_type: Some(named_type("nothing", 20)),
                body: Block {
                    stmts: vec![Stmt::VarDecl(VarDecl {
                        mutable: false,
                        ty: named_type("int64", 50),
                        name: ident("x", 56),
                        value: int_literal(42, 60),
                        span: sp(50, 64),
                    })],
                    span: sp(40, 70),
                },
                span: sp(0, 70),
            })],
            span: sp(0, 70),
        };

        let result = resolve(&module);
        let warnings: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning && d.code.code() == 202)
            .collect();
        assert_eq!(warnings.len(), 1, "expected 1 unused-variable warning");
        assert!(warnings[0].message.contains("x"));
    }

    // ---- Test: nested scope resolution (function body → if block) ----

    #[test]
    fn nested_scope_resolution() {
        // function main() returns nothing:
        //     int64 x = 1
        //     if true:
        //         int64 y = x
        //     # y is not accessible here
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("main", 0),
                type_params: vec![],
                params: vec![],
                return_type: Some(named_type("nothing", 20)),
                body: Block {
                    stmts: vec![
                        Stmt::VarDecl(VarDecl {
                            mutable: false,
                            ty: named_type("int64", 50),
                            name: ident("x", 56),
                            value: int_literal(1, 60),
                            span: sp(50, 64),
                        }),
                        Stmt::If(IfStmt {
                            condition: Expr::BoolLiteral(true, sp(70, 74)),
                            then_block: Block {
                                stmts: vec![Stmt::VarDecl(VarDecl {
                                    mutable: false,
                                    ty: named_type("int64", 80),
                                    name: ident("y", 86),
                                    value: ident_expr("x", 90),
                                    span: sp(80, 92),
                                })],
                                span: sp(75, 95),
                            },
                            else_ifs: vec![],
                            else_block: None,
                            span: sp(70, 95),
                        }),
                    ],
                    span: sp(40, 100),
                },
                span: sp(0, 100),
            })],
            span: sp(0, 100),
        };

        let result = resolve(&module);
        // No errors — x is accessible from the if block.
        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errors.is_empty(), "expected no errors, got: {:#?}", errors);
        // x at span (90, 91) should resolve.
        let x_usage = sp(90, 91);
        assert!(
            result.resolutions.contains_key(&x_usage),
            "x should be resolved inside if block"
        );
    }

    // ---- Test: use declaration resolution ----

    #[test]
    fn use_declaration_resolution() {
        // namespace models
        // function main() returns nothing:
        //     use models
        //     models
        let module = Module {
            items: vec![
                Item::Namespace(NamespaceDecl {
                    name: ident("models", 0),
                    span: sp(0, 16),
                }),
                Item::Function(FunctionDef {
                    name: ident("main", 20),
                    type_params: vec![],
                    params: vec![],
                    return_type: Some(named_type("nothing", 40)),
                    body: Block {
                        stmts: vec![
                            Stmt::Use(UseDecl {
                                path: ident("models", 60),
                                alias: None,
                                span: sp(55, 70),
                            }),
                            Stmt::Expr(ExprStmt {
                                expr: ident_expr("models", 75),
                                span: sp(75, 81),
                            }),
                        ],
                        span: sp(50, 85),
                    },
                    span: sp(20, 85),
                }),
            ],
            span: sp(0, 85),
        };

        let result = resolve(&module);
        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errors.is_empty(), "expected no errors, got: {:#?}", errors);
        // The usage of `models` at (75, 81) should resolve to the use-binding.
        let models_span = sp(75, 81);
        assert!(
            result.resolutions.contains_key(&models_span),
            "models should be resolved"
        );
    }

    // ---- Test: use declaration not at top rejected ----

    #[test]
    fn reject_use_not_at_top() {
        // namespace models
        // function main() returns nothing:
        //     int64 x = 1
        //     use models
        let module = Module {
            items: vec![
                Item::Namespace(NamespaceDecl {
                    name: ident("models", 0),
                    span: sp(0, 16),
                }),
                Item::Function(FunctionDef {
                    name: ident("main", 20),
                    type_params: vec![],
                    params: vec![],
                    return_type: Some(named_type("nothing", 40)),
                    body: Block {
                        stmts: vec![
                            Stmt::VarDecl(VarDecl {
                                mutable: false,
                                ty: named_type("int64", 50),
                                name: ident("x", 56),
                                value: int_literal(1, 60),
                                span: sp(50, 64),
                            }),
                            Stmt::Use(UseDecl {
                                path: ident("models", 70),
                                alias: None,
                                span: sp(65, 80),
                            }),
                        ],
                        span: sp(45, 85),
                    },
                    span: sp(20, 85),
                }),
            ],
            span: sp(0, 85),
        };

        let result = resolve(&module);
        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error && d.code.code() == 206)
            .collect();
        assert_eq!(
            errors.len(),
            1,
            "expected 1 use-not-at-top error, got: {:#?}",
            errors
        );
    }

    // ---- Test: undefined name ----

    #[test]
    fn reject_undefined_name() {
        // function main() returns nothing:
        //     unknown_func()
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("main", 0),
                type_params: vec![],
                params: vec![],
                return_type: Some(named_type("nothing", 20)),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Call(
                            Box::new(ident_expr("unknown_func", 50)),
                            vec![],
                            sp(50, 65),
                        ),
                        span: sp(50, 65),
                    })],
                    span: sp(40, 70),
                },
                span: sp(0, 70),
            })],
            span: sp(0, 70),
        };

        let result = resolve(&module);
        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error && d.code.code() == 200)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 undefined-name error");
        assert!(errors[0].message.contains("unknown_func"));
    }

    // ---- Test: duplicate top-level definition ----

    #[test]
    fn reject_duplicate_top_level() {
        // function foo() returns nothing: ...
        // function foo() returns nothing: ...
        let module = Module {
            items: vec![
                Item::Function(FunctionDef {
                    name: ident("foo", 0),
                    type_params: vec![],
                    params: vec![],
                    return_type: Some(named_type("nothing", 20)),
                    body: empty_block(30),
                    span: sp(0, 35),
                }),
                Item::Function(FunctionDef {
                    name: ident("foo", 40),
                    type_params: vec![],
                    params: vec![],
                    return_type: Some(named_type("nothing", 60)),
                    body: empty_block(70),
                    span: sp(40, 75),
                }),
            ],
            span: sp(0, 75),
        };

        let result = resolve(&module);
        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error && d.code.code() == 204)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 duplicate-definition error");
        assert!(errors[0].message.contains("foo"));
    }

    // ---- Test: unused import detection ----

    #[test]
    fn detect_unused_import() {
        // namespace models
        // function main() returns nothing:
        //     use models
        let module = Module {
            items: vec![
                Item::Namespace(NamespaceDecl {
                    name: ident("models", 0),
                    span: sp(0, 16),
                }),
                Item::Function(FunctionDef {
                    name: ident("main", 20),
                    type_params: vec![],
                    params: vec![],
                    return_type: Some(named_type("nothing", 40)),
                    body: Block {
                        stmts: vec![Stmt::Use(UseDecl {
                            path: ident("models", 60),
                            alias: None,
                            span: sp(55, 70),
                        })],
                        span: sp(50, 75),
                    },
                    span: sp(20, 75),
                }),
            ],
            span: sp(0, 75),
        };

        let result = resolve(&module);
        let warnings: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning && d.code.code() == 203)
            .collect();
        assert_eq!(warnings.len(), 1, "expected 1 unused-import warning");
        assert!(warnings[0].message.contains("models"));
    }

    // ---- Test: underscore-prefixed variables are not flagged as unused ----

    #[test]
    fn underscore_prefix_suppresses_unused_warning() {
        // function main() returns nothing:
        //     int64 _x = 42
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("main", 0),
                type_params: vec![],
                params: vec![],
                return_type: Some(named_type("nothing", 20)),
                body: Block {
                    stmts: vec![Stmt::VarDecl(VarDecl {
                        mutable: false,
                        ty: named_type("int64", 50),
                        name: ident("_x", 56),
                        value: int_literal(42, 60),
                        span: sp(50, 64),
                    })],
                    span: sp(40, 70),
                },
                span: sp(0, 70),
            })],
            span: sp(0, 70),
        };

        let result = resolve(&module);
        let unused_warnings: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning && d.code.code() == 202)
            .collect();
        assert!(
            unused_warnings.is_empty(),
            "underscore-prefixed variables should not trigger unused warnings"
        );
    }

    // ---- Test: `handle error:` binding does not warn when unused ----

    #[test]
    fn handle_error_binding_does_not_trigger_unused_warning() {
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("main", 0),
                type_params: vec![],
                params: vec![],
                return_type: Some(named_type("nothing", 20)),
                body: Block {
                    stmts: vec![Stmt::VarDecl(VarDecl {
                        mutable: false,
                        ty: named_type("int64", 40),
                        name: ident("_parsed", 46),
                        value: Expr::Handle(
                            Box::new(Expr::Error(sp(55, 60))),
                            Some(ident("error", 68)),
                            Block {
                                stmts: vec![Stmt::Return(ReturnStmt {
                                    value: Some(Expr::Nothing(sp(80, 87))),
                                    span: sp(73, 87),
                                })],
                                span: sp(70, 87),
                            },
                            sp(55, 87),
                        ),
                        span: sp(40, 87),
                    })],
                    span: sp(35, 90),
                },
                span: sp(0, 90),
            })],
            span: sp(0, 90),
        };

        let result = resolve(&module);
        let unused_warnings: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning && d.code.code() == 202)
            .collect();
        assert!(
            unused_warnings.is_empty(),
            "handle error binding should not trigger unused warnings"
        );
    }

    #[test]
    fn refinement_type_constraint_resolves_value_binding() {
        let module = Module {
            items: vec![Item::TypeAlias(TypeAlias {
                name: ident("Port", 5),
                base_type: named_type("int64", 12),
                constraint: Some(Expr::Binary(
                    Box::new(Expr::Ident(ident("value", 24))),
                    BinOp::GtEq,
                    Box::new(Expr::IntLiteral(1, sp(33, 34))),
                    sp(24, 34),
                )),
                span: sp(0, 34),
            })],
            span: sp(0, 34),
        };

        let result = resolve(&module);
        assert!(
            !result
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error),
            "expected no errors, got: {:#?}",
            result.diagnostics
        );
    }
}
