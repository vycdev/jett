use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use jett_common::{FileId, JsonRawFacadeArgs, Span, json_raw_facade_spec};
use jett_diagnostics::{Diagnostic, DiagnosticSink};
use jett_parser::ast::{
    self, BinOp, Block, Expr, FunctionDef, Item, Module, Stmt, StringPart, TypeExpr, UnaryOp,
    VerifyBlock,
};
use jett_resolve::resolver::ResolveResult;
use jett_resolve::scope::{DefId, DefKind};
use jett_types::{
    ActorDef as TypeActorDef, ActorMessageDef, BitfieldDef as TypeBitfieldDef,
    BitfieldFieldDef as TypeBitfieldFieldDef, BitfieldFieldKind as TypeBitfieldFieldKind,
    BitfieldId, EnumDef as TypeEnumDef, FunctionSig, InterfaceDef as TypeInterfaceDef,
    ReflectionBitfieldFieldInfo, ReflectionBitfieldInfo, ReflectionFieldInfo, ReflectionMetadata,
    ReflectionTypeInfo, ReflectionVariantInfo, StructDef as TypeStructDef, StructId, Type, TypeId,
    TypeInterner, VariantDef,
};

use crate::capability;
use crate::errors;

/// The result of type checking.
#[derive(Debug)]
pub struct CheckResult {
    /// Diagnostics (errors and warnings) emitted during type checking.
    pub diagnostics: Vec<Diagnostic>,
    /// Map from expression spans to their inferred type.
    pub type_map: HashMap<Span, TypeId>,
    /// The type interner, containing all types encountered during checking.
    pub interner: TypeInterner,
    /// Checked reflection metadata snapshot for comptime reflection builtins.
    pub reflection_metadata: Arc<ReflectionMetadata>,
}

/// Type-check a resolved module.
pub fn check(module: &Module, resolve: &ResolveResult) -> CheckResult {
    let mut checker = TypeChecker::new(resolve);
    checker.check_module(module);

    let complexity_diagnostics = crate::complexity::check_complexity(module);

    // Run ownership analysis (linear type checking) after type checking.
    let ownership_diagnostics = crate::ownership::check_ownership(module, &checker.interner);

    let reflection_metadata = Arc::new(checker.build_reflection_metadata());

    let mut diagnostics = checker.sink.into_diagnostics();
    diagnostics.extend(complexity_diagnostics);
    diagnostics.extend(ownership_diagnostics);

    CheckResult {
        diagnostics,
        type_map: checker.type_map,
        interner: checker.interner,
        reflection_metadata,
    }
}

// ---------------------------------------------------------------------------
// Internal type checker
// ---------------------------------------------------------------------------

struct TypeChecker<'a> {
    interner: TypeInterner,
    resolve: &'a ResolveResult,
    sink: DiagnosticSink,
    /// DefId → TypeId for variables, parameters, and functions.
    type_env: HashMap<DefId, TypeId>,
    /// Declaration span → DefId for locally declared names.
    decl_defs: HashMap<Span, DefId>,
    /// User-defined type name → TypeId.
    named_types: HashMap<String, TypeId>,
    type_aliases: HashMap<String, ast::TypeAlias>,
    /// Compiler-owned source compatibility aliases that do not affect reflection identity.
    legacy_compat_aliases: Vec<(TypeId, TypeId)>,
    resolving_type_aliases: HashSet<String>,
    /// Expression span → TypeId (the output type map).
    type_map: HashMap<Span, TypeId>,
    /// The expected return type for the function currently being checked.
    current_return_type: Option<TypeId>,
    /// (interface, concrete type) -> implemented method signatures.
    interface_impls: HashMap<(TypeId, TypeId), HashMap<String, FunctionSig>>,
    /// concrete type -> all methods contributed by implement blocks.
    impl_methods_by_type: HashMap<TypeId, HashMap<String, FunctionSig>>,

    // -- Capability / purity tracking --
    /// Function name → is_pure.  Built during the first pass over the module.
    purity_map: HashMap<String, bool>,
    /// User-defined function name -> parameter and return types. Entries are
    /// registered under both the historical flat name and `namespace.name`.
    function_signatures: HashMap<String, (Vec<TypeId>, TypeId)>,
    /// Name of the function currently being type-checked (None outside functions).
    current_function_name: Option<String>,
    /// Whether the function currently being type-checked is pure.
    current_function_pure: bool,
    /// Whether we are inside a verify block.
    in_verify_block: bool,
    /// Whether we are inside a property block.
    in_property_block: bool,
    /// The name of the verify block currently being checked (for error messages).
    current_verify_name: Option<String>,
    /// Nesting depth inside a handle-block body. Used to validate `default`.
    handle_body_depth: usize,

    // -- Generic struct support --
    /// AST templates for user-defined generic structs (have type_params).
    generic_struct_templates: HashMap<String, ast::StructDef>,
    /// Cache of monomorphized generic struct instances: (name, concrete type args) → TypeId.
    monomorphized_structs: HashMap<(String, Vec<TypeId>), TypeId>,
    /// Checked reflection field snapshots keyed by the public type spelling.
    reflection_fields: HashMap<String, Vec<ReflectionFieldInfo>>,
    /// Checked reflection field snapshots keyed by the canonical owner TypeId.
    reflection_fields_by_id: HashMap<TypeId, (String, Vec<ReflectionFieldInfo>)>,
    /// Checked bitfield layout snapshots keyed by the public type spelling.
    reflection_bitfields: HashMap<String, ReflectionBitfieldInfo>,
    /// Checked bitfield layout snapshots keyed by the canonical owner TypeId.
    reflection_bitfields_by_id: HashMap<TypeId, (String, ReflectionBitfieldInfo)>,
    /// Checked enum variant snapshots keyed by the public type spelling.
    reflection_variants: HashMap<String, Vec<ReflectionVariantInfo>>,
    /// Checked enum variant snapshots keyed by the canonical owner TypeId.
    reflection_variants_by_id: HashMap<TypeId, (String, Vec<ReflectionVariantInfo>)>,
    /// Active type variable substitution during monomorphization (type_param_name → TypeId).
    type_var_subst: HashMap<String, TypeId>,
    /// Trusted field types currently available from direct `type.fields[T]()` loops.
    reflected_field_type_scopes: Vec<HashMap<String, Vec<TypeId>>>,
    /// Trusted TypeInfo types currently available from direct reflected `args` loops.
    reflected_type_info_scopes: Vec<HashMap<String, Vec<TypeId>>>,
    /// Trusted TypeVariant owners currently available from direct `type.variants[T]()` loops.
    reflected_variant_type_scopes: Vec<HashMap<String, TypeId>>,

    // -- Generic function support --
    /// AST templates for user-defined generic functions (have type_params).
    generic_function_templates: HashMap<String, FunctionDef>,

    // -- Actor support --
    /// The expected `responds T` type for the receive handler being checked.
    /// `None` when not inside a receive handler.
    current_respond_type: Option<TypeId>,
}

impl<'a> TypeChecker<'a> {
    fn new(resolve: &'a ResolveResult) -> Self {
        let decl_defs = resolve
            .scope_table
            .definitions
            .iter()
            .map(|def| (def.span, def.id))
            .collect();

        let mut checker = Self {
            interner: TypeInterner::new(),
            resolve,
            sink: DiagnosticSink::new(),
            type_env: HashMap::new(),
            decl_defs,
            named_types: HashMap::new(),
            type_aliases: HashMap::new(),
            legacy_compat_aliases: Vec::new(),
            resolving_type_aliases: HashSet::new(),
            type_map: HashMap::new(),
            current_return_type: None,
            interface_impls: HashMap::new(),
            impl_methods_by_type: HashMap::new(),
            purity_map: HashMap::new(),
            function_signatures: HashMap::new(),
            current_function_name: None,
            current_function_pure: false,
            in_verify_block: false,
            in_property_block: false,
            current_verify_name: None,
            handle_body_depth: 0,
            generic_struct_templates: HashMap::new(),
            monomorphized_structs: HashMap::new(),
            reflection_fields: HashMap::new(),
            reflection_fields_by_id: HashMap::new(),
            reflection_bitfields: HashMap::new(),
            reflection_bitfields_by_id: HashMap::new(),
            reflection_variants: HashMap::new(),
            reflection_variants_by_id: HashMap::new(),
            type_var_subst: HashMap::new(),
            reflected_field_type_scopes: Vec::new(),
            reflected_type_info_scopes: Vec::new(),
            reflected_variant_type_scopes: Vec::new(),
            generic_function_templates: HashMap::new(),
            current_respond_type: None,
        };
        checker.install_builtin_metadata_types();
        checker
    }

    fn install_builtin_metadata_types(&mut self) {
        let type_kind_eid = self.interner.add_enum(TypeEnumDef {
            name: "TypeKind".to_string(),
            variants: Self::metadata_unit_variants(&[
                "primitive_type",
                "alias_type",
                "refinement_type",
                "struct_type",
                "bitfield_type",
                "enum_type",
                "list_type",
                "set_type",
                "map_type",
                "optional_type",
                "result_type",
                "secret_type",
                "function_type",
                "unknown_type",
            ]),
        });
        let type_kind_ty = self.interner.intern(Type::Enum(type_kind_eid));
        self.named_types
            .insert("TypeKind".to_string(), type_kind_ty);

        let type_primitive_eid = self.interner.add_enum(TypeEnumDef {
            name: "TypePrimitive".to_string(),
            variants: Self::metadata_unit_variants(&[
                "int8_type",
                "int16_type",
                "int32_type",
                "int64_type",
                "uint8_type",
                "uint16_type",
                "uint32_type",
                "uint64_type",
                "float32_type",
                "float64_type",
                "string_type",
                "bool_type",
                "bytes_type",
                "nothing_type",
                "json_value_type",
                "type_construction_type",
                "unknown_type",
            ]),
        });
        let type_primitive_ty = self.interner.intern(Type::Enum(type_primitive_eid));
        self.named_types
            .insert("TypePrimitive".to_string(), type_primitive_ty);

        let bitfield_shape_eid = self.interner.add_enum(TypeEnumDef {
            name: "TypeBitfieldFieldShape".to_string(),
            variants: Self::metadata_unit_variants(&["bits_field", "payload_field"]),
        });
        let bitfield_shape_ty = self.interner.intern(Type::Enum(bitfield_shape_eid));
        self.named_types
            .insert("TypeBitfieldFieldShape".to_string(), bitfield_shape_ty);

        let type_info_sid = self.interner.add_struct(TypeStructDef {
            name: "TypeInfo".to_string(),
            fields: Vec::new(),
            methods: Vec::new(),
        });
        let type_info_ty = self.interner.intern(Type::Struct(type_info_sid));
        let type_info_args_ty = self.interner.intern(Type::List(type_info_ty));
        let optional_type_primitive_ty = self.interner.intern(Type::Optional(type_primitive_ty));
        self.interner.update_struct(
            type_info_sid,
            TypeStructDef {
                name: "TypeInfo".to_string(),
                fields: vec![
                    ("type_name".to_string(), TypeInterner::STRING),
                    ("kind".to_string(), TypeInterner::STRING),
                    ("kind_tag".to_string(), type_kind_ty),
                    ("primitive_tag".to_string(), optional_type_primitive_ty),
                    ("has_secret".to_string(), TypeInterner::BOOL),
                    ("args".to_string(), type_info_args_ty),
                ],
                methods: Vec::new(),
            },
        );
        self.named_types
            .insert("TypeInfo".to_string(), type_info_ty);

        let type_field_sid = self.interner.add_struct(TypeStructDef {
            name: "TypeField".to_string(),
            fields: vec![
                ("index".to_string(), TypeInterner::INT64),
                ("name".to_string(), TypeInterner::STRING),
                ("type_name".to_string(), TypeInterner::STRING),
                ("kind".to_string(), TypeInterner::STRING),
                ("kind_tag".to_string(), type_kind_ty),
                ("serialize_name".to_string(), TypeInterner::STRING),
                ("has_secret".to_string(), TypeInterner::BOOL),
                ("type_info".to_string(), type_info_ty),
            ],
            methods: Vec::new(),
        });
        let type_field_ty = self.interner.intern(Type::Struct(type_field_sid));
        self.named_types
            .insert("TypeField".to_string(), type_field_ty);

        let optional_type_info_ty = self.interner.intern(Type::Optional(type_info_ty));
        let type_bitfield_field_sid = self.interner.add_struct(TypeStructDef {
            name: "TypeBitfieldField".to_string(),
            fields: vec![
                ("index".to_string(), TypeInterner::INT64),
                ("name".to_string(), TypeInterner::STRING),
                ("shape".to_string(), TypeInterner::STRING),
                ("shape_tag".to_string(), bitfield_shape_ty),
                ("width".to_string(), TypeInterner::INT64),
                ("type_info".to_string(), type_info_ty),
                ("enum_type".to_string(), optional_type_info_ty),
            ],
            methods: Vec::new(),
        });
        let type_bitfield_field_ty = self.interner.intern(Type::Struct(type_bitfield_field_sid));
        self.named_types
            .insert("TypeBitfieldField".to_string(), type_bitfield_field_ty);

        let type_bitfield_fields_ty = self.interner.intern(Type::List(type_bitfield_field_ty));
        let type_bitfield_sid = self.interner.add_struct(TypeStructDef {
            name: "TypeBitfield".to_string(),
            fields: vec![
                ("network_order".to_string(), TypeInterner::BOOL),
                ("fields".to_string(), type_bitfield_fields_ty),
            ],
            methods: Vec::new(),
        });
        let type_bitfield_ty = self.interner.intern(Type::Struct(type_bitfield_sid));
        self.named_types
            .insert("TypeBitfield".to_string(), type_bitfield_ty);

        let type_variant_fields_ty = self.interner.intern(Type::List(type_field_ty));
        let type_variant_sid = self.interner.add_struct(TypeStructDef {
            name: "TypeVariant".to_string(),
            fields: vec![
                ("index".to_string(), TypeInterner::INT64),
                ("name".to_string(), TypeInterner::STRING),
                ("discriminant".to_string(), TypeInterner::INT64),
                ("has_secret".to_string(), TypeInterner::BOOL),
                ("fields".to_string(), type_variant_fields_ty),
            ],
            methods: Vec::new(),
        });
        let type_variant_ty = self.interner.intern(Type::Struct(type_variant_sid));
        self.named_types
            .insert("TypeVariant".to_string(), type_variant_ty);
    }

    fn metadata_unit_variants(names: &[&str]) -> Vec<VariantDef> {
        names
            .iter()
            .enumerate()
            .map(|(index, name)| VariantDef {
                name: (*name).to_string(),
                fields: Vec::new(),
                discriminant: index as i64,
            })
            .collect()
    }

    // ------------------------------------------------------------------
    // Utility: human-readable type name
    // ------------------------------------------------------------------

    fn type_name(&self, id: TypeId) -> String {
        match self.interner.resolve(id) {
            Type::Int8 => "int8".to_string(),
            Type::Int16 => "int16".to_string(),
            Type::Int32 => "int32".to_string(),
            Type::Int64 => "int64".to_string(),
            Type::Uint8 => "uint8".to_string(),
            Type::Uint16 => "uint16".to_string(),
            Type::Uint32 => "uint32".to_string(),
            Type::Uint64 => "uint64".to_string(),
            Type::Float32 => "float32".to_string(),
            Type::Float64 => "float64".to_string(),
            Type::String => "string".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Bytes => "bytes".to_string(),
            Type::Nothing => "nothing".to_string(),
            Type::JsonValue => "JsonValue".to_string(),
            Type::TypeConstruction => "TypeConstruction".to_string(),
            Type::List(inner) => format!("list[{}]", self.type_name(*inner)),
            Type::Map(k, v) => format!("map[{}, {}]", self.type_name(*k), self.type_name(*v)),
            Type::Set(inner) => format!("set[{}]", self.type_name(*inner)),
            Type::Optional(inner) => format!("optional[{}]", self.type_name(*inner)),
            Type::Result(ok, err) => {
                format!("result[{}, {}]", self.type_name(*ok), self.type_name(*err))
            }
            Type::Secret(inner) => format!("secret[{}]", self.type_name(*inner)),
            Type::Struct(sid) => self.interner.resolve_struct(*sid).name.clone(),
            Type::Bitfield(bid) => self.interner.resolve_bitfield(*bid).name.clone(),
            Type::Enum(eid) => self.interner.resolve_enum(*eid).name.clone(),
            Type::Interface(iid) => self.interner.resolve_interface(*iid).name.clone(),
            Type::Actor(aid) => self.interner.resolve_actor(*aid).name.clone(),
            Type::Function {
                params,
                return_type,
            } => {
                let params: Vec<String> = params.iter().map(|p| self.type_name(*p)).collect();
                format!(
                    "function({}) returns {}",
                    params.join(", "),
                    self.type_name(*return_type)
                )
            }
            Type::Refinement { name, .. } => name.clone(),
            Type::Error => "<error>".to_string(),
        }
    }

    fn check_builtin_type_arg_count(
        &mut self,
        name: &str,
        type_args: &[TypeExpr],
        expected: usize,
        span: Span,
    ) -> bool {
        if type_args.len() == expected {
            return true;
        }

        self.sink.emit(errors::unknown_type(
            &format!(
                "{name} (expected {expected} type argument(s), got {})",
                type_args.len()
            ),
            span,
        ));
        false
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

    fn update_current_namespace(
        item: &Item,
        current_file: &mut Option<FileId>,
        current_namespace: &mut Option<String>,
    ) {
        let item_file = Self::item_file(item);
        if current_file.is_some_and(|file| file != item_file) {
            *current_namespace = None;
        }
        *current_file = Some(item_file);

        if let Item::Namespace(ns) = item {
            *current_namespace = Some(ns.name.name.clone());
        }
    }

    fn namespace_qualified_name(namespace: Option<&str>, name: &str) -> Option<String> {
        namespace.map(|ns| format!("{ns}.{name}"))
    }

    fn canonical_name(namespace: Option<&str>, name: &str) -> String {
        Self::namespace_qualified_name(namespace, name).unwrap_or_else(|| name.to_string())
    }

    fn function_lookup_names(namespace: Option<&str>, name: &str) -> Vec<String> {
        vec![Self::canonical_name(namespace, name)]
    }

    fn type_lookup_names(namespace: Option<&str>, name: &str) -> Vec<String> {
        Self::function_lookup_names(namespace, name)
    }

    fn register_named_type(&mut self, namespace: Option<&str>, name: &str, ty: TypeId) {
        let canonical = Self::canonical_name(namespace, name);
        self.named_types.insert(canonical, ty);
    }

    fn register_generic_struct_template(
        &mut self,
        namespace: Option<&str>,
        name: &str,
        def: ast::StructDef,
    ) {
        let canonical = Self::canonical_name(namespace, name);
        self.generic_struct_templates.insert(canonical, def);
    }

    fn declaration_def_id(&self, span: Span) -> Option<DefId> {
        self.resolve
            .resolutions
            .get(&span)
            .copied()
            .or_else(|| self.decl_defs.get(&span).copied())
    }

    fn ident_def_kind(&self, ident: &ast::Ident) -> Option<DefKind> {
        let def_id = self
            .resolve
            .resolutions
            .get(&ident.span)
            .copied()
            .or_else(|| self.decl_defs.get(&ident.span).copied())?;
        Some(self.resolve.scope_table.def(def_id).kind)
    }

    fn resolved_symbol_name(&self, name: &str, span: Span) -> String {
        self.resolve
            .resolutions
            .get(&span)
            .copied()
            .or_else(|| self.decl_defs.get(&span).copied())
            .map(|def_id| self.resolve.scope_table.def(def_id).name.clone())
            .unwrap_or_else(|| name.to_string())
    }

    fn resolved_or_expanded_name(&self, name: &str, span: Span) -> String {
        let Some((prefix, suffix)) = name.split_once('.') else {
            return self.resolved_symbol_name(name, span);
        };

        let Some(def_id) = self
            .resolve
            .resolutions
            .get(&span)
            .copied()
            .or_else(|| self.decl_defs.get(&span).copied())
        else {
            return name.to_string();
        };

        let def = self.resolve.scope_table.def(def_id);
        if def.kind == DefKind::Namespace {
            if def.name == prefix {
                if let Some(target) = self.resolve.namespace_aliases.get(&def_id) {
                    return format!("{target}.{suffix}");
                }
            }
            return name.to_string();
        }

        def.name.clone()
    }

    fn expanded_dotted_expr_name(&self, expr: &Expr) -> Option<String> {
        let name = Self::extract_dotted_name(expr)?;
        Some(self.resolved_or_expanded_name(&name, expr.span()))
    }

    fn is_struct_type_name_expr(&self, expr: &Expr) -> bool {
        let Some(name) = self.expanded_dotted_expr_name(expr) else {
            return false;
        };
        self.named_types
            .get(&name)
            .is_some_and(|ty| matches!(self.interner.resolve(*ty), Type::Struct(_)))
    }

    fn is_bitfield_type_name_expr(&self, expr: &Expr) -> bool {
        let Some(name) = self.expanded_dotted_expr_name(expr) else {
            return false;
        };
        self.named_types
            .get(&name)
            .is_some_and(|ty| matches!(self.interner.resolve(*ty), Type::Bitfield(_)))
    }

    /// Returns true if the type is numeric (any integer or float type).
    fn is_numeric(&self, id: TypeId) -> bool {
        matches!(
            self.interner.resolve(id),
            Type::Int8
                | Type::Int16
                | Type::Int32
                | Type::Int64
                | Type::Uint8
                | Type::Uint16
                | Type::Uint32
                | Type::Uint64
                | Type::Float32
                | Type::Float64
        )
    }

    fn json_read_requires_view(&self, id: TypeId) -> bool {
        match self.interner.resolve(id) {
            Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::Float32
            | Type::Float64
            | Type::String
            | Type::Bool
            | Type::Nothing
            | Type::JsonValue
            | Type::Error => false,
            Type::Refinement { base, .. } => self.json_read_requires_view(*base),
            _ => true,
        }
    }

    fn is_secret_type(&self, id: TypeId) -> bool {
        matches!(self.interner.resolve(id), Type::Secret(_))
    }

    fn is_refinement_type(&self, id: TypeId) -> bool {
        matches!(self.interner.resolve(id), Type::Refinement { .. })
    }

    fn refinement_base_type(&self, id: TypeId) -> Option<TypeId> {
        match self.interner.resolve(id) {
            Type::Refinement { base, .. } => Some(*base),
            _ => None,
        }
    }

    fn fully_coarsened_type(&self, mut id: TypeId) -> TypeId {
        while let Some(base) = self.refinement_base_type(id) {
            id = base;
        }
        id
    }

    fn can_coarsen_to(&self, mut source: TypeId, target: TypeId) -> bool {
        while let Some(base) = self.refinement_base_type(source) {
            if base == target {
                return true;
            }
            source = base;
        }
        false
    }

    fn refinement_boundary_input_type(&self, refinement_ty: TypeId) -> TypeId {
        let base_ty = self.fully_coarsened_type(refinement_ty);
        self.secret_inner_type(base_ty).unwrap_or(base_ty)
    }

    fn can_refine_from(&self, source: TypeId, mut refinement_ty: TypeId) -> bool {
        while let Some(base) = self.refinement_base_type(refinement_ty) {
            if base == source || self.secret_inner_type(base) == Some(source) {
                return true;
            }
            refinement_ty = base;
        }
        false
    }

    fn satisfies_expected_type(&self, expected: TypeId, got: TypeId) -> bool {
        self.types_compatible(expected, got)
            || (self.is_refinement_type(expected) && self.can_refine_from(got, expected))
    }

    fn type_requires_handle_error(&self, expected: TypeId, got: TypeId) -> bool {
        if matches!(
            self.interner.resolve(expected),
            Type::Result(_, _) | Type::Optional(_)
        ) {
            return false;
        }
        match self.interner.resolve(got) {
            Type::Result(ok_ty, _) => self.satisfies_expected_type(expected, *ok_ty),
            _ => false,
        }
    }

    fn type_requires_bare_handle(&self, expected: TypeId, got: TypeId) -> bool {
        if matches!(
            self.interner.resolve(expected),
            Type::Result(_, _) | Type::Optional(_)
        ) {
            return false;
        }
        match self.interner.resolve(got) {
            Type::Optional(inner_ty) => self.satisfies_expected_type(expected, *inner_ty),
            _ => false,
        }
    }

    fn secret_inner_type(&self, id: TypeId) -> Option<TypeId> {
        match self.interner.resolve(id) {
            Type::Secret(inner) => Some(*inner),
            _ => None,
        }
    }

    fn strip_secret_type(&self, id: TypeId) -> (TypeId, bool) {
        match self.secret_inner_type(id) {
            Some(inner) => (inner, true),
            None => (id, false),
        }
    }

    fn maybe_wrap_secret(&mut self, ty: TypeId, tainted: bool) -> TypeId {
        if !tainted || ty == TypeInterner::ERROR || ty == TypeInterner::NOTHING {
            return ty;
        }
        if self.is_secret_type(ty) {
            return ty;
        }
        self.interner.intern(Type::Secret(ty))
    }

    fn is_secret_output_boundary(name: &str) -> bool {
        matches!(
            name,
            "Stdout.write"
                | "json.serialize"
                | "json.serialize_public"
                | "json.serialize_raw"
                | "Filesystem.write_file"
                | "log"
                | "http.respond"
        )
    }

    fn is_impure_builtin(name: &str) -> bool {
        matches!(
            name,
            "Stdout.write" | "Environment.args" | "Filesystem.read_file" | "Filesystem.write_file"
        )
    }

    fn is_secret_safe_builtin(name: &str) -> bool {
        matches!(name, "secret.redact" | "secret.compare")
    }

    fn is_secret_liftable_call(name: &str, callee_is_pure: bool) -> bool {
        callee_is_pure
            && !Self::is_secret_output_boundary(name)
            && !Self::is_secret_safe_builtin(name)
    }

    fn secret_argument_matches_param(&self, expected: TypeId, got: TypeId) -> (bool, bool) {
        if self.types_compatible(expected, got) {
            return (true, false);
        }

        let Some(inner) = self.secret_inner_type(got) else {
            return (false, false);
        };

        if self.types_compatible(expected, inner) {
            (true, true)
        } else {
            (false, false)
        }
    }

    fn type_contains_secret_data(&self, ty: TypeId) -> bool {
        let mut visited = HashSet::new();
        self.type_contains_secret_data_inner(ty, &mut visited)
    }

    fn type_contains_secret_data_inner(&self, ty: TypeId, visited: &mut HashSet<TypeId>) -> bool {
        if !visited.insert(ty) {
            return false;
        }

        match self.interner.resolve(ty) {
            Type::Secret(_) => true,
            Type::List(inner) | Type::Set(inner) | Type::Optional(inner) => {
                self.type_contains_secret_data_inner(*inner, visited)
            }
            Type::Map(key, value) | Type::Result(key, value) => {
                self.type_contains_secret_data_inner(*key, visited)
                    || self.type_contains_secret_data_inner(*value, visited)
            }
            Type::Struct(sid) => self
                .interner
                .resolve_struct(*sid)
                .fields
                .iter()
                .any(|(_, field_ty)| self.type_contains_secret_data_inner(*field_ty, visited)),
            Type::Bitfield(bid) => self
                .interner
                .resolve_bitfield(*bid)
                .fields
                .iter()
                .any(|field| self.type_contains_secret_data_inner(field.ty, visited)),
            Type::Enum(eid) => self
                .interner
                .resolve_enum(*eid)
                .variants
                .iter()
                .flat_map(|variant| variant.fields.iter())
                .any(|(_, field_ty)| self.type_contains_secret_data_inner(*field_ty, visited)),
            Type::Refinement { base, .. } => self.type_contains_secret_data_inner(*base, visited),
            _ => false,
        }
    }

    fn build_reflection_metadata(&mut self) -> ReflectionMetadata {
        let mut metadata = ReflectionMetadata::new();

        let type_ids = self.interner.type_ids().collect::<Vec<_>>();
        for type_id in type_ids {
            metadata.insert_type_info_for_id(type_id, self.reflection_type_info_for_type(type_id));
        }

        for (name, type_id) in self.named_types.clone() {
            if let Some(alias) = self.type_aliases.get(&name).cloned() {
                let info = self.reflection_type_info_for_alias(&name, &alias);
                if alias.constraint.is_some() {
                    metadata.insert_type_info_for_id(type_id, info);
                } else {
                    metadata.insert_type_info(info);
                }
            } else {
                let info = self.reflection_type_info_for_type_named(type_id, name);
                metadata.insert_type_info_for_id(type_id, info);
            }
        }

        for ((_name, type_args), type_id) in self.monomorphized_structs.clone() {
            let display_name = self.type_name(type_id);
            let arg_infos: Vec<ReflectionTypeInfo> = type_args
                .into_iter()
                .map(|arg| self.reflection_type_info_for_type(arg))
                .collect();
            let info = self.reflection_type_info_for_type_named_with_args(
                type_id,
                display_name.clone(),
                arg_infos.clone(),
            );
            metadata.insert_type_info_for_id(type_id, info);

            if let Some((_namespace, leaf_name)) = display_name.split_once('.') {
                metadata.insert_type_info(self.reflection_type_info_for_type_named_with_args(
                    type_id,
                    leaf_name.to_string(),
                    arg_infos,
                ));
            }
        }

        for (type_id, (type_name, fields)) in self.reflection_fields_by_id.clone() {
            metadata.insert_type_fields_for_id(type_id, type_name, fields);
        }

        for (type_id, (type_name, bitfield)) in self.reflection_bitfields_by_id.clone() {
            metadata.insert_bitfield_for_id(type_id, type_name, bitfield);
        }

        for (type_id, (type_name, variants)) in self.reflection_variants_by_id.clone() {
            metadata.insert_type_variants_for_id(type_id, type_name, variants);
        }

        for (type_name, fields) in self.reflection_fields.clone() {
            metadata.insert_type_fields(type_name, fields);
        }

        for (type_name, bitfield) in self.reflection_bitfields.clone() {
            metadata.insert_bitfield(type_name, bitfield);
        }

        for (type_name, variants) in self.reflection_variants.clone() {
            metadata.insert_type_variants(type_name, variants);
        }

        metadata
    }

    fn reflection_type_info_for_alias(
        &mut self,
        name: &str,
        alias: &ast::TypeAlias,
    ) -> ReflectionTypeInfo {
        let base_ty = self.resolve_type_expr(&alias.base_type);
        let args = if base_ty == TypeInterner::ERROR {
            Vec::new()
        } else {
            vec![self.reflection_type_info_for_type(base_ty)]
        };
        let kind = if alias.constraint.is_some() {
            "refinement"
        } else {
            "alias"
        };
        let has_secret = base_ty != TypeInterner::ERROR && self.type_contains_secret_data(base_ty);
        ReflectionTypeInfo::new(name, kind, None, has_secret, args)
    }

    fn reflection_type_info_for_type(&self, type_id: TypeId) -> ReflectionTypeInfo {
        self.reflection_type_info_for_type_named(type_id, self.type_name(type_id))
    }

    fn reflection_type_info_for_type_named(
        &self,
        type_id: TypeId,
        type_name: String,
    ) -> ReflectionTypeInfo {
        let args = self
            .type_info_arg_types_for_type(type_id)
            .into_iter()
            .map(|arg| self.reflection_type_info_for_type(arg))
            .collect();
        self.reflection_type_info_for_type_named_with_args(type_id, type_name, args)
    }

    fn reflection_type_info_for_type_named_with_args(
        &self,
        type_id: TypeId,
        type_name: String,
        args: Vec<ReflectionTypeInfo>,
    ) -> ReflectionTypeInfo {
        ReflectionTypeInfo::new(
            type_name,
            self.reflection_kind_for_type(type_id),
            self.reflection_primitive_tag_for_type(type_id)
                .map(str::to_string),
            self.type_contains_secret_data(type_id),
            args,
        )
    }

    fn reflection_kind_for_type(&self, type_id: TypeId) -> &'static str {
        match self.interner.resolve(type_id) {
            Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::Float32
            | Type::Float64
            | Type::String
            | Type::Bool
            | Type::Bytes
            | Type::Nothing
            | Type::JsonValue
            | Type::TypeConstruction => "primitive",
            Type::List(_) => "list",
            Type::Map(_, _) => "map",
            Type::Set(_) => "set",
            Type::Optional(_) => "optional",
            Type::Result(_, _) => "result",
            Type::Secret(_) => "secret",
            Type::Struct(_) => "struct",
            Type::Bitfield(_) => "bitfield",
            Type::Enum(_) => "enum",
            Type::Function { .. } => "function",
            Type::Refinement { .. } => "refinement",
            Type::Interface(_) | Type::Actor(_) | Type::Error => "unknown",
        }
    }

    fn reflection_primitive_tag_for_type(&self, type_id: TypeId) -> Option<&'static str> {
        match self.interner.resolve(type_id) {
            Type::Int8 => Some("int8_type"),
            Type::Int16 => Some("int16_type"),
            Type::Int32 => Some("int32_type"),
            Type::Int64 => Some("int64_type"),
            Type::Uint8 => Some("uint8_type"),
            Type::Uint16 => Some("uint16_type"),
            Type::Uint32 => Some("uint32_type"),
            Type::Uint64 => Some("uint64_type"),
            Type::Float32 => Some("float32_type"),
            Type::Float64 => Some("float64_type"),
            Type::String => Some("string_type"),
            Type::Bool => Some("bool_type"),
            Type::Bytes => Some("bytes_type"),
            Type::Nothing => Some("nothing_type"),
            Type::JsonValue => Some("json_value_type"),
            Type::TypeConstruction => Some("type_construction_type"),
            _ => None,
        }
    }

    fn reflection_fields_for_struct_def(
        &mut self,
        def: &ast::StructDef,
        namespace: Option<&str>,
        resolved_fields: &[(String, TypeId)],
    ) -> Vec<ReflectionFieldInfo> {
        def.fields
            .iter()
            .zip(resolved_fields.iter())
            .enumerate()
            .map(|(index, (field, (_, field_ty)))| {
                self.reflection_field_info_for_type_expr(
                    index,
                    &field.name.name,
                    field.serialize_name.as_deref().unwrap_or(&field.name.name),
                    &field.ty,
                    namespace,
                    *field_ty,
                )
            })
            .collect()
    }

    fn reflection_fields_for_resolved_struct(
        &self,
        def: &ast::StructDef,
        resolved_fields: &[(String, TypeId)],
    ) -> Vec<ReflectionFieldInfo> {
        def.fields
            .iter()
            .zip(resolved_fields.iter())
            .enumerate()
            .map(|(index, (field, (_, field_ty)))| {
                self.reflection_field_info_for_type_id(
                    index,
                    &field.name.name,
                    field.serialize_name.as_deref().unwrap_or(&field.name.name),
                    *field_ty,
                )
            })
            .collect()
    }

    fn reflection_fields_for_bitfield_def(
        &mut self,
        def: &ast::BitfieldDef,
        namespace: Option<&str>,
        resolved_fields: &[TypeBitfieldFieldDef],
    ) -> Vec<ReflectionFieldInfo> {
        def.fields
            .iter()
            .zip(resolved_fields.iter())
            .enumerate()
            .map(|(index, (field, resolved_field))| {
                let ty = match &field.kind {
                    ast::BitfieldFieldKind::Bits {
                        as_type: Some(ty), ..
                    } => ty.clone(),
                    ast::BitfieldFieldKind::Bits { as_type: None, .. } => {
                        TypeExpr::Named(ast::Ident {
                            name: "int64".to_string(),
                            span: field.span,
                        })
                    }
                    ast::BitfieldFieldKind::Payload(ty) => ty.clone(),
                };
                self.reflection_field_info_for_type_expr(
                    index,
                    &field.name.name,
                    &field.name.name,
                    &ty,
                    namespace,
                    resolved_field.ty,
                )
            })
            .collect()
    }

    fn reflection_bitfield_info_for_def(
        &mut self,
        def: &ast::BitfieldDef,
        namespace: Option<&str>,
        resolved_fields: &[TypeBitfieldFieldDef],
    ) -> ReflectionBitfieldInfo {
        let fields = def
            .fields
            .iter()
            .zip(resolved_fields.iter())
            .enumerate()
            .map(|(index, (field, resolved_field))| {
                let (shape, width, ty, enum_ty) = match &field.kind {
                    ast::BitfieldFieldKind::Bits { width, as_type } => {
                        let ty = as_type.clone().unwrap_or_else(|| {
                            TypeExpr::Named(ast::Ident {
                                name: "int64".to_string(),
                                span: field.span,
                            })
                        });
                        ("bits", i64::from(*width), ty, as_type.as_ref())
                    }
                    ast::BitfieldFieldKind::Payload(ty) => ("payload", 0, ty.clone(), None),
                };
                let type_info =
                    self.reflection_type_info_for_type_expr(&ty, namespace, resolved_field.ty);
                let enum_type = enum_ty.map(|ty| {
                    let enum_ty = self.resolve_type_expr(ty);
                    self.reflection_type_info_for_type_expr(ty, namespace, enum_ty)
                });
                ReflectionBitfieldFieldInfo::new(
                    index,
                    &field.name.name,
                    shape,
                    width,
                    type_info,
                    enum_type,
                )
            })
            .collect();
        ReflectionBitfieldInfo::new(def.network_order, fields)
    }

    fn reflection_variants_for_enum_def(
        &mut self,
        def: &ast::EnumDef,
        namespace: Option<&str>,
        resolved_variants: &[VariantDef],
    ) -> Vec<ReflectionVariantInfo> {
        def.variants
            .iter()
            .zip(resolved_variants.iter())
            .enumerate()
            .map(|(variant_index, (variant, resolved_variant))| {
                let fields = variant
                    .fields
                    .iter()
                    .zip(resolved_variant.fields.iter())
                    .enumerate()
                    .map(|(field_index, (field, (_, field_ty)))| {
                        self.reflection_field_info_for_type_expr(
                            field_index,
                            &field.name.name,
                            field.serialize_name.as_deref().unwrap_or(&field.name.name),
                            &field.ty,
                            namespace,
                            *field_ty,
                        )
                    })
                    .collect::<Vec<_>>();
                let has_secret = fields.iter().any(|field| field.has_secret);
                ReflectionVariantInfo::new(
                    variant_index,
                    &variant.name.name,
                    resolved_variant.discriminant,
                    has_secret,
                    fields,
                )
            })
            .collect()
    }

    fn reflection_field_info_for_type_expr(
        &mut self,
        index: usize,
        name: &str,
        serialize_name: &str,
        ty: &TypeExpr,
        namespace: Option<&str>,
        resolved_ty: TypeId,
    ) -> ReflectionFieldInfo {
        let type_info = self.reflection_type_info_for_type_expr(ty, namespace, resolved_ty);
        Self::reflection_field_info_from_type_info(index, name, serialize_name, type_info)
    }

    fn reflection_field_info_for_type_id(
        &self,
        index: usize,
        name: &str,
        serialize_name: &str,
        ty: TypeId,
    ) -> ReflectionFieldInfo {
        let type_info = self.reflection_type_info_for_type(ty);
        Self::reflection_field_info_from_type_info(index, name, serialize_name, type_info)
    }

    fn reflection_field_info_from_type_info(
        index: usize,
        name: &str,
        serialize_name: &str,
        type_info: ReflectionTypeInfo,
    ) -> ReflectionFieldInfo {
        ReflectionFieldInfo::new(
            index,
            name,
            type_info.type_name.clone(),
            type_info.kind.clone(),
            serialize_name,
            type_info.has_secret,
            type_info,
        )
    }

    fn reflection_type_info_for_type_expr(
        &mut self,
        ty: &TypeExpr,
        namespace: Option<&str>,
        resolved_ty: TypeId,
    ) -> ReflectionTypeInfo {
        if let TypeExpr::View(inner, _) = ty {
            return self.reflection_type_info_for_type_expr(inner, namespace, resolved_ty);
        }

        let type_name = self.reflection_type_expr_display(ty, namespace);
        let args = self.reflection_type_info_args_for_type_expr(ty, namespace, resolved_ty);
        ReflectionTypeInfo::new(
            type_name,
            self.reflection_kind_for_type_expr(ty, namespace, resolved_ty),
            self.reflection_primitive_tag_for_type_expr(ty, namespace)
                .map(str::to_string),
            resolved_ty != TypeInterner::ERROR && self.type_contains_secret_data(resolved_ty),
            args,
        )
    }

    fn reflection_type_info_args_for_type_expr(
        &mut self,
        ty: &TypeExpr,
        namespace: Option<&str>,
        resolved_ty: TypeId,
    ) -> Vec<ReflectionTypeInfo> {
        match ty {
            TypeExpr::View(inner, _) => {
                self.reflection_type_info_args_for_type_expr(inner, namespace, resolved_ty)
            }
            TypeExpr::Named(ident) => {
                let display_name = self.reflection_type_name_in_namespace(ident, namespace);
                if let Some(alias) = self.type_aliases.get(&display_name).cloned() {
                    let alias_namespace = display_name
                        .rsplit_once('.')
                        .map(|(namespace, _)| namespace)
                        .or(namespace);
                    let base_ty = self.resolve_type_expr(&alias.base_type);
                    if base_ty == TypeInterner::ERROR {
                        Vec::new()
                    } else {
                        vec![self.reflection_type_info_for_type_expr(
                            &alias.base_type,
                            alias_namespace,
                            base_ty,
                        )]
                    }
                } else {
                    self.type_info_arg_types_for_type(resolved_ty)
                        .into_iter()
                        .map(|arg| self.reflection_type_info_for_type(arg))
                        .collect()
                }
            }
            TypeExpr::Generic(_, args, _) => args
                .iter()
                .map(|arg| {
                    let arg_ty = self.resolve_type_expr(arg);
                    self.reflection_type_info_for_type_expr(arg, namespace, arg_ty)
                })
                .collect(),
            TypeExpr::Function(params, return_type, _) => params
                .iter()
                .chain(std::iter::once(return_type.as_ref()))
                .map(|arg| {
                    let arg_ty = self.resolve_type_expr(arg);
                    self.reflection_type_info_for_type_expr(arg, namespace, arg_ty)
                })
                .collect(),
        }
    }

    fn reflection_kind_for_type_expr(
        &self,
        ty: &TypeExpr,
        namespace: Option<&str>,
        resolved_ty: TypeId,
    ) -> &'static str {
        match ty {
            TypeExpr::View(inner, _) => {
                self.reflection_kind_for_type_expr(inner, namespace, resolved_ty)
            }
            TypeExpr::Named(ident) => {
                let name = self.reflection_type_name_in_namespace(ident, namespace);
                if Self::reflection_primitive_tag_for_name(&name).is_some() {
                    "primitive"
                } else if let Some(alias) = self.type_aliases.get(&name) {
                    if alias.constraint.is_some() {
                        "refinement"
                    } else {
                        "alias"
                    }
                } else {
                    self.reflection_kind_for_type(resolved_ty)
                }
            }
            TypeExpr::Generic(ident, _, _) => {
                let name = self.reflection_type_name_in_namespace(ident, namespace);
                match name.as_str() {
                    "list" => "list",
                    "map" => "map",
                    "set" => "set",
                    "optional" => "optional",
                    "result" => "result",
                    "secret" => "secret",
                    _ if self.generic_struct_templates.contains_key(&name) => "struct",
                    _ => self.reflection_kind_for_type(resolved_ty),
                }
            }
            TypeExpr::Function(_, _, _) => "function",
        }
    }

    fn reflection_primitive_tag_for_type_expr(
        &self,
        ty: &TypeExpr,
        namespace: Option<&str>,
    ) -> Option<&'static str> {
        match ty {
            TypeExpr::Named(ident) => {
                let name = self.reflection_type_name_in_namespace(ident, namespace);
                Self::reflection_primitive_tag_for_name(&name)
            }
            TypeExpr::View(inner, _) => {
                self.reflection_primitive_tag_for_type_expr(inner, namespace)
            }
            TypeExpr::Generic(_, _, _) | TypeExpr::Function(_, _, _) => None,
        }
    }

    fn reflection_primitive_tag_for_name(name: &str) -> Option<&'static str> {
        match name {
            "int8" => Some("int8_type"),
            "int16" => Some("int16_type"),
            "int32" => Some("int32_type"),
            "int64" => Some("int64_type"),
            "uint8" => Some("uint8_type"),
            "uint16" => Some("uint16_type"),
            "uint32" => Some("uint32_type"),
            "uint64" => Some("uint64_type"),
            "float32" => Some("float32_type"),
            "float64" => Some("float64_type"),
            "string" => Some("string_type"),
            "bool" => Some("bool_type"),
            "bytes" => Some("bytes_type"),
            "nothing" => Some("nothing_type"),
            "JsonValue" => Some("json_value_type"),
            "TypeConstruction" => Some("type_construction_type"),
            _ => None,
        }
    }

    fn reflection_type_expr_display(&self, ty: &TypeExpr, namespace: Option<&str>) -> String {
        match ty {
            TypeExpr::Named(ident) => self.reflection_type_name_in_namespace(ident, namespace),
            TypeExpr::Generic(ident, args, _) => {
                let args = args
                    .iter()
                    .map(|arg| self.reflection_type_expr_display(arg, namespace))
                    .collect::<Vec<_>>();
                format!(
                    "{}[{}]",
                    self.reflection_type_name_in_namespace(ident, namespace),
                    args.join(", ")
                )
            }
            TypeExpr::View(inner, _) => {
                format!(
                    "view {}",
                    self.reflection_type_expr_display(inner, namespace)
                )
            }
            TypeExpr::Function(params, return_type, _) => {
                let params = params
                    .iter()
                    .map(|param| self.reflection_type_expr_display(param, namespace))
                    .collect::<Vec<_>>();
                format!(
                    "function({}) returns {}",
                    params.join(", "),
                    self.reflection_type_expr_display(return_type, namespace)
                )
            }
        }
    }

    fn reflection_type_name_in_namespace(
        &self,
        ident: &ast::Ident,
        namespace: Option<&str>,
    ) -> String {
        if ident.name.contains('.') {
            return ident.name.clone();
        }
        if let Some(namespace) = namespace {
            let qualified = format!("{namespace}.{}", ident.name);
            if self.reflection_type_name_is_registered(&qualified) {
                return qualified;
            }
        }
        ident.name.clone()
    }

    fn reflection_type_name_is_registered(&self, name: &str) -> bool {
        self.named_types.contains_key(name)
            || self.type_aliases.contains_key(name)
            || self.generic_struct_templates.contains_key(name)
    }

    fn secret_field_names(&self, ty: TypeId) -> Vec<String> {
        match self.interner.resolve(ty) {
            Type::Struct(sid) => self
                .interner
                .resolve_struct(*sid)
                .fields
                .iter()
                .filter(|(_, field_ty)| self.type_contains_secret_data(*field_ty))
                .map(|(name, _)| name.clone())
                .collect(),
            Type::Bitfield(bid) => self
                .interner
                .resolve_bitfield(*bid)
                .fields
                .iter()
                .filter(|field| self.type_contains_secret_data(field.ty))
                .map(|field| field.name.clone())
                .collect(),
            _ => Vec::new(),
        }
    }

    fn json_non_string_map_key_types(&self, ty: TypeId) -> Vec<String> {
        let mut visited = HashSet::new();
        let mut keys = Vec::new();
        self.collect_json_non_string_map_key_types(ty, &mut visited, &mut keys);
        keys
    }

    fn collect_json_non_string_map_key_types(
        &self,
        ty: TypeId,
        visited: &mut HashSet<TypeId>,
        keys: &mut Vec<String>,
    ) {
        if !visited.insert(ty) {
            return;
        }

        match self.interner.resolve(ty) {
            Type::List(inner) | Type::Set(inner) | Type::Optional(inner) | Type::Secret(inner) => {
                self.collect_json_non_string_map_key_types(*inner, visited, keys);
            }
            Type::Map(key, value) => {
                if self.fully_coarsened_type(*key) != TypeInterner::STRING {
                    keys.push(self.type_name(*key));
                }
                self.collect_json_non_string_map_key_types(*key, visited, keys);
                self.collect_json_non_string_map_key_types(*value, visited, keys);
            }
            Type::Result(ok, err) => {
                self.collect_json_non_string_map_key_types(*ok, visited, keys);
                self.collect_json_non_string_map_key_types(*err, visited, keys);
            }
            Type::Struct(sid) => {
                for (_, field_ty) in &self.interner.resolve_struct(*sid).fields {
                    self.collect_json_non_string_map_key_types(*field_ty, visited, keys);
                }
            }
            Type::Bitfield(bid) => {
                for field in &self.interner.resolve_bitfield(*bid).fields {
                    self.collect_json_non_string_map_key_types(field.ty, visited, keys);
                }
            }
            Type::Enum(eid) => {
                for (_, field_ty) in self
                    .interner
                    .resolve_enum(*eid)
                    .variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter())
                {
                    self.collect_json_non_string_map_key_types(*field_ty, visited, keys);
                }
            }
            Type::Refinement { base, .. } => {
                self.collect_json_non_string_map_key_types(*base, visited, keys);
            }
            _ => {}
        }
    }

    fn check_json_public_call_policy(
        &mut self,
        callee_name: Option<&str>,
        checked_arg_types: &[TypeId],
        args: &[ast::CallArg],
        return_type: TypeId,
    ) {
        let Some(callee_name) = callee_name else {
            return;
        };

        match callee_name {
            "json.serialize" => {
                let Some((&value_ty, arg)) = checked_arg_types.first().zip(args.first()) else {
                    return;
                };
                if self.type_contains_secret_data(value_ty) {
                    self.sink.emit(errors::type_contains_secret_data(
                        "json.serialize",
                        &self.type_name(value_ty),
                        &self.secret_field_names(value_ty),
                        arg.value.span(),
                    ));
                }
                self.check_json_public_serialize_policy(callee_name, value_ty, arg);
            }
            "json.serialize_public" => {
                let Some((&value_ty, arg)) = checked_arg_types.first().zip(args.first()) else {
                    return;
                };
                self.check_json_public_serialize_policy(callee_name, value_ty, arg);
            }
            "json.parse" => {
                let Some(arg) = args.first() else {
                    return;
                };
                if let Type::Result(parsed_ty, _) = self.interner.resolve(return_type) {
                    for key_type in self.json_non_string_map_key_types(*parsed_ty) {
                        self.sink.emit(errors::json_map_key_must_be_string(
                            &key_type,
                            arg.value.span(),
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    fn check_json_public_serialize_policy(
        &mut self,
        function_name: &str,
        value_ty: TypeId,
        arg: &ast::CallArg,
    ) {
        if !matches!(&arg.value, Expr::View(_, _)) && self.json_read_requires_view(value_ty) {
            self.sink.emit(errors::json_serialize_requires_view(
                function_name,
                &self.type_name(value_ty),
                arg.value.span(),
            ));
        }

        for key_type in self.json_non_string_map_key_types(value_ty) {
            self.sink.emit(errors::json_map_key_must_be_string(
                &key_type,
                arg.value.span(),
            ));
        }
    }

    fn types_compatible(&self, expected: TypeId, got: TypeId) -> bool {
        if expected == got || expected == TypeInterner::ERROR || got == TypeInterner::ERROR {
            return true;
        }
        if self.legacy_compat_alias_compatible(expected, got) {
            return true;
        }

        match (self.interner.resolve(expected), self.interner.resolve(got)) {
            (Type::Interface(_), Type::Interface(_)) => expected == got,
            (Type::Interface(_), _) => self.interface_impls.contains_key(&(expected, got)),
            (Type::Secret(expected_inner), Type::Secret(got_inner)) => {
                self.types_compatible(*expected_inner, *got_inner)
            }
            (Type::Secret(expected_inner), _) => self.types_compatible(*expected_inner, got),
            (Type::List(expected_inner), Type::List(got_inner))
            | (Type::Optional(expected_inner), Type::Optional(got_inner)) => {
                self.types_compatible(*expected_inner, *got_inner)
            }
            (Type::Set(expected_inner), Type::Set(got_inner)) => {
                self.types_compatible(*expected_inner, *got_inner)
            }
            (Type::Map(expected_key, expected_val), Type::Map(got_key, got_val))
            | (Type::Result(expected_key, expected_val), Type::Result(got_key, got_val)) => {
                self.types_compatible(*expected_key, *got_key)
                    && self.types_compatible(*expected_val, *got_val)
            }
            _ => false,
        }
    }

    fn legacy_compat_alias_compatible(&self, expected: TypeId, got: TypeId) -> bool {
        self.legacy_compat_aliases
            .iter()
            .any(|(legacy, canonical)| {
                (*legacy == expected && *canonical == got)
                    || (*legacy == got && *canonical == expected)
            })
    }

    fn seed_legacy_compat_aliases(&mut self) {
        self.legacy_compat_aliases.clear();
        if let Some(&json_tree_ty) = self.named_types.get("json.JsonTree") {
            self.legacy_compat_aliases
                .push((TypeInterner::JSON_VALUE, json_tree_ty));
        }
    }

    fn json_tree_type_or_legacy_json_value(&self) -> TypeId {
        self.named_types
            .get("json.JsonTree")
            .copied()
            .unwrap_or(TypeInterner::JSON_VALUE)
    }

    fn json_raw_facade_builtin_signature(
        &mut self,
        name: &str,
        type_args: &[TypeExpr],
        span: Span,
    ) -> Option<(Vec<TypeId>, TypeId)> {
        let spec = json_raw_facade_spec(name)?;
        let valid_type_args = self.check_builtin_type_arg_count(name, type_args, 0, span);
        let json_tree_ty = self.json_tree_type_or_legacy_json_value();

        let params = match spec.args {
            JsonRawFacadeArgs::RawString => vec![TypeInterner::STRING],
            JsonRawFacadeArgs::Tree => vec![json_tree_ty],
            JsonRawFacadeArgs::TreeAndString => vec![json_tree_ty, TypeInterner::STRING],
            JsonRawFacadeArgs::TreeAndInt64 => vec![json_tree_ty, TypeInterner::INT64],
        };

        let return_ty = match name {
            "json.parse_raw" => {
                if valid_type_args {
                    self.interner
                        .intern(Type::Result(json_tree_ty, TypeInterner::STRING))
                } else {
                    TypeInterner::ERROR
                }
            }
            "json.serialize_raw" | "json.kind" => TypeInterner::STRING,
            "json.is_null" | "json.is_bool" | "json.is_number" | "json.is_string"
            | "json.is_array" | "json.is_object" => TypeInterner::BOOL,
            "json.field" | "json.index" => self.interner.intern(Type::Optional(json_tree_ty)),
            "json.array_length" => self
                .interner
                .intern(Type::Result(TypeInterner::INT64, TypeInterner::STRING)),
            "json.object_keys" => {
                let list_string = self.interner.intern(Type::List(TypeInterner::STRING));
                self.interner
                    .intern(Type::Result(list_string, TypeInterner::STRING))
            }
            "json.as_string" => self
                .interner
                .intern(Type::Result(TypeInterner::STRING, TypeInterner::STRING)),
            "json.as_int64" => self
                .interner
                .intern(Type::Result(TypeInterner::INT64, TypeInterner::STRING)),
            "json.as_float64" => self
                .interner
                .intern(Type::Result(TypeInterner::FLOAT64, TypeInterner::STRING)),
            "json.as_bool" => self
                .interner
                .intern(Type::Result(TypeInterner::BOOL, TypeInterner::STRING)),
            _ => return None,
        };

        Some((params, return_ty))
    }

    /// Extract (key_type, value_type) from map builtin type args.
    /// Uses ERROR as a wildcard when type args are absent (matches any map).
    fn map_type_args(&mut self, type_args: &[TypeExpr]) -> (TypeId, TypeId) {
        match type_args.len() {
            2 => (
                self.resolve_type_expr(&type_args[0]),
                self.resolve_type_expr(&type_args[1]),
            ),
            _ => (TypeInterner::ERROR, TypeInterner::ERROR),
        }
    }

    fn extract_dotted_name(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(ident) => Some(ident.name.clone()),
            Expr::FieldAccess(inner, field, _) => {
                let prefix = Self::extract_dotted_name(inner)?;
                Some(format!("{prefix}.{}", field.name))
            }
            _ => None,
        }
    }

    fn resolved_expr_name(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(ident) => Some(self.resolved_symbol_name(&ident.name, ident.span)),
            Expr::FieldAccess(_, _, _) => self.expanded_dotted_expr_name(expr),
            _ => None,
        }
    }

    fn builtin_signature(
        &mut self,
        callee: &Expr,
        type_args: &[TypeExpr],
        span: Span,
    ) -> Option<(Vec<TypeId>, TypeId)> {
        let name = self.resolved_expr_name(callee)?;
        if let Some((type_name, method_name)) = name.rsplit_once('.') {
            if let Some(&type_id) = self.named_types.get(type_name) {
                if matches!(self.interner.resolve(type_id), Type::Bitfield(_)) {
                    match method_name {
                        "to_bytes" => {
                            if !type_args.is_empty() {
                                self.sink.emit(errors::unknown_type(
                                    &format!(
                                        "{name} (expected 0 type arguments, got {})",
                                        type_args.len()
                                    ),
                                    span,
                                ));
                            }
                            return Some((vec![type_id], TypeInterner::BYTES));
                        }
                        "from_bytes" => {
                            if !type_args.is_empty() {
                                self.sink.emit(errors::unknown_type(
                                    &format!(
                                        "{name} (expected 0 type arguments, got {})",
                                        type_args.len()
                                    ),
                                    span,
                                ));
                            }
                            return Some((
                                vec![TypeInterner::BYTES],
                                self.interner
                                    .intern(Type::Result(type_id, TypeInterner::STRING)),
                            ));
                        }
                        _ => {}
                    }
                }
            }
        }

        if let Some(signature) = self.json_raw_facade_builtin_signature(&name, type_args, span) {
            return Some(signature);
        }

        match name.as_str() {
            "int64.from_string" => Some((
                vec![TypeInterner::STRING],
                self.interner
                    .intern(Type::Result(TypeInterner::INT64, TypeInterner::STRING)),
            )),
            "string.from_int64" => Some((vec![TypeInterner::INT64], TypeInterner::STRING)),
            "string.from_float64" => Some((vec![TypeInterner::FLOAT64], TypeInterner::STRING)),
            "float64.from_int64" => Some((vec![TypeInterner::INT64], TypeInterner::FLOAT64)),
            "string.length" | "string.char_count" => {
                Some((vec![TypeInterner::STRING], TypeInterner::INT64))
            }
            "string.contains" | "string.starts_with" | "string.ends_with" => Some((
                vec![TypeInterner::STRING, TypeInterner::STRING],
                TypeInterner::BOOL,
            )),
            "string.trim" | "string.upper" | "string.lower" => {
                Some((vec![TypeInterner::STRING], TypeInterner::STRING))
            }
            "string.replace" => Some((
                vec![
                    TypeInterner::STRING,
                    TypeInterner::STRING,
                    TypeInterner::STRING,
                ],
                TypeInterner::STRING,
            )),
            "string.split" => Some((
                vec![TypeInterner::STRING, TypeInterner::STRING],
                self.interner.intern(Type::List(TypeInterner::STRING)),
            )),
            "string.join" => Some((
                vec![
                    self.interner.intern(Type::List(TypeInterner::STRING)),
                    TypeInterner::STRING,
                ],
                TypeInterner::STRING,
            )),
            "Environment.args" => Some((
                vec![TypeInterner::ERROR],
                self.interner.intern(Type::List(TypeInterner::STRING)),
            )),
            "Filesystem.read_file" => Some((
                vec![TypeInterner::ERROR, TypeInterner::STRING],
                self.interner
                    .intern(Type::Result(TypeInterner::STRING, TypeInterner::STRING)),
            )),
            "Filesystem.write_file" => Some((
                vec![
                    TypeInterner::ERROR,
                    TypeInterner::STRING,
                    TypeInterner::STRING,
                ],
                self.interner
                    .intern(Type::Result(TypeInterner::NOTHING, TypeInterner::STRING)),
            )),
            "Stdout.write" => Some((
                vec![TypeInterner::ERROR, TypeInterner::STRING],
                TypeInterner::NOTHING,
            )),
            "json.parse" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![TypeInterner::STRING], TypeInterner::ERROR));
                }

                let value_ty = self.resolve_type_expr(&type_args[0]);
                let result_ty = self
                    .interner
                    .intern(Type::Result(value_ty, TypeInterner::STRING));
                Some((vec![TypeInterner::STRING], result_ty))
            }
            "json.serialize" | "json.serialize_public" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![TypeInterner::ERROR], TypeInterner::ERROR));
                }

                let value_ty = self.resolve_type_expr(&type_args[0]);
                Some((vec![value_ty], TypeInterner::STRING))
            }
            "type.name" | "type.kind" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![], TypeInterner::ERROR));
                }
                let _ = self.resolve_type_expr(&type_args[0]);
                Some((vec![], TypeInterner::STRING))
            }
            "type.kind_tag" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![], TypeInterner::ERROR));
                }
                let _ = self.resolve_type_expr(&type_args[0]);
                Some((
                    vec![],
                    self.named_types
                        .get("TypeKind")
                        .copied()
                        .unwrap_or(TypeInterner::ERROR),
                ))
            }
            "type.primitive_tag" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![], TypeInterner::ERROR));
                }
                let _ = self.resolve_type_expr(&type_args[0]);
                let primitive_ty = self
                    .named_types
                    .get("TypePrimitive")
                    .copied()
                    .unwrap_or(TypeInterner::ERROR);
                Some((vec![], self.interner.intern(Type::Optional(primitive_ty))))
            }
            "type.has_secret" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![], TypeInterner::ERROR));
                }
                let _ = self.resolve_type_expr(&type_args[0]);
                Some((vec![], TypeInterner::BOOL))
            }
            "type.info" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![], TypeInterner::ERROR));
                }
                let _ = self.resolve_type_expr(&type_args[0]);
                let type_info_ty = self
                    .named_types
                    .get("TypeInfo")
                    .copied()
                    .unwrap_or(TypeInterner::ERROR);
                Some((vec![], type_info_ty))
            }
            "type.arg" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![TypeInterner::INT64], TypeInterner::ERROR));
                }
                let _ = self.resolve_type_expr(&type_args[0]);
                let type_info_ty = self
                    .named_types
                    .get("TypeInfo")
                    .copied()
                    .unwrap_or(TypeInterner::ERROR);
                Some((vec![TypeInterner::INT64], type_info_ty))
            }
            "type.construct_start" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![], TypeInterner::ERROR));
                }
                let target_ty = self.resolve_type_expr(&type_args[0]);
                if !matches!(
                    self.interner.resolve(target_ty),
                    Type::Struct(_) | Type::Bitfield(_)
                ) {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "{name} supports only structs and bitfields, got {}",
                            self.type_name(target_ty)
                        ),
                        span,
                    ));
                }
                Some((vec![], TypeInterner::TYPE_CONSTRUCTION))
            }
            "type.construct_variant_start" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![TypeInterner::ERROR], TypeInterner::ERROR));
                }
                let target_ty = self.resolve_type_expr(&type_args[0]);
                if !matches!(self.interner.resolve(target_ty), Type::Enum(_)) {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "{name} supports only enums, got {}",
                            self.type_name(target_ty)
                        ),
                        span,
                    ));
                }
                let type_variant_ty = self
                    .named_types
                    .get("TypeVariant")
                    .copied()
                    .unwrap_or(TypeInterner::ERROR);
                Some((
                    vec![type_variant_ty],
                    self.interner.intern(Type::Result(
                        TypeInterner::TYPE_CONSTRUCTION,
                        TypeInterner::STRING,
                    )),
                ))
            }
            "type.construct_put" => {
                if type_args.len() != 2 {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "{name} (expected 2 type arguments, got {})",
                            type_args.len()
                        ),
                        span,
                    ));
                    return Some((
                        vec![
                            TypeInterner::TYPE_CONSTRUCTION,
                            self.named_types
                                .get("TypeField")
                                .copied()
                                .unwrap_or(TypeInterner::ERROR),
                            TypeInterner::ERROR,
                        ],
                        TypeInterner::ERROR,
                    ));
                }
                let target_ty = self.resolve_type_expr(&type_args[0]);
                if !matches!(
                    self.interner.resolve(target_ty),
                    Type::Struct(_) | Type::Bitfield(_) | Type::Enum(_)
                ) {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "{name} supports only structs, bitfields, and enums, got {}",
                            self.type_name(target_ty)
                        ),
                        span,
                    ));
                }
                let field_ty = self.resolve_type_expr(&type_args[1]);
                let type_field_ty = self
                    .named_types
                    .get("TypeField")
                    .copied()
                    .unwrap_or(TypeInterner::ERROR);
                Some((
                    vec![TypeInterner::TYPE_CONSTRUCTION, type_field_ty, field_ty],
                    self.interner.intern(Type::Result(
                        TypeInterner::TYPE_CONSTRUCTION,
                        TypeInterner::STRING,
                    )),
                ))
            }
            "type.construct_finish" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![TypeInterner::TYPE_CONSTRUCTION], TypeInterner::ERROR));
                }
                let target_ty = self.resolve_type_expr(&type_args[0]);
                if !matches!(
                    self.interner.resolve(target_ty),
                    Type::Struct(_) | Type::Bitfield(_) | Type::Enum(_)
                ) {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "{name} supports only structs, bitfields, and enums, got {}",
                            self.type_name(target_ty)
                        ),
                        span,
                    ));
                }
                Some((
                    vec![TypeInterner::TYPE_CONSTRUCTION],
                    self.interner
                        .intern(Type::Result(target_ty, TypeInterner::STRING)),
                ))
            }
            "type.fields" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![], TypeInterner::ERROR));
                }
                let _ = self.resolve_type_expr(&type_args[0]);
                let type_field_ty = self
                    .named_types
                    .get("TypeField")
                    .copied()
                    .unwrap_or(TypeInterner::ERROR);
                Some((vec![], self.interner.intern(Type::List(type_field_ty))))
            }
            "type.bitfield_layout" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![], TypeInterner::ERROR));
                }
                let _ = self.resolve_type_expr(&type_args[0]);
                let type_bitfield_ty = self
                    .named_types
                    .get("TypeBitfield")
                    .copied()
                    .unwrap_or(TypeInterner::ERROR);
                Some((vec![], type_bitfield_ty))
            }
            "type.bitfield_fields" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![], TypeInterner::ERROR));
                }
                let _ = self.resolve_type_expr(&type_args[0]);
                let type_bitfield_field_ty = self
                    .named_types
                    .get("TypeBitfieldField")
                    .copied()
                    .unwrap_or(TypeInterner::ERROR);
                Some((
                    vec![],
                    self.interner.intern(Type::List(type_bitfield_field_ty)),
                ))
            }
            "type.variants" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![], TypeInterner::ERROR));
                }
                let _ = self.resolve_type_expr(&type_args[0]);
                let type_variant_ty = self
                    .named_types
                    .get("TypeVariant")
                    .copied()
                    .unwrap_or(TypeInterner::ERROR);
                Some((vec![], self.interner.intern(Type::List(type_variant_ty))))
            }
            "type.variant_value" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!("{name} (expected 1 type argument, got {})", type_args.len()),
                        span,
                    ));
                    return Some((vec![TypeInterner::ERROR], TypeInterner::ERROR));
                }

                let value_ty = self.resolve_type_expr(&type_args[0]);
                let type_variant_ty = self
                    .named_types
                    .get("TypeVariant")
                    .copied()
                    .unwrap_or(TypeInterner::ERROR);
                Some((vec![value_ty], type_variant_ty))
            }
            "type.field_value" => {
                if type_args.len() != 2 {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "{name} (expected 2 type arguments, got {})",
                            type_args.len()
                        ),
                        span,
                    ));
                    return Some((vec![TypeInterner::ERROR], TypeInterner::ERROR));
                }

                let value_ty = self.resolve_type_expr(&type_args[0]);
                let return_ty = self.resolve_type_expr(&type_args[1]);
                let type_field_ty = self
                    .named_types
                    .get("TypeField")
                    .copied()
                    .unwrap_or(TypeInterner::ERROR);
                Some((vec![value_ty, type_field_ty], return_ty))
            }
            "type.variant_field_value" => {
                if type_args.len() != 2 {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "{name} (expected 2 type arguments, got {})",
                            type_args.len()
                        ),
                        span,
                    ));
                    return Some((vec![TypeInterner::ERROR], TypeInterner::ERROR));
                }

                let value_ty = self.resolve_type_expr(&type_args[0]);
                let return_ty = self.resolve_type_expr(&type_args[1]);
                let type_field_ty = self
                    .named_types
                    .get("TypeField")
                    .copied()
                    .unwrap_or(TypeInterner::ERROR);
                Some((vec![value_ty, type_field_ty], return_ty))
            }
            "secret.redact" => Some((
                vec![self.interner.intern(Type::Secret(TypeInterner::ERROR))],
                TypeInterner::STRING,
            )),
            "secret.compare" => Some((
                vec![
                    self.interner.intern(Type::Secret(TypeInterner::ERROR)),
                    self.interner.intern(Type::Secret(TypeInterner::ERROR)),
                ],
                TypeInterner::BOOL,
            )),
            "bytes.new" => Some((vec![], TypeInterner::BYTES)),
            "bytes.length" => Some((vec![TypeInterner::BYTES], TypeInterner::INT64)),
            "bytes.slice" => Some((
                vec![
                    TypeInterner::BYTES,
                    TypeInterner::INT64,
                    TypeInterner::INT64,
                ],
                TypeInterner::BYTES,
            )),
            "bytes.concat" => Some((
                vec![TypeInterner::BYTES, TypeInterner::BYTES],
                TypeInterner::BYTES,
            )),
            "bytes.from_string" => Some((vec![TypeInterner::STRING], TypeInterner::BYTES)),
            "bytes.to_string" => Some((
                vec![TypeInterner::BYTES],
                self.interner
                    .intern(Type::Result(TypeInterner::STRING, TypeInterner::STRING)),
            )),
            "bytes.get" => Some((
                vec![TypeInterner::BYTES, TypeInterner::INT64],
                self.interner.intern(Type::Optional(TypeInterner::INT64)),
            )),
            "bytes.to_hex" => Some((vec![TypeInterner::BYTES], TypeInterner::STRING)),
            "bytes.from_hex" => Some((
                vec![TypeInterner::STRING],
                self.interner
                    .intern(Type::Result(TypeInterner::BYTES, TypeInterner::STRING)),
            )),
            "list.new" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "list.new (expected 1 type argument, got {})",
                            type_args.len()
                        ),
                        span,
                    ));
                    return Some((vec![], TypeInterner::ERROR));
                }
                let inner = self.resolve_type_expr(&type_args[0]);
                Some((vec![], self.interner.intern(Type::List(inner))))
            }
            "list.append" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "list.append (expected 1 type argument, got {})",
                            type_args.len()
                        ),
                        span,
                    ));
                    return Some((
                        vec![TypeInterner::ERROR, TypeInterner::ERROR],
                        TypeInterner::ERROR,
                    ));
                }
                let inner = self.resolve_type_expr(&type_args[0]);
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty, inner], list_ty))
            }
            "list.length" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                Some((
                    vec![self.interner.intern(Type::List(inner))],
                    TypeInterner::INT64,
                ))
            }
            "list.get" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "list.get (expected 1 type argument, got {})",
                            type_args.len()
                        ),
                        span,
                    ));
                    return Some((
                        vec![TypeInterner::ERROR, TypeInterner::INT64],
                        TypeInterner::ERROR,
                    ));
                }
                let inner = self.resolve_type_expr(&type_args[0]);
                Some((
                    vec![self.interner.intern(Type::List(inner)), TypeInterner::INT64],
                    self.interner.intern(Type::Optional(inner)),
                ))
            }
            // list builtins that transform a list → list
            "list.reverse" | "list.sort" | "list.unique" | "list.flatten" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty], list_ty))
            }
            "list.is_empty" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty], TypeInterner::BOOL))
            }
            "list.skip" | "list.take" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty, TypeInterner::INT64], list_ty))
            }
            "list.contains" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty, inner], TypeInterner::BOOL))
            }
            "list.index_of" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((
                    vec![list_ty, inner],
                    self.interner.intern(Type::Optional(TypeInterner::INT64)),
                ))
            }
            "list.remove" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty, TypeInterner::INT64], list_ty))
            }
            "list.concat" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty, list_ty], list_ty))
            }
            "list.zip" => {
                let inner_a = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let inner_b = if type_args.len() >= 2 {
                    self.resolve_type_expr(&type_args[1])
                } else {
                    TypeInterner::ERROR
                };
                let list_a = self.interner.intern(Type::List(inner_a));
                let list_b = self.interner.intern(Type::List(inner_b));
                let result_inner = self.interner.intern(Type::List(TypeInterner::ERROR));
                let result_ty = self.interner.intern(Type::List(result_inner));
                Some((vec![list_a, list_b], result_ty))
            }
            // math builtins
            "math.sqrt" | "math.log" | "math.log2" | "math.log10" => {
                Some((vec![TypeInterner::FLOAT64], TypeInterner::FLOAT64))
            }
            "math.pow" => Some((
                vec![TypeInterner::FLOAT64, TypeInterner::FLOAT64],
                TypeInterner::FLOAT64,
            )),
            "math.floor" | "math.ceil" | "math.round" => {
                Some((vec![TypeInterner::FLOAT64], TypeInterner::FLOAT64))
            }
            "math.clamp" => Some((
                vec![
                    TypeInterner::FLOAT64,
                    TypeInterner::FLOAT64,
                    TypeInterner::FLOAT64,
                ],
                TypeInterner::FLOAT64,
            )),
            "math.average" | "math.median" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::FLOAT64
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty], TypeInterner::FLOAT64))
            }
            // string extras
            "string.reverse" | "string.trim_start" | "string.trim_end" => {
                Some((vec![TypeInterner::STRING], TypeInterner::STRING))
            }
            "string.after" | "string.before" => Some((
                vec![TypeInterner::STRING, TypeInterner::STRING],
                TypeInterner::STRING,
            )),
            // string.chars / string.words / string.lines → list[string]
            "string.chars" | "string.words" | "string.lines" => {
                let list_str = self.interner.intern(Type::List(TypeInterner::STRING));
                Some((vec![TypeInterner::STRING], list_str))
            }
            // random builtins
            "random.int64" => Some((
                vec![TypeInterner::INT64, TypeInterner::INT64],
                TypeInterner::INT64,
            )),
            "random.float64" => Some((vec![], TypeInterner::FLOAT64)),
            "random.bool" => Some((vec![], TypeInterner::BOOL)),
            "random.choice" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty], self.interner.intern(Type::Optional(inner))))
            }
            "random.shuffle" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty], list_ty))
            }
            "string.is_empty" => Some((vec![TypeInterner::STRING], TypeInterner::BOOL)),
            "string.is_not_empty" => Some((vec![TypeInterner::STRING], TypeInterner::BOOL)),
            "string.repeat" => Some((
                vec![TypeInterner::STRING, TypeInterner::INT64],
                TypeInterner::STRING,
            )),
            "string.slice" => Some((
                vec![
                    TypeInterner::STRING,
                    TypeInterner::INT64,
                    TypeInterner::INT64,
                ],
                TypeInterner::STRING,
            )),
            // string.pad_left is the canonical name; pad_start/pad_end are aliases
            "string.pad_left" | "string.pad_start" | "string.pad_end" => Some((
                vec![
                    TypeInterner::STRING,
                    TypeInterner::INT64,
                    TypeInterner::STRING,
                ],
                TypeInterner::STRING,
            )),
            "string.slugify" => Some((vec![TypeInterner::STRING], TypeInterner::STRING)),
            "string.truncate" => Some((
                vec![
                    TypeInterner::STRING,
                    TypeInterner::INT64,
                    TypeInterner::STRING,
                ],
                TypeInterner::STRING,
            )),
            "string.between" => Some((
                vec![
                    TypeInterner::STRING,
                    TypeInterner::STRING,
                    TypeInterner::STRING,
                ],
                TypeInterner::STRING,
            )),
            "map.new" => {
                let (k, v) = self.map_type_args(type_args);
                let map_ty = self.interner.intern(Type::Map(k, v));
                Some((vec![], map_ty))
            }
            "map.length" => {
                let (k, v) = self.map_type_args(type_args);
                let map_ty = self.interner.intern(Type::Map(k, v));
                Some((vec![map_ty], TypeInterner::INT64))
            }
            "map.is_empty" => {
                let (k, v) = self.map_type_args(type_args);
                let map_ty = self.interner.intern(Type::Map(k, v));
                Some((vec![map_ty], TypeInterner::BOOL))
            }
            "map.has" => {
                let (k, v) = self.map_type_args(type_args);
                let map_ty = self.interner.intern(Type::Map(k, v));
                Some((vec![map_ty, k], TypeInterner::BOOL))
            }
            "map.get" => {
                let (k, v) = self.map_type_args(type_args);
                let map_ty = self.interner.intern(Type::Map(k, v));
                Some((vec![map_ty, k], self.interner.intern(Type::Optional(v))))
            }
            "map.insert" => {
                let (k, v) = self.map_type_args(type_args);
                let map_ty = self.interner.intern(Type::Map(k, v));
                Some((vec![map_ty, k, v], map_ty))
            }
            "map.remove" => {
                let (k, v) = self.map_type_args(type_args);
                let map_ty = self.interner.intern(Type::Map(k, v));
                Some((vec![map_ty, k], map_ty))
            }
            "map.keys" => {
                let (k, v) = self.map_type_args(type_args);
                let map_ty = self.interner.intern(Type::Map(k, v));
                Some((vec![map_ty], self.interner.intern(Type::List(k))))
            }
            "map.values" => {
                let (k, v) = self.map_type_args(type_args);
                let map_ty = self.interner.intern(Type::Map(k, v));
                Some((vec![map_ty], self.interner.intern(Type::List(v))))
            }
            // list.first / list.last — no fn arg
            "list.first" | "list.last" => {
                let inner = if type_args.len() == 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty], self.interner.intern(Type::Optional(inner))))
            }
            // higher-order: list.filter[T](list, fn) -> list[T]
            "list.filter" => {
                let inner = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty, TypeInterner::ERROR], list_ty))
            }
            // higher-order: list.map[T, U](list, fn) -> list[U]
            "list.map" => {
                let inner_u = if type_args.len() >= 2 {
                    self.resolve_type_expr(&type_args[1])
                } else {
                    TypeInterner::ERROR
                };
                let inner_t = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_t = self.interner.intern(Type::List(inner_t));
                let list_u = self.interner.intern(Type::List(inner_u));
                Some((vec![list_t, TypeInterner::ERROR], list_u))
            }
            // higher-order: list.find[T](list, fn) -> optional[T]
            "list.find" => {
                let inner = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((
                    vec![list_ty, TypeInterner::ERROR],
                    self.interner.intern(Type::Optional(inner)),
                ))
            }
            // higher-order: list.sort_by[T](list, fn) -> list[T]
            "list.sort_by" => {
                let inner = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty, TypeInterner::ERROR], list_ty))
            }
            // higher-order: list.all / list.any [T](list, fn) -> bool
            "list.all" | "list.any" => {
                let inner = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty, TypeInterner::ERROR], TypeInterner::BOOL))
            }
            // higher-order: list.count[T](list, fn) -> int64
            "list.count" => {
                let inner = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty, TypeInterner::ERROR], TypeInterner::INT64))
            }
            // list.sum[T](list) -> T (numeric)
            "list.sum" => {
                let inner = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty], inner))
            }
            // list.group_by[T](list, fn) -> map[string, list[T]]
            "list.group_by" => {
                let inner = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                let group_map_ty = self
                    .interner
                    .intern(Type::Map(TypeInterner::STRING, list_ty));
                Some((vec![list_ty, TypeInterner::ERROR], group_map_ty))
            }
            // list.reduce[T, U](list, initial, fn) -> U
            "list.reduce" => {
                let inner = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((
                    vec![list_ty, TypeInterner::ERROR, TypeInterner::ERROR],
                    TypeInterner::ERROR,
                ))
            }
            // list.chunk[T](list, size) -> list[list[T]]
            "list.chunk" => {
                let inner = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                let list_of_list = self.interner.intern(Type::List(list_ty));
                Some((vec![list_ty, TypeInterner::INT64], list_of_list))
            }
            // list.sort_by_index[T](list[T], index) -> list[T]
            // where T is the element type (e.g., list[string])
            "list.sort_by_index" => {
                let elem = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(elem));
                Some((vec![list_ty, TypeInterner::INT64], list_ty))
            }
            "list.is_sorted" => {
                let inner = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty], TypeInterner::BOOL))
            }
            "list.all_elements_in" => {
                let inner = if type_args.len() >= 1 {
                    self.resolve_type_expr(&type_args[0])
                } else {
                    TypeInterner::ERROR
                };
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty, list_ty], TypeInterner::BOOL))
            }
            // map extras
            "map.set" => {
                // alias for map.insert
                let (k, v) = self.map_type_args(type_args);
                let map_ty = self.interner.intern(Type::Map(k, v));
                Some((vec![map_ty, k, v], map_ty))
            }
            "map.get_or" => {
                let (k, v) = self.map_type_args(type_args);
                let map_ty = self.interner.intern(Type::Map(k, v));
                Some((vec![map_ty, k, v], v))
            }
            "map.merge" => {
                let (k, v) = self.map_type_args(type_args);
                let map_ty = self.interner.intern(Type::Map(k, v));
                Some((vec![map_ty, map_ty], map_ty))
            }
            "map.contains_key" => {
                // alias for map.has
                let (k, v) = self.map_type_args(type_args);
                let map_ty = self.interner.intern(Type::Map(k, v));
                Some((vec![map_ty, k], TypeInterner::BOOL))
            }
            "uuid.new" => Some((vec![], TypeInterner::STRING)),
            // char-level string operations
            "string.take_chars" | "string.take_last_chars" | "string.drop_chars" => Some((
                vec![TypeInterner::STRING, TypeInterner::INT64],
                TypeInterner::STRING,
            )),
            "string.char_at" => {
                let opt_str = self.interner.intern(Type::Optional(TypeInterner::STRING));
                Some((vec![TypeInterner::STRING, TypeInterner::INT64], opt_str))
            }
            // encoding module (all string → string)
            "encoding.base64_encode"
            | "encoding.base64_decode"
            | "encoding.hex_encode"
            | "encoding.hex_decode"
            | "encoding.url_encode"
            | "encoding.url_decode" => Some((vec![TypeInterner::STRING], TypeInterner::STRING)),
            // crypto module (string → string)
            "crypto.sha256" | "crypto.md5" => {
                Some((vec![TypeInterner::STRING], TypeInterner::STRING))
            }
            _ => None,
        }
    }

    // ------------------------------------------------------------------
    // Module
    // ------------------------------------------------------------------

    fn check_module(&mut self, module: &Module) {
        self.collect_type_aliases(module);

        // First pass: predeclare all user-defined types so function signatures,
        // fields, and methods can refer to them by name.
        let mut current_file = None;
        let mut current_namespace = None;
        for item in &module.items {
            Self::update_current_namespace(item, &mut current_file, &mut current_namespace);
            match item {
                Item::Interface(def) => {
                    self.predeclare_interface(def, current_namespace.as_deref())
                }
                Item::Struct(def) => self.predeclare_struct(def, current_namespace.as_deref()),
                Item::Bitfield(def) => self.predeclare_bitfield(def, current_namespace.as_deref()),
                Item::Enum(def) => self.predeclare_enum(def, current_namespace.as_deref()),
                Item::Actor(def) => self.predeclare_actor(def, current_namespace.as_deref()),
                _ => {}
            }
        }

        self.seed_legacy_compat_aliases();

        // Second pass: fill in the struct/enum contents now that all names exist.
        let mut current_file = None;
        let mut current_namespace = None;
        for item in &module.items {
            Self::update_current_namespace(item, &mut current_file, &mut current_namespace);
            match item {
                Item::Interface(def) => self.finish_interface(def, current_namespace.as_deref()),
                Item::Struct(def) => self.finish_struct(def, current_namespace.as_deref()),
                Item::Bitfield(def) => self.finish_bitfield(def, current_namespace.as_deref()),
                Item::Enum(def) => self.finish_enum(def, current_namespace.as_deref()),
                Item::Actor(def) => self.finish_actor(def, current_namespace.as_deref()),
                _ => {}
            }
        }

        // Third pass: register all top-level function signatures into the type env
        // and build the purity map.
        let mut current_file = None;
        let mut current_namespace = None;
        for item in &module.items {
            Self::update_current_namespace(item, &mut current_file, &mut current_namespace);
            match item {
                Item::Mutual(block) => {
                    for decl in &block.declarations {
                        let is_pure = Self::params_are_pure(&decl.params);
                        for name in Self::function_lookup_names(
                            current_namespace.as_deref(),
                            &decl.name.name,
                        ) {
                            self.purity_map.insert(name, is_pure);
                        }

                        if decl.type_params.is_empty() {
                            self.register_function_decl_sig(decl);
                            let signature = self.function_decl_signature(decl);
                            for name in Self::function_lookup_names(
                                current_namespace.as_deref(),
                                &decl.name.name,
                            ) {
                                self.function_signatures.insert(name, signature.clone());
                            }
                        }
                    }
                }
                Item::Function(func) => {
                    if func.type_params.is_empty() {
                        self.register_function_sig(func);
                        let signature = self.function_signature(func);
                        for name in Self::function_lookup_names(
                            current_namespace.as_deref(),
                            &func.name.name,
                        ) {
                            self.function_signatures.insert(name, signature.clone());
                        }
                    } else {
                        // Generic function — store the template; type checking happens at call sites.
                        for name in Self::function_lookup_names(
                            current_namespace.as_deref(),
                            &func.name.name,
                        ) {
                            self.generic_function_templates.insert(name, func.clone());
                        }
                    }
                    let is_pure = Self::function_is_pure(func);
                    for name in
                        Self::function_lookup_names(current_namespace.as_deref(), &func.name.name)
                    {
                        self.purity_map.insert(name, is_pure);
                    }
                }
                Item::Interface(def) => {
                    for method in &def.methods {
                        let is_pure = Self::params_are_pure(&method.params);
                        for owner_name in
                            Self::type_lookup_names(current_namespace.as_deref(), &def.name.name)
                        {
                            self.purity_map
                                .insert(format!("{owner_name}.{}", method.name.name), is_pure);
                        }
                    }
                }
                Item::Implement(block) => self.register_implement_block(block),
                Item::Struct(def) => {
                    for method in &def.methods {
                        let is_pure = Self::function_is_pure(method);
                        for owner_name in
                            Self::type_lookup_names(current_namespace.as_deref(), &def.name.name)
                        {
                            self.purity_map
                                .insert(format!("{owner_name}.{}", method.name.name), is_pure);
                        }
                    }
                }
                Item::Bitfield(_) => {}
                _ => {}
            }
        }

        self.validate_mutual_blocks(module);
        self.validate_implement_blocks(module);

        // Fourth pass: type-check function bodies, methods, and verify blocks.
        let mut current_file = None;
        let mut current_namespace = None;
        for item in &module.items {
            Self::update_current_namespace(item, &mut current_file, &mut current_namespace);
            match item {
                Item::TypeAlias(alias) => {
                    self.check_type_alias(alias, current_namespace.as_deref())
                }
                Item::Function(func) => {
                    if func.type_params.is_empty() {
                        self.check_function(func);
                    }
                    // Generic function bodies are checked at each call site.
                }
                Item::Implement(block) => self.check_implement_block(block),
                Item::Struct(def) => {
                    let owner_name = Self::namespace_qualified_name(
                        current_namespace.as_deref(),
                        &def.name.name,
                    )
                    .unwrap_or_else(|| def.name.name.clone());
                    for method in &def.methods {
                        self.check_method(&owner_name, method);
                    }
                }
                Item::Bitfield(_) => {}
                Item::Actor(def) => self.check_actor(def),
                Item::VarDecl(decl) => self.check_var_decl(decl),
                Item::Verify(verify) => self.check_verify_block(verify),
                Item::Property(prop) => self.check_property_block(prop),
                _ => {}
            }
        }
    }

    fn collect_type_aliases(&mut self, module: &Module) {
        let mut current_file = None;
        let mut current_namespace = None;
        for item in &module.items {
            Self::update_current_namespace(item, &mut current_file, &mut current_namespace);
            let Item::TypeAlias(alias) = item else {
                continue;
            };
            let canonical = Self::canonical_name(current_namespace.as_deref(), &alias.name.name);
            self.type_aliases.insert(canonical, alias.clone());
        }
    }

    /// Returns true if a function has no capability-type parameters (i.e. is pure).
    fn function_is_pure(func: &FunctionDef) -> bool {
        Self::params_are_pure(&func.params)
    }

    fn params_are_pure(params: &[ast::Param]) -> bool {
        !params
            .iter()
            .any(|p| capability::type_expr_is_capability(&p.ty))
    }

    fn check_type_alias(&mut self, alias: &ast::TypeAlias, namespace: Option<&str>) {
        let alias_name = Self::canonical_name(namespace, &alias.name.name);
        let Some(constraint) = &alias.constraint else {
            let _ = self.resolve_type_alias(&alias_name, alias.name.span);
            return;
        };

        let alias_ty = self.resolve_type_alias(&alias_name, alias.name.span);
        let Some(_base_ty) = self.refinement_base_type(alias_ty) else {
            return;
        };
        let constraint_value_ty = self.refinement_boundary_input_type(alias_ty);

        let saved_name = self.current_function_name.clone();
        let saved_pure = self.current_function_pure;
        self.current_function_name = Some(format!("type {alias_name}"));
        self.current_function_pure = true;

        if let Some(def_id) = self.declaration_def_id(constraint.span()) {
            self.type_env.insert(def_id, constraint_value_ty);
        }

        let constraint_ty = self.check_expr(constraint);
        if constraint_ty != TypeInterner::ERROR && constraint_ty != TypeInterner::BOOL {
            self.sink.emit(errors::refinement_constraint_not_bool(
                &alias_name,
                &self.type_name(constraint_ty),
                constraint.span(),
            ));
        }

        self.current_function_name = saved_name;
        self.current_function_pure = saved_pure;
    }

    // ------------------------------------------------------------------
    // Actors
    // ------------------------------------------------------------------

    fn predeclare_actor(&mut self, def: &ast::ActorDef, namespace: Option<&str>) {
        let canonical_name = Self::canonical_name(namespace, &def.name.name);
        let aid = self.interner.add_actor(TypeActorDef {
            name: canonical_name,
            capability_params: Vec::new(),
            state_fields: Vec::new(),
            messages: Vec::new(),
        });
        let ty = self.interner.intern(Type::Actor(aid));
        self.register_named_type(namespace, &def.name.name, ty);
        if let Some(def_id) = self.declaration_def_id(def.name.span) {
            self.type_env.insert(def_id, ty);
        }
    }

    fn finish_actor(&mut self, def: &ast::ActorDef, namespace: Option<&str>) {
        let canonical_name = Self::canonical_name(namespace, &def.name.name);
        let Some(&ty) = self.named_types.get(&canonical_name) else {
            return;
        };
        let Type::Actor(aid) = *self.interner.resolve(ty) else {
            return;
        };

        let capability_params: Vec<(String, TypeId)> = def
            .capability_params
            .iter()
            .map(|p| (p.name.name.clone(), self.resolve_type_expr(&p.ty)))
            .collect();

        let state_fields: Vec<(String, TypeId)> = def
            .state_fields
            .iter()
            .map(|f| (f.name.name.clone(), self.resolve_type_expr(&f.ty)))
            .collect();

        let messages: Vec<ActorMessageDef> = def
            .handlers
            .iter()
            .map(|h| {
                let params = h
                    .params
                    .iter()
                    .map(|p| (p.name.name.clone(), self.resolve_type_expr(&p.ty)))
                    .collect();
                let responds = h
                    .responds
                    .as_ref()
                    .map(|t| self.resolve_type_expr(t))
                    .unwrap_or(TypeInterner::NOTHING);
                ActorMessageDef {
                    name: h.name.name.clone(),
                    params,
                    responds,
                }
            })
            .collect();

        self.interner.update_actor(
            aid,
            TypeActorDef {
                name: canonical_name,
                capability_params,
                state_fields,
                messages,
            },
        );
    }

    fn check_actor(&mut self, def: &ast::ActorDef) {
        let Some(&actor_ty) = self.named_types.get(&def.name.name) else {
            return;
        };
        let Type::Actor(aid) = *self.interner.resolve(actor_ty) else {
            return;
        };
        let actor_def = self.interner.resolve_actor(aid).clone();

        // Register state fields and capability params in a fresh type env scope.
        let mut local_env: HashMap<String, TypeId> = HashMap::new();
        for (name, ty) in &actor_def.capability_params {
            local_env.insert(name.clone(), *ty);
        }
        for (name, ty) in &actor_def.state_fields {
            local_env.insert(name.clone(), *ty);
        }

        // Type-check state field initializers.
        for field in &def.state_fields {
            let init_ty = self.check_expr(&field.value);
            let declared_ty = self.resolve_type_expr(&field.ty);
            if init_ty != TypeInterner::ERROR
                && declared_ty != TypeInterner::ERROR
                && !self.types_compatible(declared_ty, init_ty)
            {
                self.sink.emit(errors::type_mismatch(
                    &self.type_name(declared_ty),
                    &self.type_name(init_ty),
                    field.value.span(),
                ));
            }
        }

        // Type-check each handler body.
        let prev_return = self.current_return_type;
        let prev_respond = self.current_respond_type;
        let prev_function_name = self.current_function_name.clone();

        for (handler_ast, handler_def) in def.handlers.iter().zip(actor_def.messages.iter()) {
            // Set up respond type.
            let responds_ty = if handler_def.responds == TypeInterner::NOTHING {
                None
            } else {
                Some(handler_def.responds)
            };
            self.current_respond_type = responds_ty;
            self.current_return_type = None;
            self.current_function_name =
                Some(format!("{}.{}", def.name.name, handler_ast.name.name));

            // Register message params in the type env temporarily.
            for (param_ast, (_, param_ty)) in
                handler_ast.params.iter().zip(handler_def.params.iter())
            {
                if let Some(def_id) = self.declaration_def_id(param_ast.name.span) {
                    self.type_env.insert(def_id, *param_ty);
                }
            }

            // Register local_env vars into type_env using resolve declarations.
            for field in &def.state_fields {
                if let Some(def_id) = self.declaration_def_id(field.name.span) {
                    let ty = self.resolve_type_expr(&field.ty);
                    self.type_env.insert(def_id, ty);
                }
            }
            for cap in &def.capability_params {
                if let Some(def_id) = self.declaration_def_id(cap.name.span) {
                    let ty = self.resolve_type_expr(&cap.ty);
                    self.type_env.insert(def_id, ty);
                }
            }

            self.check_block(&handler_ast.body);
        }

        self.current_return_type = prev_return;
        self.current_respond_type = prev_respond;
        self.current_function_name = prev_function_name;
    }

    fn predeclare_struct(&mut self, def: &ast::StructDef, namespace: Option<&str>) {
        if !def.type_params.is_empty() {
            // Generic struct — store the template for later monomorphization.
            self.register_generic_struct_template(namespace, &def.name.name, def.clone());
            return;
        }

        let canonical_name = Self::canonical_name(namespace, &def.name.name);
        let sid = self.interner.add_struct(TypeStructDef {
            name: canonical_name,
            fields: Vec::new(),
            methods: Vec::new(),
        });
        let ty = self.interner.intern(Type::Struct(sid));
        self.register_named_type(namespace, &def.name.name, ty);

        if let Some(def_id) = self.declaration_def_id(def.name.span) {
            self.type_env.insert(def_id, ty);
        }
    }

    fn predeclare_interface(&mut self, def: &ast::InterfaceDecl, namespace: Option<&str>) {
        let canonical_name = Self::canonical_name(namespace, &def.name.name);
        let iid = self.interner.add_interface(TypeInterfaceDef {
            name: canonical_name,
            methods: Vec::new(),
        });
        let ty = self.interner.intern(Type::Interface(iid));
        self.register_named_type(namespace, &def.name.name, ty);

        if let Some(def_id) = self.declaration_def_id(def.name.span) {
            self.type_env.insert(def_id, ty);
        }
    }

    fn predeclare_bitfield(&mut self, def: &ast::BitfieldDef, namespace: Option<&str>) {
        let canonical_name = Self::canonical_name(namespace, &def.name.name);
        let bid = self.interner.add_bitfield(TypeBitfieldDef {
            name: canonical_name,
            network_order: def.network_order,
            fields: Vec::new(),
        });
        let ty = self.interner.intern(Type::Bitfield(bid));
        self.register_named_type(namespace, &def.name.name, ty);

        if let Some(def_id) = self.declaration_def_id(def.name.span) {
            self.type_env.insert(def_id, ty);
        }
    }

    fn finish_struct(&mut self, def: &ast::StructDef, namespace: Option<&str>) {
        if !def.type_params.is_empty() {
            // Generic structs are monomorphized on demand — nothing to finish here.
            return;
        }
        let canonical_name = Self::canonical_name(namespace, &def.name.name);
        let Some(&ty) = self.named_types.get(&canonical_name) else {
            return;
        };
        let Type::Struct(sid) = *self.interner.resolve(ty) else {
            return;
        };

        let fields: Vec<(String, TypeId)> = def
            .fields
            .iter()
            .map(|field| (field.name.name.clone(), self.resolve_type_expr(&field.ty)))
            .collect();
        let reflection_fields =
            self.reflection_fields_for_struct_def(def, namespace, fields.as_slice());
        self.reflection_fields
            .insert(canonical_name.clone(), reflection_fields.clone());
        self.reflection_fields_by_id
            .insert(ty, (canonical_name.clone(), reflection_fields));
        let methods = def
            .methods
            .iter()
            .map(|method| self.method_signature(method))
            .collect();

        self.interner.update_struct(
            sid,
            TypeStructDef {
                name: canonical_name,
                fields,
                methods,
            },
        );
    }

    fn finish_interface(&mut self, def: &ast::InterfaceDecl, namespace: Option<&str>) {
        let canonical_name = Self::canonical_name(namespace, &def.name.name);
        let Some(&ty) = self.named_types.get(&canonical_name) else {
            return;
        };
        let Type::Interface(iid) = *self.interner.resolve(ty) else {
            return;
        };

        let methods = def
            .methods
            .iter()
            .map(|method| self.function_decl_method_signature(method))
            .collect();

        self.interner.update_interface(
            iid,
            TypeInterfaceDef {
                name: canonical_name,
                methods,
            },
        );
    }

    fn finish_bitfield(&mut self, def: &ast::BitfieldDef, namespace: Option<&str>) {
        let canonical_name = Self::canonical_name(namespace, &def.name.name);
        let Some(&ty) = self.named_types.get(&canonical_name) else {
            return;
        };
        let Type::Bitfield(bid) = *self.interner.resolve(ty) else {
            return;
        };

        let list_u8 = self.interner.intern(Type::List(TypeInterner::UINT8));
        let mut fields = Vec::with_capacity(def.fields.len());
        let mut bits_before_payload = 0usize;
        for (index, field) in def.fields.iter().enumerate() {
            let (ty, kind) = match &field.kind {
                ast::BitfieldFieldKind::Bits { width, as_type } => {
                    if *width == 0 {
                        self.sink.emit(errors::invalid_bitfield_field(
                            &def.name.name,
                            &field.name.name,
                            "bit width must be at least 1",
                            field.span,
                        ));
                    }

                    let ty = if let Some(ty_expr) = as_type {
                        let resolved = self.resolve_type_expr(ty_expr);
                        if resolved != TypeInterner::ERROR {
                            match self.interner.resolve(resolved) {
                                Type::Enum(eid) => {
                                    let enum_def = self.interner.resolve_enum(*eid);
                                    if enum_def
                                        .variants
                                        .iter()
                                        .any(|variant| !variant.fields.is_empty())
                                    {
                                        self.sink.emit(errors::invalid_bitfield_field(
                                            &def.name.name,
                                            &field.name.name,
                                            "`as` annotations require an enum with only unit variants",
                                            field.span,
                                        ));
                                    }

                                    let max_discriminant = enum_def
                                        .variants
                                        .iter()
                                        .map(|variant| variant.discriminant)
                                        .max()
                                        .unwrap_or(0);
                                    let fits = if max_discriminant < 0 {
                                        false
                                    } else if *width >= 63 {
                                        true
                                    } else {
                                        (max_discriminant as u64) < (1_u64 << *width)
                                    };
                                    if !fits {
                                        self.sink.emit(errors::invalid_bitfield_field(
                                            &def.name.name,
                                            &field.name.name,
                                            "enum annotation has a discriminant that does not fit in the declared bit width",
                                            field.span,
                                        ));
                                    }
                                }
                                _ => {
                                    self.sink.emit(errors::invalid_bitfield_field(
                                        &def.name.name,
                                        &field.name.name,
                                        "`as` annotations must name an enum type",
                                        field.span,
                                    ));
                                }
                            }
                        }
                        resolved
                    } else {
                        TypeInterner::INT64
                    };
                    bits_before_payload += *width as usize;
                    (ty, TypeBitfieldFieldKind::Bits { width: *width })
                }
                ast::BitfieldFieldKind::Payload(ty_expr) => {
                    let resolved = self.resolve_type_expr(ty_expr);
                    if resolved != TypeInterner::ERROR && resolved != list_u8 {
                        self.sink.emit(errors::invalid_bitfield_field(
                            &def.name.name,
                            &field.name.name,
                            "payload fields must have type `list[uint8]`",
                            field.span,
                        ));
                    }
                    if index + 1 != def.fields.len() {
                        self.sink.emit(errors::invalid_bitfield_field(
                            &def.name.name,
                            &field.name.name,
                            "payload fields must be the final field",
                            field.span,
                        ));
                    }
                    if bits_before_payload % 8 != 0 {
                        self.sink.emit(errors::invalid_bitfield_field(
                            &def.name.name,
                            &field.name.name,
                            "payload fields must start on a byte boundary",
                            field.span,
                        ));
                    }
                    (resolved, TypeBitfieldFieldKind::Payload)
                }
            };

            fields.push(TypeBitfieldFieldDef {
                name: field.name.name.clone(),
                ty,
                kind,
            });
        }

        let reflection_fields =
            self.reflection_fields_for_bitfield_def(def, namespace, fields.as_slice());
        let reflection_bitfield =
            self.reflection_bitfield_info_for_def(def, namespace, fields.as_slice());
        self.reflection_fields
            .insert(canonical_name.clone(), reflection_fields.clone());
        self.reflection_bitfields
            .insert(canonical_name.clone(), reflection_bitfield.clone());
        self.reflection_fields_by_id
            .insert(ty, (canonical_name.clone(), reflection_fields));
        self.reflection_bitfields_by_id
            .insert(ty, (canonical_name.clone(), reflection_bitfield));

        self.interner.update_bitfield(
            bid,
            TypeBitfieldDef {
                name: canonical_name,
                network_order: def.network_order,
                fields,
            },
        );
    }

    fn predeclare_enum(&mut self, def: &ast::EnumDef, namespace: Option<&str>) {
        let canonical_name = Self::canonical_name(namespace, &def.name.name);
        let eid = self.interner.add_enum(TypeEnumDef {
            name: canonical_name,
            variants: Vec::new(),
        });
        let ty = self.interner.intern(Type::Enum(eid));
        self.register_named_type(namespace, &def.name.name, ty);

        if let Some(def_id) = self.declaration_def_id(def.name.span) {
            self.type_env.insert(def_id, ty);
        }
    }

    fn finish_enum(&mut self, def: &ast::EnumDef, namespace: Option<&str>) {
        let canonical_name = Self::canonical_name(namespace, &def.name.name);
        let Some(&ty) = self.named_types.get(&canonical_name) else {
            return;
        };
        let Type::Enum(eid) = *self.interner.resolve(ty) else {
            return;
        };

        let mut next_discriminant = 0_i64;
        let mut seen_discriminants = HashMap::new();
        let variants: Vec<VariantDef> = def
            .variants
            .iter()
            .map(|variant| {
                if variant.discriminant.is_some() && !variant.fields.is_empty() {
                    self.sink
                        .emit(errors::enum_discriminant_requires_unit_variant(
                            &def.name.name,
                            &variant.name.name,
                            variant.span,
                        ));
                }

                let discriminant = variant.discriminant.unwrap_or(next_discriminant);
                next_discriminant = discriminant.saturating_add(1);

                if let Some(previous_span) =
                    seen_discriminants.insert(discriminant, variant.name.span)
                {
                    self.sink.emit(errors::duplicate_enum_discriminant(
                        &def.name.name,
                        &variant.name.name,
                        discriminant,
                        variant.name.span,
                        previous_span,
                    ));
                }

                VariantDef {
                    name: variant.name.name.clone(),
                    fields: variant
                        .fields
                        .iter()
                        .map(|field| (field.name.name.clone(), self.resolve_type_expr(&field.ty)))
                        .collect(),
                    discriminant,
                }
            })
            .collect();
        let reflection_variants =
            self.reflection_variants_for_enum_def(def, namespace, variants.as_slice());
        self.reflection_variants
            .insert(canonical_name.clone(), reflection_variants.clone());
        self.reflection_variants_by_id
            .insert(ty, (canonical_name.clone(), reflection_variants));

        self.interner.update_enum(
            eid,
            TypeEnumDef {
                name: canonical_name,
                variants,
            },
        );
    }

    fn method_signature(&mut self, func: &FunctionDef) -> FunctionSig {
        let params = func
            .params
            .iter()
            .map(|param| {
                (
                    param.name.name.clone(),
                    self.resolve_type_expr(&param.ty),
                    param.view,
                )
            })
            .collect();
        let return_type = func
            .return_type
            .as_ref()
            .map(|ty| self.resolve_type_expr(ty))
            .unwrap_or(TypeInterner::NOTHING);

        FunctionSig {
            name: func.name.name.clone(),
            params,
            return_type,
            is_pure: Self::function_is_pure(func),
        }
    }

    fn function_decl_method_signature(&mut self, decl: &ast::FunctionDecl) -> FunctionSig {
        let params = decl
            .params
            .iter()
            .map(|param| {
                (
                    param.name.name.clone(),
                    self.resolve_type_expr(&param.ty),
                    param.view,
                )
            })
            .collect();
        let return_type = decl
            .return_type
            .as_ref()
            .map(|ty| self.resolve_type_expr(ty))
            .unwrap_or(TypeInterner::NOTHING);

        FunctionSig {
            name: decl.name.name.clone(),
            params,
            return_type,
            is_pure: Self::params_are_pure(&decl.params),
        }
    }

    fn function_signature(&mut self, func: &FunctionDef) -> (Vec<TypeId>, TypeId) {
        let params = func
            .params
            .iter()
            .map(|p| self.resolve_type_expr(&p.ty))
            .collect();
        let return_type = func
            .return_type
            .as_ref()
            .map(|t| self.resolve_type_expr(t))
            .unwrap_or(TypeInterner::NOTHING);
        (params, return_type)
    }

    fn function_decl_signature(&mut self, decl: &ast::FunctionDecl) -> (Vec<TypeId>, TypeId) {
        let params = decl
            .params
            .iter()
            .map(|p| self.resolve_type_expr(&p.ty))
            .collect();
        let return_type = decl
            .return_type
            .as_ref()
            .map(|t| self.resolve_type_expr(t))
            .unwrap_or(TypeInterner::NOTHING);
        (params, return_type)
    }

    // ------------------------------------------------------------------
    // Function registration (builds FunctionType + binds to DefId)
    // ------------------------------------------------------------------

    fn register_function_decl_sig(&mut self, decl: &ast::FunctionDecl) {
        let (param_types, return_type) = self.function_decl_signature(decl);
        let fn_type = self.interner.intern(Type::Function {
            params: param_types,
            return_type,
        });

        if let Some(def_id) = self.declaration_def_id(decl.name.span) {
            self.type_env.insert(def_id, fn_type);
        }
    }

    fn register_function_sig(&mut self, func: &FunctionDef) {
        let param_types: Vec<TypeId> = func
            .params
            .iter()
            .map(|p| self.resolve_type_expr(&p.ty))
            .collect();

        let return_type = func
            .return_type
            .as_ref()
            .map(|t| self.resolve_type_expr(t))
            .unwrap_or(TypeInterner::NOTHING);

        let fn_type = self.interner.intern(Type::Function {
            params: param_types,
            return_type,
        });

        // Bind the function name's DefId to this function type.
        if let Some(def_id) = self.declaration_def_id(func.name.span) {
            self.type_env.insert(def_id, fn_type);
        }
    }

    fn register_implement_block(&mut self, block: &ast::ImplementBlock) {
        let interface_ty = self.resolve_type_expr(&TypeExpr::Named(block.interface_name.clone()));
        let owner_ty = self.resolve_type_expr(&block.for_type);
        if interface_ty == TypeInterner::ERROR || owner_ty == TypeInterner::ERROR {
            return;
        }

        let owner_name = self.type_name(owner_ty);
        let interface_name = self.type_name(interface_ty);
        let method_sigs: Vec<_> = block
            .methods
            .iter()
            .map(|method| {
                let sig = self.method_signature(method);
                (method.name.name.clone(), sig)
            })
            .collect();

        let impl_methods = self.impl_methods_by_type.entry(owner_ty).or_default();
        let interface_methods = self
            .interface_impls
            .entry((interface_ty, owner_ty))
            .or_default();

        for (method_name, sig) in method_sigs {
            self.purity_map
                .insert(format!("{owner_name}.{method_name}"), sig.is_pure);
            self.purity_map
                .insert(format!("{interface_name}.{method_name}"), sig.is_pure);
            impl_methods.insert(method_name.clone(), sig.clone());
            interface_methods.insert(method_name, sig);
        }
    }

    fn validate_implement_blocks(&mut self, module: &Module) {
        for item in &module.items {
            let Item::Implement(block) = item else {
                continue;
            };

            let interface_ty =
                self.resolve_type_expr(&TypeExpr::Named(block.interface_name.clone()));
            let owner_ty = self.resolve_type_expr(&block.for_type);
            if interface_ty == TypeInterner::ERROR || owner_ty == TypeInterner::ERROR {
                continue;
            }

            let Type::Interface(iid) = *self.interner.resolve(interface_ty) else {
                self.sink.emit(errors::expected_interface(
                    &block.interface_name.name,
                    block.interface_name.span,
                ));
                continue;
            };
            let interface_def = self.interner.resolve_interface(iid).clone();

            let Some(impl_methods) = self.interface_impls.get(&(interface_ty, owner_ty)).cloned()
            else {
                continue;
            };

            let mut seen = HashSet::new();
            for method in &block.methods {
                if !seen.insert(method.name.name.clone()) {
                    self.sink.emit(errors::duplicate_implemented_method(
                        &self.type_name(owner_ty),
                        &method.name.name,
                        method.name.span,
                    ));
                    continue;
                }

                let Some(interface_method) = interface_def
                    .methods
                    .iter()
                    .find(|candidate| candidate.name == method.name.name)
                    .cloned()
                else {
                    self.sink.emit(errors::interface_has_no_member(
                        &interface_def.name,
                        &method.name.name,
                        method.name.span,
                    ));
                    continue;
                };

                let impl_sig = impl_methods
                    .get(&method.name.name)
                    .expect("impl method must exist");
                if !self.implementation_matches_interface(owner_ty, impl_sig, &interface_method) {
                    self.sink
                        .emit(errors::implemented_method_signature_mismatch(
                            &interface_def.name,
                            &self.type_name(owner_ty),
                            &method.name.name,
                            method.name.span,
                        ));
                }
            }

            for interface_method in &interface_def.methods {
                if !seen.contains(&interface_method.name) {
                    self.sink.emit(errors::missing_implemented_method(
                        &interface_def.name,
                        &self.type_name(owner_ty),
                        &interface_method.name,
                        block.span,
                    ));
                }
            }
        }
    }

    fn implementation_matches_interface(
        &mut self,
        owner_ty: TypeId,
        impl_sig: &FunctionSig,
        interface_sig: &FunctionSig,
    ) -> bool {
        if impl_sig.params.len() != interface_sig.params.len() {
            return false;
        }

        for (index, (impl_param, interface_param)) in impl_sig
            .params
            .iter()
            .zip(interface_sig.params.iter())
            .enumerate()
        {
            if impl_param.0 != interface_param.0 || impl_param.2 != interface_param.2 {
                return false;
            }

            let expected_ty = if index == 0 {
                match self.interner.resolve(interface_param.1) {
                    Type::Interface(_) => owner_ty,
                    _ => interface_param.1,
                }
            } else {
                interface_param.1
            };

            if !self.types_compatible(expected_ty, impl_param.1)
                || !self.types_compatible(impl_param.1, expected_ty)
            {
                return false;
            }
        }

        self.types_compatible(interface_sig.return_type, impl_sig.return_type)
            && self.types_compatible(impl_sig.return_type, interface_sig.return_type)
    }

    fn validate_mutual_blocks(&mut self, module: &Module) {
        let mut function_defs: HashMap<String, &FunctionDef> = HashMap::new();
        let mut current_file = None;
        let mut current_namespace = None;
        for item in &module.items {
            Self::update_current_namespace(item, &mut current_file, &mut current_namespace);
            if let Item::Function(func) = item {
                function_defs.insert(
                    Self::canonical_name(current_namespace.as_deref(), &func.name.name),
                    func,
                );
            }
        }

        let mut current_file = None;
        let mut current_namespace = None;
        for item in &module.items {
            Self::update_current_namespace(item, &mut current_file, &mut current_namespace);
            let Item::Mutual(block) = item else {
                continue;
            };

            for decl in &block.declarations {
                let canonical_name =
                    Self::canonical_name(current_namespace.as_deref(), &decl.name.name);
                let Some(func) = function_defs.get(&canonical_name).copied() else {
                    self.sink.emit(errors::mutual_function_missing_definition(
                        &decl.name.name,
                        decl.name.span,
                    ));
                    continue;
                };

                if !self.function_matches_decl(func, decl) {
                    self.sink.emit(errors::mutual_signature_mismatch(
                        &decl.name.name,
                        func.name.span,
                    ));
                }
            }
        }
    }

    fn function_matches_decl(&mut self, func: &FunctionDef, decl: &ast::FunctionDecl) -> bool {
        if !func.type_params.is_empty() || !decl.type_params.is_empty() {
            return Self::generic_function_matches_decl(func, decl);
        }

        if func.params.len() != decl.params.len() {
            return false;
        }

        for (func_param, decl_param) in func.params.iter().zip(decl.params.iter()) {
            if func_param.name.name != decl_param.name.name
                || func_param.view != decl_param.view
                || func_param.mutable != decl_param.mutable
            {
                return false;
            }

            let func_ty = self.resolve_type_expr(&func_param.ty);
            let decl_ty = self.resolve_type_expr(&decl_param.ty);
            if !self.types_compatible(decl_ty, func_ty) || !self.types_compatible(func_ty, decl_ty)
            {
                return false;
            }
        }

        let func_return = func
            .return_type
            .as_ref()
            .map(|ty| self.resolve_type_expr(ty))
            .unwrap_or(TypeInterner::NOTHING);
        let decl_return = decl
            .return_type
            .as_ref()
            .map(|ty| self.resolve_type_expr(ty))
            .unwrap_or(TypeInterner::NOTHING);

        self.types_compatible(decl_return, func_return)
            && self.types_compatible(func_return, decl_return)
    }

    fn generic_function_matches_decl(func: &FunctionDef, decl: &ast::FunctionDecl) -> bool {
        if func.type_params.len() != decl.type_params.len()
            || func.params.len() != decl.params.len()
        {
            return false;
        }

        let type_params: HashMap<String, String> = decl
            .type_params
            .iter()
            .zip(func.type_params.iter())
            .map(|(decl_param, func_param)| (decl_param.name.clone(), func_param.name.clone()))
            .collect();

        for (func_param, decl_param) in func.params.iter().zip(decl.params.iter()) {
            if func_param.name.name != decl_param.name.name
                || func_param.view != decl_param.view
                || func_param.mutable != decl_param.mutable
                || !Self::type_expr_matches_decl(&func_param.ty, &decl_param.ty, &type_params)
            {
                return false;
            }
        }

        Self::return_type_matches_decl(
            func.return_type.as_ref(),
            decl.return_type.as_ref(),
            &type_params,
        )
    }

    fn return_type_matches_decl(
        func_ty: Option<&TypeExpr>,
        decl_ty: Option<&TypeExpr>,
        type_params: &HashMap<String, String>,
    ) -> bool {
        match (func_ty, decl_ty) {
            (None, None) => true,
            (Some(func_ty), Some(decl_ty)) => {
                Self::type_expr_matches_decl(func_ty, decl_ty, type_params)
            }
            (None, Some(decl_ty)) => Self::type_expr_is_nothing(decl_ty),
            (Some(func_ty), None) => Self::type_expr_is_nothing(func_ty),
        }
    }

    fn type_expr_is_nothing(ty: &TypeExpr) -> bool {
        matches!(ty, TypeExpr::Named(name) if name.name == "nothing")
    }

    fn type_expr_matches_decl(
        func_ty: &TypeExpr,
        decl_ty: &TypeExpr,
        type_params: &HashMap<String, String>,
    ) -> bool {
        match (func_ty, decl_ty) {
            (TypeExpr::Named(func_name), TypeExpr::Named(decl_name)) => {
                if let Some(expected_func_name) = type_params.get(&decl_name.name) {
                    func_name.name == *expected_func_name
                } else {
                    func_name.name == decl_name.name
                }
            }
            (
                TypeExpr::Generic(func_name, func_args, _),
                TypeExpr::Generic(decl_name, decl_args, _),
            ) => {
                func_name.name == decl_name.name
                    && func_args.len() == decl_args.len()
                    && func_args
                        .iter()
                        .zip(decl_args.iter())
                        .all(|(func_arg, decl_arg)| {
                            Self::type_expr_matches_decl(func_arg, decl_arg, type_params)
                        })
            }
            (TypeExpr::View(func_inner, _), TypeExpr::View(decl_inner, _)) => {
                Self::type_expr_matches_decl(func_inner, decl_inner, type_params)
            }
            (
                TypeExpr::Function(func_params, func_return, _),
                TypeExpr::Function(decl_params, decl_return, _),
            ) => {
                func_params.len() == decl_params.len()
                    && func_params
                        .iter()
                        .zip(decl_params.iter())
                        .all(|(func_param, decl_param)| {
                            Self::type_expr_matches_decl(func_param, decl_param, type_params)
                        })
                    && Self::type_expr_matches_decl(func_return, decl_return, type_params)
            }
            _ => false,
        }
    }

    // ------------------------------------------------------------------
    // Function body
    // ------------------------------------------------------------------

    fn check_function(&mut self, func: &FunctionDef) {
        self.check_function_impl(func, func.name.name.clone());
    }

    fn check_method(&mut self, owner: &str, func: &FunctionDef) {
        self.check_function_impl(func, format!("{owner}.{}", func.name.name));
    }

    fn check_implement_block(&mut self, block: &ast::ImplementBlock) {
        let owner_ty = self.resolve_type_expr(&block.for_type);
        let owner_name = self.type_name(owner_ty);
        for method in &block.methods {
            self.check_method(&owner_name, method);
        }
    }

    fn check_function_impl(&mut self, func: &FunctionDef, function_name: String) {
        let return_type = func
            .return_type
            .as_ref()
            .map(|t| self.resolve_type_expr(t))
            .unwrap_or(TypeInterner::NOTHING);

        self.current_return_type = Some(return_type);

        // Set the purity context for this function.
        let is_pure = Self::function_is_pure(func);
        self.current_function_name = Some(function_name);
        self.current_function_pure = is_pure;

        // Bind parameter types into the type environment.
        for param in &func.params {
            let param_type = self.resolve_type_expr(&param.ty);
            if let Some(def_id) = self.declaration_def_id(param.name.span) {
                self.type_env.insert(def_id, param_type);
            }
        }

        self.check_block(&func.body);

        self.current_return_type = None;
        self.current_function_name = None;
        self.current_function_pure = false;
    }

    fn check_verify_block(&mut self, verify: &VerifyBlock) {
        self.in_verify_block = true;
        self.current_verify_name = Some(verify.name.name.clone());
        self.check_block(&verify.body);
        self.in_verify_block = false;
        self.current_verify_name = None;
    }

    fn check_property_block(&mut self, prop: &ast::PropertyBlock) {
        self.in_property_block = true;
        self.check_block(&prop.body);
        self.in_property_block = false;
    }

    // ------------------------------------------------------------------
    // Block
    // ------------------------------------------------------------------

    fn check_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
    }

    // ------------------------------------------------------------------
    // Statements
    // ------------------------------------------------------------------

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl(decl) => self.check_var_decl(decl),
            Stmt::Assign(assign) => self.check_assign(assign),
            Stmt::Return(ret) => self.check_return(ret),
            Stmt::ComptimeTypeBind(bind) => self.check_comptime_type_bind(bind),
            Stmt::If(if_stmt) => self.check_if(if_stmt),
            Stmt::For(for_stmt) => self.check_for(for_stmt),
            Stmt::While(while_stmt) => self.check_while(while_stmt),
            Stmt::Expr(expr_stmt) => {
                let ty = self.check_expr(&expr_stmt.expr);
                // Warn if a result or optional value is silently discarded.
                if ty != TypeInterner::ERROR {
                    let resolved = self.interner.resolve(ty);
                    if matches!(resolved, Type::Result(_, _)) {
                        self.sink.emit(errors::unhandled_result(expr_stmt.span));
                    } else if matches!(resolved, Type::Optional(_)) {
                        self.sink.emit(errors::unhandled_optional(expr_stmt.span));
                    }
                }
            }
            Stmt::Assert(assert_stmt) => self.check_assert(assert_stmt),
            Stmt::Trace(trace_stmt) => self.check_trace(trace_stmt),
            Stmt::Breakpoint(breakpoint_stmt) => self.check_breakpoint(breakpoint_stmt),
            Stmt::Match(match_stmt) => self.check_match(match_stmt),
            Stmt::Respond(resp) => self.check_respond(resp),
            Stmt::Use(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn check_comptime_type_bind(&mut self, bind: &ast::ComptimeTypeBindStmt) {
        self.check_expr(&bind.value);

        if let Some(bound_type_expr) = comptime_type_info_binding(&bind.value) {
            let bound_ty = self.resolve_type_expr(bound_type_expr);
            if bound_ty == TypeInterner::ERROR {
                self.check_block(&bind.body);
                return;
            }

            self.check_comptime_type_bind_body(&bind.name.name, bound_ty, &bind.body);
            return;
        }

        if let Some((source_type_expr, index)) = comptime_type_arg_binding(&bind.value) {
            let source_ty = self.resolve_type_expr(source_type_expr);
            if source_ty == TypeInterner::ERROR {
                self.check_block(&bind.body);
                return;
            }

            let arg_types = self.type_info_arg_types_for_type_expr(source_type_expr);
            if let Some(&bound_ty) = arg_types.get(index) {
                if bound_ty != TypeInterner::ERROR {
                    self.check_comptime_type_bind_body(&bind.name.name, bound_ty, &bind.body);
                }
                return;
            }

            self.sink
                .emit(errors::invalid_comptime_type_binding(bind.value.span()));
            self.check_block(&bind.body);
            return;
        }

        if let Some(field_name) = reflected_field_type_info_binding(&bind.value) {
            if let Some(field_types) = self.reflected_field_types_for_name(field_name) {
                for field_ty in field_types {
                    if field_ty != TypeInterner::ERROR {
                        self.check_comptime_type_bind_body(&bind.name.name, field_ty, &bind.body);
                    }
                }
                return;
            }
        }

        if let Some(info_name) = reflected_type_info_binding(&bind.value) {
            if let Some(info_types) = self.reflected_type_info_types_for_name(info_name) {
                for info_ty in info_types {
                    if info_ty != TypeInterner::ERROR {
                        self.check_comptime_type_bind_body(&bind.name.name, info_ty, &bind.body);
                    }
                }
                return;
            }
        }

        self.sink
            .emit(errors::invalid_comptime_type_binding(bind.value.span()));
        self.check_block(&bind.body);
    }

    fn check_comptime_type_bind_body(&mut self, name: &str, bound_ty: TypeId, body: &Block) {
        let previous = self.type_var_subst.insert(name.to_string(), bound_ty);
        self.check_block(body);
        if let Some(previous) = previous {
            self.type_var_subst.insert(name.to_string(), previous);
        } else {
            self.type_var_subst.remove(name);
        }
    }

    fn reflected_field_types_for_name(&self, name: &str) -> Option<Vec<TypeId>> {
        self.reflected_field_type_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .cloned()
    }

    fn reflected_type_info_types_for_name(&self, name: &str) -> Option<Vec<TypeId>> {
        self.reflected_type_info_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .cloned()
    }

    fn reflected_variant_owner_for_name(&self, name: &str) -> Option<TypeId> {
        self.reflected_variant_type_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .copied()
    }

    fn reflected_field_types_for_owner(&self, owner_ty: TypeId) -> Vec<TypeId> {
        match self.interner.resolve(owner_ty) {
            Type::Struct(sid) => self
                .interner
                .resolve_struct(*sid)
                .fields
                .iter()
                .map(|(_, ty)| *ty)
                .collect(),
            Type::Bitfield(bid) => self
                .interner
                .resolve_bitfield(*bid)
                .fields
                .iter()
                .map(|field| field.ty)
                .collect(),
            _ => Vec::new(),
        }
    }

    fn reflected_variant_field_types_for_owner(&self, owner_ty: TypeId) -> Vec<TypeId> {
        match self.interner.resolve(owner_ty) {
            Type::Enum(eid) => self
                .interner
                .resolve_enum(*eid)
                .variants
                .iter()
                .flat_map(|variant| variant.fields.iter().map(|(_, ty)| *ty))
                .collect(),
            _ => Vec::new(),
        }
    }

    fn reflected_type_info_arg_types_for_iterable(
        &mut self,
        iterable: &Expr,
    ) -> Option<Vec<TypeId>> {
        let source = reflected_type_info_args_source(iterable)?;
        let source_types = match source {
            ReflectedTypeInfoSource::Direct(ty) => vec![self.resolve_type_expr(ty)],
            ReflectedTypeInfoSource::Field(field_name) => {
                self.reflected_field_types_for_name(field_name)?
            }
            ReflectedTypeInfoSource::TypeInfo(info_name) => {
                self.reflected_type_info_types_for_name(info_name)?
            }
        };

        Some(
            source_types
                .into_iter()
                .flat_map(|ty| self.type_info_arg_types_for_type(ty))
                .collect(),
        )
    }

    fn type_info_arg_types_for_type(&self, ty: TypeId) -> Vec<TypeId> {
        match self.interner.resolve(ty) {
            Type::List(inner) | Type::Set(inner) | Type::Optional(inner) | Type::Secret(inner) => {
                vec![*inner]
            }
            Type::Map(key, value) | Type::Result(key, value) => vec![*key, *value],
            Type::Refinement { base, .. } => vec![*base],
            Type::Function {
                params,
                return_type,
            } => params
                .iter()
                .copied()
                .chain(std::iter::once(*return_type))
                .collect(),
            _ => Vec::new(),
        }
    }

    fn type_info_arg_types_for_type_expr(&mut self, ty: &TypeExpr) -> Vec<TypeId> {
        match ty {
            TypeExpr::View(inner, _) => self.type_info_arg_types_for_type_expr(inner),
            TypeExpr::Named(ident) if self.type_aliases.contains_key(&ident.name) => {
                let alias = self
                    .type_aliases
                    .get(&ident.name)
                    .cloned()
                    .expect("type alias existence checked before lookup");
                vec![self.resolve_type_expr(&alias.base_type)]
            }
            TypeExpr::Generic(_, args, _) => {
                args.iter().map(|arg| self.resolve_type_expr(arg)).collect()
            }
            TypeExpr::Function(params, return_type, _) => params
                .iter()
                .chain(std::iter::once(return_type.as_ref()))
                .map(|arg| self.resolve_type_expr(arg))
                .collect(),
            _ => {
                let resolved = self.resolve_type_expr(ty);
                self.type_info_arg_types_for_type(resolved)
            }
        }
    }

    fn push_reflected_field_type_scope(&mut self, field_name: &str, field_types: Vec<TypeId>) {
        let mut scope = HashMap::new();
        scope.insert(field_name.to_string(), field_types);
        self.reflected_field_type_scopes.push(scope);
    }

    fn pop_reflected_field_type_scope(&mut self) {
        self.reflected_field_type_scopes.pop();
    }

    fn push_reflected_type_info_scope(&mut self, info_name: &str, info_types: Vec<TypeId>) {
        let mut scope = HashMap::new();
        scope.insert(info_name.to_string(), info_types);
        self.reflected_type_info_scopes.push(scope);
    }

    fn pop_reflected_type_info_scope(&mut self) {
        self.reflected_type_info_scopes.pop();
    }

    fn push_reflected_variant_type_scope(&mut self, variant_name: &str, owner_ty: TypeId) {
        let mut scope = HashMap::new();
        scope.insert(variant_name.to_string(), owner_ty);
        self.reflected_variant_type_scopes.push(scope);
    }

    fn pop_reflected_variant_type_scope(&mut self) {
        self.reflected_variant_type_scopes.pop();
    }

    fn check_trace(&mut self, trace_stmt: &ast::TraceStmt) {
        self.check_ident(&trace_stmt.name);
    }

    fn check_breakpoint(&mut self, breakpoint_stmt: &ast::BreakpointStmt) {
        if let Some(condition) = &breakpoint_stmt.condition {
            let cond_type = self.check_expr(condition);
            if cond_type != TypeInterner::ERROR && cond_type != TypeInterner::BOOL {
                self.sink.emit(errors::condition_not_bool(
                    &self.type_name(cond_type),
                    condition.span(),
                ));
            }
        }
    }

    fn check_respond(&mut self, resp: &ast::RespondStmt) {
        let val_ty = self.check_expr(&resp.value);
        match self.current_respond_type {
            None => {
                self.sink.emit(errors::respond_outside_handler(resp.span));
            }
            Some(expected) => {
                if val_ty != TypeInterner::ERROR
                    && expected != TypeInterner::ERROR
                    && !self.types_compatible(expected, val_ty)
                {
                    self.sink.emit(errors::type_mismatch(
                        &self.type_name(expected),
                        &self.type_name(val_ty),
                        resp.value.span(),
                    ));
                }
            }
        }
    }

    fn check_expr_for_expected(
        &mut self,
        expr: &Expr,
        expected_ty: TypeId,
        allow_refinement_handle: bool,
    ) -> TypeId {
        let ty = match expr {
            Expr::Coarsen(inner, span) => {
                let inner_ty = self.check_expr(inner);
                if inner_ty == TypeInterner::ERROR {
                    TypeInterner::ERROR
                } else if !self.is_refinement_type(inner_ty) {
                    self.sink.emit(errors::coarsen_requires_refinement(
                        &self.type_name(inner_ty),
                        *span,
                    ));
                    inner_ty
                } else if self.can_coarsen_to(inner_ty, expected_ty)
                    || self.fully_coarsened_type(inner_ty) == expected_ty
                {
                    expected_ty
                } else {
                    self.fully_coarsened_type(inner_ty)
                }
            }
            Expr::Handle(target, bind_name, body, span)
                if allow_refinement_handle && self.is_refinement_type(expected_ty) =>
            {
                let target_ty = self.check_expr(target);
                if matches!(
                    self.interner.resolve(target_ty),
                    Type::Result(_, _) | Type::Optional(_)
                ) {
                    self.check_handle_with_target_type(target_ty, bind_name.as_ref(), body, *span)
                } else {
                    self.check_refinement_handle_with_input_type(
                        expected_ty,
                        target_ty,
                        target.span(),
                        bind_name.as_ref(),
                        body,
                        *span,
                    )
                }
            }
            _ => {
                let actual_ty = self.check_expr(expr);
                if actual_ty != TypeInterner::ERROR
                    && self.type_requires_handle_error(expected_ty, actual_ty)
                {
                    self.sink
                        .emit(errors::result_requires_handle_error(expr.span()));
                    expected_ty
                } else if actual_ty != TypeInterner::ERROR
                    && self.type_requires_bare_handle(expected_ty, actual_ty)
                {
                    self.sink
                        .emit(errors::optional_requires_bare_handle(expr.span()));
                    expected_ty
                } else if allow_refinement_handle
                    && self.is_refinement_type(expected_ty)
                    && actual_ty != TypeInterner::ERROR
                    && actual_ty != expected_ty
                    && self.can_refine_from(actual_ty, expected_ty)
                {
                    self.sink.emit(errors::refinement_requires_handle_error(
                        &self.type_name(expected_ty),
                        &self.type_name(actual_ty),
                        expr.span(),
                    ));
                    expected_ty
                } else {
                    actual_ty
                }
            }
        };

        self.type_map.insert(expr.span(), ty);
        ty
    }

    fn check_refinement_handle_with_input_type(
        &mut self,
        refinement_ty: TypeId,
        input_ty: TypeId,
        target_span: Span,
        bind_name: Option<&ast::Ident>,
        body: &Block,
        span: Span,
    ) -> TypeId {
        let expected_input_ty = self.refinement_boundary_input_type(refinement_ty);
        let Some(_base_ty) = self.refinement_base_type(refinement_ty) else {
            return TypeInterner::ERROR;
        };

        if input_ty != TypeInterner::ERROR && !self.can_refine_from(input_ty, refinement_ty) {
            self.sink.emit(errors::type_mismatch(
                &self.type_name(expected_input_ty),
                &self.type_name(input_ty),
                target_span,
            ));
        }

        if bind_name.is_none() {
            self.sink.emit(errors::refinement_requires_handle_error(
                &self.type_name(refinement_ty),
                &self.type_name(expected_input_ty),
                span,
            ));
        }

        if let Some(name) = bind_name {
            if let Some(def_id) = self.declaration_def_id(name.span) {
                self.type_env.insert(def_id, TypeInterner::STRING);
            }
        }

        self.check_handle_body(body);
        self.validate_handle_terminator(body, refinement_ty);
        refinement_ty
    }

    fn check_var_decl(&mut self, decl: &ast::VarDecl) {
        let declared_type = self.resolve_type_expr(&decl.ty);
        let init_type = self.check_expr_for_expected(&decl.value, declared_type, true);

        // Bind the variable's DefId to its declared type.
        if let Some(def_id) = self.declaration_def_id(decl.name.span) {
            self.type_env.insert(def_id, declared_type);
        }

        // Check that the initializer type matches the declared type (skip if Error).
        if !self.types_compatible(declared_type, init_type) {
            self.sink.emit(errors::var_decl_type_mismatch(
                &decl.name.name,
                &self.type_name(declared_type),
                &self.type_name(init_type),
                decl.span,
            ));
        }
    }

    fn check_assign(&mut self, assign: &ast::AssignStmt) {
        let target_type = self.check_expr(&assign.target);
        let value_type = self.check_expr_for_expected(&assign.value, target_type, false);

        if !self.types_compatible(target_type, value_type) {
            self.sink.emit(errors::assign_type_mismatch(
                &self.type_name(target_type),
                &self.type_name(value_type),
                assign.span,
            ));
        }
    }

    fn check_return(&mut self, ret: &ast::ReturnStmt) {
        let ret_type = match &ret.value {
            Some(expr) => {
                if let Some(expected) = self.current_return_type {
                    self.check_expr_for_expected(expr, expected, false)
                } else {
                    self.check_expr(expr)
                }
            }
            None => TypeInterner::NOTHING,
        };

        if let Some(expected) = self.current_return_type {
            if !self.satisfies_expected_type(expected, ret_type) {
                self.sink.emit(errors::return_type_mismatch(
                    &self.type_name(expected),
                    &self.type_name(ret_type),
                    ret.span,
                ));
            }
        }
    }

    fn check_if(&mut self, if_stmt: &ast::IfStmt) {
        let cond_type = self.check_expr(&if_stmt.condition);
        if cond_type != TypeInterner::ERROR && cond_type != TypeInterner::BOOL {
            self.sink.emit(errors::condition_not_bool(
                &self.type_name(cond_type),
                if_stmt.condition.span(),
            ));
        }

        self.check_block(&if_stmt.then_block);

        for (else_if_cond, else_if_block) in &if_stmt.else_ifs {
            let ei_type = self.check_expr(else_if_cond);
            if ei_type != TypeInterner::ERROR && ei_type != TypeInterner::BOOL {
                self.sink.emit(errors::condition_not_bool(
                    &self.type_name(ei_type),
                    else_if_cond.span(),
                ));
            }
            self.check_block(else_if_block);
        }

        if let Some(else_block) = &if_stmt.else_block {
            self.check_block(else_block);
        }
    }

    fn check_for(&mut self, for_stmt: &ast::ForStmt) {
        let iterable_type = self.check_expr(&for_stmt.iterable);

        // The iterable must be list[T], string, or map[K,V].
        let resolved = self.interner.resolve(iterable_type);
        let elem_type = if iterable_type == TypeInterner::ERROR {
            TypeInterner::ERROR
        } else if let Type::List(inner) = resolved {
            *inner
        } else if iterable_type == TypeInterner::STRING {
            TypeInterner::STRING
        } else if let Type::Map(key_ty, _) = resolved {
            // Map iteration: first variable gets key type.
            *key_ty
        } else if let Type::Set(inner) = resolved {
            *inner
        } else {
            self.sink.emit(errors::not_iterable(
                &self.type_name(iterable_type),
                for_stmt.iterable.span(),
            ));
            TypeInterner::ERROR
        };

        // Bind the loop variable (key for maps, element for lists/strings).
        if let Some(def_id) = self.declaration_def_id(for_stmt.variable.span) {
            self.type_env.insert(def_id, elem_type);
        }

        // Bind the optional value variable (only for map iteration).
        if let Some(ref val_var) = for_stmt.value_variable {
            let val_type = if let Type::Map(_, val_ty) = self.interner.resolve(iterable_type) {
                *val_ty
            } else {
                TypeInterner::ERROR
            };
            if let Some(def_id) = self.declaration_def_id(val_var.span) {
                self.type_env.insert(def_id, val_type);
            }
        }

        let pushed_variant_scope =
            if let Some(owner_ty_expr) = comptime_type_variants_binding(&for_stmt.iterable) {
                let owner_ty = self.resolve_type_expr(owner_ty_expr);
                self.push_reflected_variant_type_scope(&for_stmt.variable.name, owner_ty);
                true
            } else {
                false
            };

        let pushed_field_scope = if let Some(owner_ty_expr) =
            comptime_type_fields_binding(&for_stmt.iterable)
        {
            let owner_ty = self.resolve_type_expr(owner_ty_expr);
            let field_types = if owner_ty == TypeInterner::ERROR {
                Vec::new()
            } else {
                self.reflected_field_types_for_owner(owner_ty)
            };
            self.push_reflected_field_type_scope(&for_stmt.variable.name, field_types);
            true
        } else if let Some(owner_ty_expr) = comptime_type_variant_fields_binding(&for_stmt.iterable)
        {
            let owner_ty = self.resolve_type_expr(owner_ty_expr);
            let field_types = if owner_ty == TypeInterner::ERROR {
                Vec::new()
            } else {
                self.reflected_variant_field_types_for_owner(owner_ty)
            };
            self.push_reflected_field_type_scope(&for_stmt.variable.name, field_types);
            true
        } else if let Some(variant_name) = reflected_variant_fields_binding(&for_stmt.iterable) {
            if let Some(owner_ty) = self.reflected_variant_owner_for_name(variant_name) {
                let field_types = self.reflected_variant_field_types_for_owner(owner_ty);
                self.push_reflected_field_type_scope(&for_stmt.variable.name, field_types);
                true
            } else {
                false
            }
        } else {
            false
        };

        let pushed_type_info_scope = if let Some(info_types) =
            self.reflected_type_info_arg_types_for_iterable(&for_stmt.iterable)
        {
            self.push_reflected_type_info_scope(&for_stmt.variable.name, info_types);
            true
        } else {
            false
        };

        self.check_block(&for_stmt.body);

        if pushed_type_info_scope {
            self.pop_reflected_type_info_scope();
        }
        if pushed_field_scope {
            self.pop_reflected_field_type_scope();
        }
        if pushed_variant_scope {
            self.pop_reflected_variant_type_scope();
        }
    }

    fn check_while(&mut self, while_stmt: &ast::WhileStmt) {
        let cond_type = self.check_expr(&while_stmt.condition);
        if cond_type != TypeInterner::ERROR && cond_type != TypeInterner::BOOL {
            self.sink.emit(errors::condition_not_bool(
                &self.type_name(cond_type),
                while_stmt.condition.span(),
            ));
        }
        self.check_block(&while_stmt.body);
    }

    fn check_assert(&mut self, assert_stmt: &ast::AssertStmt) {
        if !self.in_verify_block && !self.in_property_block {
            self.sink
                .emit(errors::assert_outside_test_block(assert_stmt.span));
        }
        let cond_type = self.check_expr(&assert_stmt.condition);
        if cond_type != TypeInterner::ERROR && cond_type != TypeInterner::BOOL {
            self.sink.emit(errors::assert_condition_not_bool(
                &self.type_name(cond_type),
                assert_stmt.condition.span(),
            ));
        }
        if let Some(msg) = &assert_stmt.message {
            self.check_expr(msg);
        }
    }

    fn check_match(&mut self, match_stmt: &ast::MatchStmt) {
        let expr_ty = self.check_expr(&match_stmt.expr);
        if expr_ty == TypeInterner::ERROR {
            for arm in &match_stmt.arms {
                self.check_block(&arm.body);
            }
            return;
        }

        let Type::Enum(enum_id) = *self.interner.resolve(expr_ty) else {
            self.sink.emit(errors::match_requires_enum(
                &self.type_name(expr_ty),
                match_stmt.expr.span(),
            ));
            for arm in &match_stmt.arms {
                self.check_block(&arm.body);
            }
            return;
        };

        let enum_def = self.interner.resolve_enum(enum_id).clone();
        let mut covered = HashSet::new();
        let mut has_other = false;

        for arm in &match_stmt.arms {
            match &arm.pattern {
                ast::Pattern::Ident(name) => {
                    if enum_def
                        .variants
                        .iter()
                        .any(|variant| variant.name == name.name)
                    {
                        covered.insert(name.name.clone());
                    } else {
                        self.sink.emit(errors::type_has_no_member(
                            &enum_def.name,
                            &name.name,
                            name.span,
                        ));
                    }
                }
                ast::Pattern::Variant(name, bindings) => {
                    if let Some(variant) = enum_def
                        .variants
                        .iter()
                        .find(|variant| variant.name == name.name)
                    {
                        covered.insert(name.name.clone());
                        if bindings.len() != variant.fields.len() {
                            self.sink.emit(errors::variant_binding_count_mismatch(
                                &name.name,
                                variant.fields.len(),
                                bindings.len(),
                                name.span,
                            ));
                        }

                        for (binding, (_, field_ty)) in bindings.iter().zip(variant.fields.iter()) {
                            if let Some(def_id) = self.declaration_def_id(binding.span) {
                                self.type_env.insert(def_id, *field_ty);
                            }
                        }
                    } else {
                        self.sink.emit(errors::type_has_no_member(
                            &enum_def.name,
                            &name.name,
                            name.span,
                        ));
                    }
                }
                ast::Pattern::Other(_) => {
                    has_other = true;
                }
            }

            self.check_block(&arm.body);
        }

        if !has_other {
            for variant in &enum_def.variants {
                if !covered.contains(&variant.name) {
                    self.sink.emit(errors::non_exhaustive_match(
                        &enum_def.name,
                        &variant.name,
                        match_stmt.span,
                    ));
                    break;
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    fn check_expr(&mut self, expr: &Expr) -> TypeId {
        let ty = match expr {
            Expr::IntLiteral(_, _) => TypeInterner::INT64,
            Expr::FloatLiteral(_, _) => TypeInterner::FLOAT64,
            Expr::StringLiteral(_, _) => TypeInterner::STRING,
            Expr::BoolLiteral(_, _) => TypeInterner::BOOL,
            Expr::Nothing(_) => TypeInterner::NOTHING,

            Expr::Ident(ident) => self.check_ident(ident),
            Expr::Binary(lhs, op, rhs, span) => self.check_binary(lhs, *op, rhs, *span),
            Expr::Unary(op, operand, span) => self.check_unary(*op, operand, *span),
            Expr::Call(callee, args, span) => self.check_call(callee, &[], args, *span),
            Expr::GenericCall(callee, type_args, args, span) => {
                self.check_call(callee, type_args, args, *span)
            }
            Expr::Paren(inner, _) => self.check_expr(inner),
            Expr::FieldAccess(base, field, span) => self.check_field_access(base, field, *span),
            Expr::View(inner, _) => self.check_expr(inner),

            Expr::ListConstruct(elems, _span) => self.check_list_construct(elems),
            Expr::MapConstruct(_entries, _span) => {
                // Map construction type checking is deferred.
                TypeInterner::ERROR
            }

            Expr::Handle(target, bind_name, body, span) => {
                self.check_handle(target, bind_name.as_ref(), body, *span)
            }

            Expr::Ok(inner, _span) => {
                let inner_ty = self.check_expr(inner);
                // ok(T) → result[T, <error>] — the error type is unknown without context.
                // For now, produce result[T, nothing].
                self.interner
                    .intern(Type::Result(inner_ty, TypeInterner::ERROR))
            }
            Expr::Fail(inner, _span) => {
                let inner_ty = self.check_expr(inner);
                // fail(E) → result[<error>, E]
                self.interner
                    .intern(Type::Result(TypeInterner::ERROR, inner_ty))
            }
            Expr::Some(inner, _span) => {
                let inner_ty = self.check_expr(inner);
                self.interner.intern(Type::Optional(inner_ty))
            }
            Expr::None(_) => {
                // none → optional[<error>] (unknown inner type without context)
                self.interner.intern(Type::Optional(TypeInterner::ERROR))
            }
            Expr::Default(inner, span) => {
                if self.handle_body_depth == 0 {
                    self.sink.emit(errors::default_outside_handle(*span));
                }
                self.check_expr(inner)
            }

            Expr::StringInterpolation(parts, _) => {
                // Each interpolated expression must be displayable; the overall result is string.
                for part in parts {
                    if let StringPart::Expr(expr) = part {
                        let expr_ty = self.check_expr(expr);
                        if !self.is_displayable_type(expr_ty) {
                            self.sink.emit(errors::type_does_not_implement_interface(
                                &self.type_name(expr_ty),
                                "Displayable",
                                expr.span(),
                            ));
                        }
                    }
                }
                TypeInterner::STRING
            }
            Expr::Declassify(inner, span) => {
                let inner_ty = self.check_expr(inner);
                if let Some(unwrapped) = self.secret_inner_type(inner_ty) {
                    unwrapped
                } else {
                    self.sink.emit(errors::declassify_requires_secret(
                        &self.type_name(inner_ty),
                        *span,
                    ));
                    TypeInterner::ERROR
                }
            }
            Expr::Coarsen(inner, _) => {
                let inner_ty = self.check_expr(inner);
                if inner_ty != TypeInterner::ERROR && !self.is_refinement_type(inner_ty) {
                    self.sink.emit(errors::coarsen_requires_refinement(
                        &self.type_name(inner_ty),
                        expr.span(),
                    ));
                }
                self.fully_coarsened_type(inner_ty)
            }
            Expr::Pipeline(initial, steps, _) => {
                // Check the initial expression and each step; return the type
                // of the last step (or the initial expression if there are no steps).
                let mut current_ty = self.check_expr(initial);
                for step in steps {
                    // Check the function and extra args but return Error for now
                    // since full pipeline type inference is not yet implemented.
                    self.check_expr(&step.function);
                    for arg in &step.extra_args {
                        self.check_expr(&arg.value);
                    }
                    current_ty = TypeInterner::ERROR;
                }
                current_ty
            }
            Expr::At(inner, _state, _) => {
                // `expr at state` returns a bool.
                self.check_expr(inner);
                TypeInterner::BOOL
            }
            Expr::Spawn(inner, _) => self.check_spawn(inner),
            Expr::Send(inner, _) => {
                self.check_send_ask_inner(inner);
                TypeInterner::NOTHING
            }
            Expr::Ask(inner, _) => self.check_send_ask_inner(inner),
            Expr::Clone(inner, _) => {
                // `clone expr` returns the same type as the expression.
                self.check_expr(inner)
            }
            Expr::Run(inner, _) => {
                // `run call` returns the same type as the call (pending tracked internally).
                self.check_expr(inner)
            }
            Expr::Join(inner, _) => {
                // `join task` returns result[T, string] so `handle error:` works.
                // If the task already has a result type, preserve it as-is.
                let inner_ty = self.check_expr(inner);
                match self.interner.resolve(inner_ty).clone() {
                    jett_types::Type::Result(_, _) => inner_ty,
                    _ => self
                        .interner
                        .intern(jett_types::Type::Result(inner_ty, TypeInterner::STRING)),
                }
            }
            Expr::Cancel(inner, _) => {
                // `cancel task` — checks the inner expression and returns nothing.
                self.check_expr(inner);
                TypeInterner::NOTHING
            }
            Expr::Error(_) => TypeInterner::ERROR,
            Expr::EnumVariant(type_name, variant, span) => {
                self.check_enum_variant(type_name, variant, &[], *span)
            }
            Expr::InlineFn(params, return_type, body, _) => {
                // Type-check the inline function body with parameters bound.
                let saved_return_type = self.current_return_type;
                let saved_fn_name = self.current_function_name.take();
                let saved_pure = self.current_function_pure;

                let ret = return_type
                    .as_ref()
                    .map(|t| self.resolve_type_expr(t))
                    .unwrap_or(TypeInterner::NOTHING);
                self.current_return_type = Some(ret);
                self.current_function_pure = false;

                for param in params {
                    let param_type = self.resolve_type_expr(&param.ty);
                    if let Some(def_id) = self.declaration_def_id(param.name.span) {
                        self.type_env.insert(def_id, param_type);
                    }
                }

                let param_types: Vec<_> = params
                    .iter()
                    .map(|p| self.resolve_type_expr(&p.ty))
                    .collect();

                self.check_block(body);

                self.current_return_type = saved_return_type;
                self.current_function_name = saved_fn_name;
                self.current_function_pure = saved_pure;

                self.interner.intern(Type::Function {
                    params: param_types,
                    return_type: ret,
                })
            }
        };

        // Record the type for this expression span.
        self.type_map.insert(expr.span(), ty);
        ty
    }

    fn check_spawn(&mut self, inner: &Expr) -> TypeId {
        // `spawn ActorType(args)` — the inner expr should be a call to the actor type name.
        // We check the arguments but return the actor type.
        let callee = match inner {
            Expr::Call(callee, args, _span) => {
                // Check argument expressions.
                for arg in args {
                    self.check_expr(&arg.value);
                }
                callee.as_ref()
            }
            _ => {
                self.check_expr(inner);
                return TypeInterner::ERROR;
            }
        };
        // The callee should be an actor type name.
        match callee {
            Expr::Ident(ident) => {
                if let Some(&ty) = self.named_types.get(&ident.name) {
                    if matches!(self.interner.resolve(ty), Type::Actor(_)) {
                        return ty;
                    }
                }
                self.check_expr(callee)
            }
            _ => {
                self.check_expr(callee);
                TypeInterner::ERROR
            }
        }
    }

    fn check_send_ask_inner(&mut self, inner: &Expr) -> TypeId {
        // inner is `actor_expr.handler_name` or `actor_expr.handler_name(args)`
        // We check the actor expression and any args, and return the responds type.
        let (actor_expr, message_name, args) = match inner {
            Expr::Call(callee, args, _) => match callee.as_ref() {
                Expr::FieldAccess(base, field, _) => (base.as_ref(), &field.name, Some(args)),
                _ => {
                    self.check_expr(inner);
                    return TypeInterner::ERROR;
                }
            },
            Expr::FieldAccess(base, field, _) => (base.as_ref(), &field.name, None),
            _ => {
                self.check_expr(inner);
                return TypeInterner::ERROR;
            }
        };

        let actor_ty = self.check_expr(actor_expr);
        if let Some(arg_list) = args {
            for arg in arg_list {
                self.check_expr(&arg.value);
            }
        }

        // Look up the handler and return its responds type.
        if let Type::Actor(aid) = *self.interner.resolve(actor_ty) {
            let actor_def = self.interner.resolve_actor(aid).clone();
            if let Some(msg) = actor_def.messages.iter().find(|m| m.name == *message_name) {
                return msg.responds;
            }
        }
        TypeInterner::ERROR
    }

    fn check_ident(&mut self, ident: &ast::Ident) -> TypeId {
        if let Some(&def_id) = self
            .resolve
            .resolutions
            .get(&ident.span)
            .or_else(|| self.decl_defs.get(&ident.span))
        {
            if let Some(&type_id) = self.type_env.get(&def_id) {
                return type_id;
            }
        }
        // If name resolution didn't find this ident, the resolver already
        // emitted an error. We return Error to avoid cascading type errors.
        TypeInterner::ERROR
    }

    fn is_displayable_type(&self, ty: TypeId) -> bool {
        matches!(
            self.interner.resolve(ty),
            Type::Int8
                | Type::Int16
                | Type::Int32
                | Type::Int64
                | Type::Uint8
                | Type::Uint16
                | Type::Uint32
                | Type::Uint64
                | Type::Float32
                | Type::Float64
                | Type::String
                | Type::Bool
        ) || self.type_implements_named_interface(ty, "Displayable")
    }

    fn type_implements_named_interface(&self, ty: TypeId, interface_name: &str) -> bool {
        let Some(&interface_ty) = self.named_types.get(interface_name) else {
            return false;
        };
        matches!(self.interner.resolve(interface_ty), Type::Interface(_))
            && self.interface_impls.contains_key(&(interface_ty, ty))
    }

    fn check_binary(&mut self, lhs: &Expr, op: BinOp, rhs: &Expr, span: Span) -> TypeId {
        let lhs_ty = self.check_expr(lhs);
        let rhs_ty = self.check_expr(rhs);

        // If either side is an error, propagate.
        if lhs_ty == TypeInterner::ERROR || rhs_ty == TypeInterner::ERROR {
            return TypeInterner::ERROR;
        }

        let (lhs_base, lhs_secret) = self.strip_secret_type(lhs_ty);
        let (rhs_base, rhs_secret) = self.strip_secret_type(rhs_ty);
        let tainted = lhs_secret || rhs_secret;

        match op {
            // Arithmetic operators: both sides must be the same numeric type.
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Modulo => {
                if !self.is_numeric(lhs_base) || !self.is_numeric(rhs_base) || lhs_base != rhs_base
                {
                    self.sink.emit(errors::binary_op_mismatch(
                        Self::binop_str(op),
                        &self.type_name(lhs_ty),
                        &self.type_name(rhs_ty),
                        span,
                    ));
                    return TypeInterner::ERROR;
                }
                self.maybe_wrap_secret(lhs_base, tainted)
            }

            // Comparison operators: both sides must be the same type, returns bool.
            BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => {
                if lhs_base != rhs_base {
                    self.sink.emit(errors::binary_op_mismatch(
                        Self::binop_str(op),
                        &self.type_name(lhs_ty),
                        &self.type_name(rhs_ty),
                        span,
                    ));
                    return TypeInterner::ERROR;
                }
                self.maybe_wrap_secret(TypeInterner::BOOL, tainted)
            }

            // Logical operators: both sides must be bool.
            BinOp::And | BinOp::Or => {
                if lhs_base != TypeInterner::BOOL || rhs_base != TypeInterner::BOOL {
                    self.sink.emit(errors::binary_op_mismatch(
                        Self::binop_str(op),
                        &self.type_name(lhs_ty),
                        &self.type_name(rhs_ty),
                        span,
                    ));
                    return TypeInterner::ERROR;
                }
                self.maybe_wrap_secret(TypeInterner::BOOL, tainted)
            }
        }
    }

    fn check_unary(&mut self, op: UnaryOp, operand: &Expr, span: Span) -> TypeId {
        let operand_ty = self.check_expr(operand);

        if operand_ty == TypeInterner::ERROR {
            return TypeInterner::ERROR;
        }

        let (operand_base, tainted) = self.strip_secret_type(operand_ty);

        match op {
            UnaryOp::Not => {
                if operand_base != TypeInterner::BOOL {
                    self.sink.emit(errors::unary_op_mismatch(
                        "not",
                        &self.type_name(operand_ty),
                        span,
                    ));
                    return TypeInterner::ERROR;
                }
                self.maybe_wrap_secret(TypeInterner::BOOL, tainted)
            }
            UnaryOp::Neg => {
                if !self.is_numeric(operand_base) {
                    self.sink.emit(errors::unary_op_mismatch(
                        "-",
                        &self.type_name(operand_ty),
                        span,
                    ));
                    return TypeInterner::ERROR;
                }
                self.maybe_wrap_secret(operand_base, tainted)
            }
        }
    }

    fn check_call(
        &mut self,
        callee: &Expr,
        type_args: &[TypeExpr],
        args: &[ast::CallArg],
        span: Span,
    ) -> TypeId {
        let callee_name = self.resolved_expr_name(callee);
        let callee_is_pure = callee_name
            .as_deref()
            .map(|name| {
                if Self::is_impure_builtin(name) {
                    false
                } else {
                    self.purity_map.get(name).copied().unwrap_or(true)
                }
            })
            .unwrap_or(false);
        let builtin_signature = self.builtin_signature(callee, type_args, span);

        // -- Capability / purity check --
        // Extract the callee name so we can look it up in the purity map.
        if let Some(callee_name) = callee_name.as_deref() {
            if !callee_is_pure {
                // E0500: pure function calls impure function
                if self.current_function_pure {
                    if let Some(caller_name) = &self.current_function_name {
                        self.sink
                            .emit(errors::pure_calls_impure(caller_name, &callee_name, span));
                    }
                }
                // E0501: verify block calls impure function
                if self.in_verify_block {
                    if let Some(verify_name) = &self.current_verify_name {
                        self.sink.emit(errors::verify_calls_impure(
                            verify_name,
                            &callee_name,
                            span,
                        ));
                    }
                }
            }
        }

        // Check for generic function call: `name[T](args...)`.
        if builtin_signature.is_none() && !type_args.is_empty() {
            if let Some(function_name) = callee_name.as_deref() {
                if let Some(template) = self.generic_function_templates.get(function_name).cloned()
                {
                    let concrete_args: Vec<TypeId> = type_args
                        .iter()
                        .map(|a| self.resolve_type_expr(a))
                        .collect();

                    if template.type_params.len() != concrete_args.len() {
                        self.sink.emit(errors::unknown_type(
                            &format!(
                                "{} (expected {} type argument(s), got {})",
                                function_name,
                                template.type_params.len(),
                                concrete_args.len()
                            ),
                            span,
                        ));
                        return TypeInterner::ERROR;
                    }

                    let subst: HashMap<String, TypeId> = template
                        .type_params
                        .iter()
                        .zip(concrete_args.iter())
                        .map(|(p, &ty)| (p.name.clone(), ty))
                        .collect();

                    let old_subst = std::mem::replace(&mut self.type_var_subst, subst);

                    let param_types: Vec<TypeId> = template
                        .params
                        .iter()
                        .map(|p| self.resolve_type_expr(&p.ty))
                        .collect();
                    let return_type = template
                        .return_type
                        .as_ref()
                        .map(|t| self.resolve_type_expr(t))
                        .unwrap_or(TypeInterner::NOTHING);

                    self.type_var_subst = old_subst;

                    // Check argument count and types.
                    if args.len() != param_types.len() {
                        self.sink.emit(errors::argument_count_mismatch(
                            function_name,
                            param_types.len(),
                            args.len(),
                            span,
                        ));
                        for arg in args {
                            self.check_expr(&arg.value);
                        }
                        return TypeInterner::ERROR;
                    }
                    for (arg, &expected) in args.iter().zip(param_types.iter()) {
                        let got = self.check_expr(&arg.value);
                        if !self.types_compatible(got, expected) {
                            self.sink.emit(errors::type_mismatch(
                                &self.type_name(expected),
                                &self.type_name(got),
                                arg.value.span(),
                            ));
                        }
                    }
                    return return_type;
                }
            }
        }

        // Check for generic struct construction: `Name[T, U](fields...)`.
        if !type_args.is_empty() {
            if let Some(struct_name) = callee_name.as_deref() {
                if self.generic_struct_templates.contains_key(struct_name) {
                    let concrete_args: Vec<TypeId> = type_args
                        .iter()
                        .map(|a| self.resolve_type_expr(a))
                        .collect();
                    let mono_ty = self.monomorphize_struct(struct_name, &concrete_args, span);
                    if mono_ty != TypeInterner::ERROR {
                        let sid = match self.interner.resolve(mono_ty) {
                            Type::Struct(sid) => *sid,
                            _ => return TypeInterner::ERROR,
                        };
                        return self.check_struct_constructor(sid, args, span);
                    }
                    return TypeInterner::ERROR;
                }
            }
        }

        if type_args.is_empty() {
            if let Some(type_name) = callee_name.as_deref() {
                if let Some(type_id) = self.named_types.get(type_name).copied() {
                    match self.interner.resolve(type_id).clone() {
                        Type::Struct(sid) => return self.check_struct_constructor(sid, args, span),
                        Type::Bitfield(bid) => {
                            return self.check_bitfield_constructor(bid, args, span);
                        }
                        _ => {}
                    }
                }
            }
        }

        let user_function_signature = if type_args.is_empty() {
            if builtin_signature.is_none() {
                callee_name
                    .as_deref()
                    .and_then(|name| self.function_signatures.get(name).cloned())
            } else {
                None
            }
        } else {
            None
        };

        let (param_types, return_type) = if let Some(signature) = builtin_signature {
            signature
        } else if let Some(signature) = user_function_signature {
            signature
        } else {
            let callee_ty = self.check_expr(callee);

            if callee_ty == TypeInterner::ERROR {
                // Still check argument expressions so we populate the type map.
                for arg in args {
                    self.check_expr(&arg.value);
                }
                return TypeInterner::ERROR;
            }

            // The callee must be a function type.
            match self.interner.resolve(callee_ty).clone() {
                Type::Function {
                    params,
                    return_type,
                } => (params, return_type),
                Type::Struct(sid) if self.is_struct_type_name_expr(callee) => {
                    return self.check_struct_constructor(sid, args, span);
                }
                Type::Bitfield(bid) if self.is_bitfield_type_name_expr(callee) => {
                    return self.check_bitfield_constructor(bid, args, span);
                }
                _ => {
                    self.sink
                        .emit(errors::not_callable(&self.type_name(callee_ty), span));
                    for arg in args {
                        self.check_expr(&arg.value);
                    }
                    return TypeInterner::ERROR;
                }
            }
        };

        // Check argument count.
        if args.len() != param_types.len() {
            let func_name = callee_name
                .clone()
                .unwrap_or_else(|| "<anonymous>".to_string());
            self.sink.emit(errors::argument_count_mismatch(
                &func_name,
                param_types.len(),
                args.len(),
                span,
            ));
            // Still type-check the provided arguments.
            for arg in args {
                self.check_expr(&arg.value);
            }
            return return_type;
        }

        // Check each argument type.
        let mut tainted_return = false;
        let mut checked_arg_types = Vec::with_capacity(args.len());
        for (i, arg) in args.iter().enumerate() {
            let param_ty = param_types[i];
            let arg_ty = self.check_expr_for_expected(&arg.value, param_ty, false);
            checked_arg_types.push(arg_ty);

            if self.is_refinement_type(param_ty)
                && arg_ty != TypeInterner::ERROR
                && self.can_refine_from(arg_ty, param_ty)
            {
                self.sink.emit(errors::refinement_requires_handle_error(
                    &self.type_name(param_ty),
                    &self.type_name(arg_ty),
                    arg.value.span(),
                ));
                continue;
            }

            if let Some(callee_name) = callee_name.as_deref() {
                if Self::is_secret_output_boundary(callee_name) && self.is_secret_type(arg_ty) {
                    self.sink.emit(errors::secret_exposure(
                        callee_name,
                        &self.type_name(arg_ty),
                        arg.value.span(),
                    ));
                    continue;
                }

                if matches!(callee_name, "secret.redact" | "secret.compare")
                    && !self.is_secret_type(arg_ty)
                {
                    self.sink.emit(errors::secret_operation_requires_secret(
                        callee_name,
                        &self.type_name(arg_ty),
                        arg.value.span(),
                    ));
                    continue;
                }
            }

            let (matches, lifted_secret) = self.secret_argument_matches_param(param_ty, arg_ty);
            if matches {
                if lifted_secret {
                    let allows_secret_lifting = callee_name
                        .as_deref()
                        .map(|name| Self::is_secret_liftable_call(name, callee_is_pure))
                        .unwrap_or(callee_is_pure);

                    if !allows_secret_lifting {
                        self.sink.emit(errors::secret_exposure(
                            callee_name.as_deref().unwrap_or("<call>"),
                            &self.type_name(arg_ty),
                            arg.value.span(),
                        ));
                        continue;
                    }
                    tainted_return = true;
                }
                continue;
            }

            if !self.types_compatible(param_ty, arg_ty) {
                let param_name = format!("#{}", i + 1);
                self.sink.emit(errors::argument_type_mismatch(
                    &param_name,
                    &self.type_name(param_ty),
                    &self.type_name(arg_ty),
                    arg.value.span(),
                ));
            }
        }

        if matches!(callee_name.as_deref(), Some("secret.compare")) && checked_arg_types.len() == 2
        {
            if let (Some(lhs_inner), Some(rhs_inner)) = (
                self.secret_inner_type(checked_arg_types[0]),
                self.secret_inner_type(checked_arg_types[1]),
            ) {
                if !self.types_compatible(lhs_inner, rhs_inner)
                    || !self.types_compatible(rhs_inner, lhs_inner)
                {
                    self.sink.emit(errors::argument_type_mismatch(
                        "#2",
                        &self.type_name(checked_arg_types[0]),
                        &self.type_name(checked_arg_types[1]),
                        args[1].value.span(),
                    ));
                }
            }
        }

        self.check_json_public_call_policy(
            callee_name.as_deref(),
            &checked_arg_types,
            args,
            return_type,
        );

        if let Some(callee_name) = callee_name.as_deref() {
            if tainted_return && Self::is_secret_liftable_call(callee_name, callee_is_pure) {
                return self.maybe_wrap_secret(return_type, true);
            }
        }

        return_type
    }

    fn check_field_access(&mut self, base: &Expr, field: &ast::Ident, span: Span) -> TypeId {
        if let Expr::Ident(base_ident) = base {
            if self.ident_def_kind(base_ident) == Some(DefKind::Enum) {
                return self.check_enum_variant(base_ident, field, &[], span);
            }
            if self.ident_def_kind(base_ident) == Some(DefKind::Interface) {
                let base_ty = self.check_ident(base_ident);
                if let Type::Interface(iid) = *self.interner.resolve(base_ty) {
                    let interface_def = self.interner.resolve_interface(iid);
                    if let Some(method) = interface_def
                        .methods
                        .iter()
                        .find(|m| m.name == field.name)
                        .cloned()
                    {
                        let params = method.params.iter().map(|(_, ty, _)| *ty).collect();
                        return self.interner.intern(Type::Function {
                            params,
                            return_type: method.return_type,
                        });
                    }

                    self.sink.emit(errors::interface_has_no_member(
                        &interface_def.name,
                        &field.name,
                        span,
                    ));
                    return TypeInterner::ERROR;
                }
            }
            if self.ident_def_kind(base_ident) == Some(DefKind::Struct) {
                let base_ty = self.check_ident(base_ident);
                if let Some(method_ty) = self.check_type_module_method(base_ty, field, span) {
                    return method_ty;
                }
            }
            if self.ident_def_kind(base_ident) == Some(DefKind::Bitfield) {
                let base_ty = self.check_ident(base_ident);
                if let Some(method_ty) = self.check_type_module_method(base_ty, field, span) {
                    return method_ty;
                }
            }
            if let Some(type_id) = self.named_types.get(&base_ident.name).copied() {
                if matches!(self.interner.resolve(type_id), Type::Enum(_)) {
                    return self.check_enum_variant(base_ident, field, &[], span);
                }
                if let Some(method_ty) = self.check_type_module_method(type_id, field, span) {
                    return method_ty;
                }
            }
        }

        if !matches!(base, Expr::Ident(_)) {
            if let Some(type_name) = Self::extract_dotted_name(base) {
                let type_name = self.resolved_or_expanded_name(&type_name, span);
                if let Some(type_id) = self.named_types.get(&type_name).copied() {
                    if matches!(self.interner.resolve(type_id), Type::Enum(_)) {
                        return self.check_enum_variant_by_type(type_id, field, &[], span);
                    }
                    if let Some(method_ty) = self.check_type_module_method(type_id, field, span) {
                        return method_ty;
                    }
                }
            }
        }

        let base_ty = self.check_expr(base);
        if base_ty == TypeInterner::ERROR {
            return TypeInterner::ERROR;
        }

        match self.interner.resolve(base_ty) {
            Type::Secret(inner) => match self.interner.resolve(*inner) {
                Type::Struct(sid) => {
                    let struct_def = self.interner.resolve_struct(*sid);
                    if let Some((_, field_ty)) = struct_def
                        .fields
                        .iter()
                        .find(|(name, _)| name == &field.name)
                    {
                        self.maybe_wrap_secret(*field_ty, true)
                    } else {
                        self.sink.emit(errors::type_has_no_member(
                            &format!("secret[{}]", struct_def.name),
                            &field.name,
                            span,
                        ));
                        TypeInterner::ERROR
                    }
                }
                Type::Bitfield(bid) => {
                    let bitfield_def = self.interner.resolve_bitfield(*bid);
                    if let Some(field_def) = bitfield_def
                        .fields
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                    {
                        self.maybe_wrap_secret(field_def.ty, true)
                    } else {
                        self.sink.emit(errors::type_has_no_member(
                            &format!("secret[{}]", bitfield_def.name),
                            &field.name,
                            span,
                        ));
                        TypeInterner::ERROR
                    }
                }
                _ => {
                    self.sink.emit(errors::type_has_no_member(
                        &self.type_name(base_ty),
                        &field.name,
                        span,
                    ));
                    TypeInterner::ERROR
                }
            },
            Type::Struct(sid) => {
                let struct_def = self.interner.resolve_struct(*sid);
                if let Some((_, field_ty)) = struct_def
                    .fields
                    .iter()
                    .find(|(name, _)| name == &field.name)
                {
                    *field_ty
                } else {
                    self.sink.emit(errors::type_has_no_member(
                        &struct_def.name,
                        &field.name,
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            Type::Bitfield(bid) => {
                let bitfield_def = self.interner.resolve_bitfield(*bid);
                if let Some(field_def) = bitfield_def
                    .fields
                    .iter()
                    .find(|candidate| candidate.name == field.name)
                {
                    field_def.ty
                } else {
                    self.sink.emit(errors::type_has_no_member(
                        &bitfield_def.name,
                        &field.name,
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            _ => {
                self.sink.emit(errors::type_has_no_member(
                    &self.type_name(base_ty),
                    &field.name,
                    span,
                ));
                TypeInterner::ERROR
            }
        }
    }

    fn check_bitfield_literal_range(
        &mut self,
        bitfield_name: &str,
        field_name: &str,
        width: u16,
        value: i64,
        span: Span,
    ) {
        let max_value = if width >= 63 {
            i64::MAX
        } else {
            (1_i64 << width) - 1
        };

        if value < 0 || value > max_value {
            self.sink.emit(errors::bitfield_literal_out_of_range(
                bitfield_name,
                field_name,
                width,
                value,
                span,
            ));
        }
    }

    fn check_type_module_method(
        &mut self,
        type_id: TypeId,
        field: &ast::Ident,
        span: Span,
    ) -> Option<TypeId> {
        if let Type::Struct(sid) = *self.interner.resolve(type_id) {
            let struct_def = self.interner.resolve_struct(sid);
            if let Some(method) = struct_def
                .methods
                .iter()
                .find(|m| m.name == field.name)
                .cloned()
            {
                let params = method.params.iter().map(|(_, ty, _)| *ty).collect();
                return Some(self.interner.intern(Type::Function {
                    params,
                    return_type: method.return_type,
                }));
            }
        }

        if matches!(self.interner.resolve(type_id), Type::Bitfield(_)) {
            match field.name.as_str() {
                "to_bytes" => {
                    return Some(self.interner.intern(Type::Function {
                        params: vec![type_id],
                        return_type: TypeInterner::BYTES,
                    }));
                }
                "from_bytes" => {
                    let result_ty = self
                        .interner
                        .intern(Type::Result(type_id, TypeInterner::STRING));
                    return Some(self.interner.intern(Type::Function {
                        params: vec![TypeInterner::BYTES],
                        return_type: result_ty,
                    }));
                }
                _ => {}
            }
        }

        if let Some(method) = self
            .impl_methods_by_type
            .get(&type_id)
            .and_then(|methods| methods.get(&field.name))
            .cloned()
        {
            let params = method.params.iter().map(|(_, ty, _)| *ty).collect();
            return Some(self.interner.intern(Type::Function {
                params,
                return_type: method.return_type,
            }));
        }

        self.sink.emit(errors::type_has_no_member(
            &self.type_name(type_id),
            &field.name,
            span,
        ));
        None
    }

    fn check_enum_variant(
        &mut self,
        type_name: &ast::Ident,
        variant: &ast::Ident,
        args: &[ast::CallArg],
        span: Span,
    ) -> TypeId {
        let enum_ty = self.check_ident(type_name);
        if enum_ty == TypeInterner::ERROR {
            return TypeInterner::ERROR;
        }
        self.check_enum_variant_by_type(enum_ty, variant, args, span)
    }

    fn check_enum_variant_by_type(
        &mut self,
        enum_ty: TypeId,
        variant: &ast::Ident,
        args: &[ast::CallArg],
        span: Span,
    ) -> TypeId {
        let Type::Enum(eid) = *self.interner.resolve(enum_ty) else {
            self.sink.emit(errors::type_has_no_member(
                &self.type_name(enum_ty),
                &variant.name,
                span,
            ));
            return TypeInterner::ERROR;
        };

        let enum_def = self.interner.resolve_enum(eid).clone();
        let Some(variant_def) = enum_def
            .variants
            .iter()
            .find(|candidate| candidate.name == variant.name)
            .cloned()
        else {
            self.sink.emit(errors::type_has_no_member(
                &enum_def.name,
                &variant.name,
                span,
            ));
            return TypeInterner::ERROR;
        };

        if args.is_empty() {
            if variant_def.fields.is_empty() {
                return enum_ty;
            }

            let params = variant_def.fields.iter().map(|(_, ty)| *ty).collect();
            return self.interner.intern(Type::Function {
                params,
                return_type: enum_ty,
            });
        }

        if args.len() != variant_def.fields.len() {
            self.sink.emit(errors::argument_count_mismatch(
                &format!("{}.{}", enum_def.name, variant_def.name),
                variant_def.fields.len(),
                args.len(),
                span,
            ));
        }

        for (index, arg) in args.iter().enumerate() {
            if let Some((field_name, expected_ty)) = variant_def.fields.get(index) {
                let arg_ty = self.check_expr_for_expected(&arg.value, *expected_ty, false);
                if !self.types_compatible(*expected_ty, arg_ty) {
                    self.sink.emit(errors::argument_type_mismatch(
                        field_name,
                        &self.type_name(*expected_ty),
                        &self.type_name(arg_ty),
                        arg.value.span(),
                    ));
                }
            }
        }

        enum_ty
    }

    fn check_struct_constructor(
        &mut self,
        sid: StructId,
        args: &[ast::CallArg],
        span: Span,
    ) -> TypeId {
        let struct_def = self.interner.resolve_struct(sid).clone();
        let mut assigned = vec![false; struct_def.fields.len()];
        let validates_refinements = struct_def
            .fields
            .iter()
            .any(|(_, ty)| self.is_refinement_type(*ty));

        for arg in args {
            let Some(field_index) = (match &arg.name {
                Some(name) => struct_def
                    .fields
                    .iter()
                    .position(|(field_name, _)| field_name == &name.name),
                None => assigned.iter().position(|filled| !filled),
            }) else {
                if let Some(name) = &arg.name {
                    self.sink.emit(errors::type_has_no_member(
                        &struct_def.name,
                        &name.name,
                        arg.span,
                    ));
                } else {
                    self.sink.emit(errors::argument_count_mismatch(
                        &struct_def.name,
                        struct_def.fields.len(),
                        args.len(),
                        span,
                    ));
                }
                self.check_expr(&arg.value);
                continue;
            };

            if assigned[field_index] {
                self.sink.emit(errors::duplicate_constructor_field(
                    &struct_def.name,
                    &struct_def.fields[field_index].0,
                    arg.span,
                ));
                self.check_expr(&arg.value);
                continue;
            }

            assigned[field_index] = true;
            let expected_ty = struct_def.fields[field_index].1;
            let arg_ty = if self.is_refinement_type(expected_ty) {
                match &arg.value {
                    Expr::Handle(_, _, _, _) => {
                        self.check_expr_for_expected(&arg.value, expected_ty, true)
                    }
                    _ => self.check_expr(&arg.value),
                }
            } else {
                self.check_expr_for_expected(&arg.value, expected_ty, false)
            };
            if self.is_refinement_type(expected_ty) && self.can_refine_from(arg_ty, expected_ty) {
                continue;
            }
            if !self.types_compatible(expected_ty, arg_ty) {
                self.sink.emit(errors::argument_type_mismatch(
                    &struct_def.fields[field_index].0,
                    &self.type_name(expected_ty),
                    &self.type_name(arg_ty),
                    arg.value.span(),
                ));
            }
        }

        for (index, (field_name, _)) in struct_def.fields.iter().enumerate() {
            if !assigned[index] {
                self.sink.emit(errors::missing_constructor_field(
                    &struct_def.name,
                    field_name,
                    span,
                ));
            }
        }

        let struct_ty = self.interner.intern(Type::Struct(sid));
        if validates_refinements {
            self.interner
                .intern(Type::Result(struct_ty, TypeInterner::STRING))
        } else {
            struct_ty
        }
    }

    fn check_bitfield_constructor(
        &mut self,
        bid: BitfieldId,
        args: &[ast::CallArg],
        span: Span,
    ) -> TypeId {
        let bitfield_def = self.interner.resolve_bitfield(bid).clone();
        let mut assigned = vec![false; bitfield_def.fields.len()];
        let mut requires_runtime_validation = false;

        for arg in args {
            let Some(field_index) = (match &arg.name {
                Some(name) => bitfield_def
                    .fields
                    .iter()
                    .position(|field| field.name == name.name),
                None => assigned.iter().position(|filled| !filled),
            }) else {
                if let Some(name) = &arg.name {
                    self.sink.emit(errors::type_has_no_member(
                        &bitfield_def.name,
                        &name.name,
                        arg.span,
                    ));
                } else {
                    self.sink.emit(errors::argument_count_mismatch(
                        &bitfield_def.name,
                        bitfield_def.fields.len(),
                        args.len(),
                        span,
                    ));
                }
                self.check_expr(&arg.value);
                continue;
            };

            if assigned[field_index] {
                self.sink.emit(errors::duplicate_constructor_field(
                    &bitfield_def.name,
                    &bitfield_def.fields[field_index].name,
                    arg.span,
                ));
                self.check_expr(&arg.value);
                continue;
            }

            assigned[field_index] = true;
            let field_def = &bitfield_def.fields[field_index];
            let arg_ty = self.check_expr_for_expected(&arg.value, field_def.ty, false);
            if !self.types_compatible(field_def.ty, arg_ty) {
                self.sink.emit(errors::argument_type_mismatch(
                    &field_def.name,
                    &self.type_name(field_def.ty),
                    &self.type_name(arg_ty),
                    arg.value.span(),
                ));
                continue;
            }

            if let TypeBitfieldFieldKind::Bits { width } = field_def.kind {
                if field_def.ty == TypeInterner::INT64 {
                    match &arg.value {
                        Expr::IntLiteral(value, literal_span) => self.check_bitfield_literal_range(
                            &bitfield_def.name,
                            &field_def.name,
                            width,
                            *value,
                            *literal_span,
                        ),
                        _ => {
                            requires_runtime_validation = true;
                        }
                    }
                }
            }
        }

        for (index, field_def) in bitfield_def.fields.iter().enumerate() {
            if !assigned[index] {
                self.sink.emit(errors::missing_constructor_field(
                    &bitfield_def.name,
                    &field_def.name,
                    span,
                ));
            }
        }

        let bitfield_ty = self.interner.intern(Type::Bitfield(bid));
        if requires_runtime_validation {
            self.interner
                .intern(Type::Result(bitfield_ty, TypeInterner::STRING))
        } else {
            bitfield_ty
        }
    }

    fn check_list_construct(&mut self, elems: &[Expr]) -> TypeId {
        if elems.is_empty() {
            // Empty list: list[<error>] since we can't infer the element type.
            return self.interner.intern(Type::List(TypeInterner::ERROR));
        }

        let first_ty = self.check_expr(&elems[0]);
        let (element_ty, mut tainted) = self.strip_secret_type(first_ty);
        for elem in &elems[1..] {
            let elem_ty = self.check_expr(elem);
            let (elem_base_ty, elem_secret) = self.strip_secret_type(elem_ty);
            if !self.types_compatible(element_ty, elem_base_ty)
                && !self.types_compatible(elem_base_ty, element_ty)
            {
                self.sink.emit(errors::type_mismatch(
                    &self.type_name(element_ty),
                    &self.type_name(elem_ty),
                    elem.span(),
                ));
            }
            tainted |= elem_secret;
        }

        let element_ty = self.maybe_wrap_secret(element_ty, tainted);
        self.interner.intern(Type::List(element_ty))
    }

    fn check_handle(
        &mut self,
        target: &Expr,
        bind_name: Option<&ast::Ident>,
        body: &Block,
        span: Span,
    ) -> TypeId {
        let target_ty = self.check_expr(target);
        self.check_handle_with_target_type(target_ty, bind_name, body, span)
    }

    fn check_handle_with_target_type(
        &mut self,
        target_ty: TypeId,
        bind_name: Option<&ast::Ident>,
        body: &Block,
        span: Span,
    ) -> TypeId {
        if target_ty == TypeInterner::ERROR {
            self.check_handle_body(body);
            self.validate_handle_terminator(body, TypeInterner::ERROR);
            return TypeInterner::ERROR;
        }

        match self.interner.resolve(target_ty).clone() {
            Type::Result(ok_ty, err_ty) => {
                if bind_name.is_none() {
                    self.sink.emit(errors::result_requires_handle_error(span));
                }
                if let Some(name) = bind_name {
                    if let Some(def_id) = self.declaration_def_id(name.span) {
                        self.type_env.insert(def_id, err_ty);
                    }
                }
                self.check_handle_body(body);
                self.validate_handle_terminator(body, ok_ty);
                ok_ty
            }
            Type::Optional(inner_ty) => {
                if bind_name.is_some() {
                    self.sink.emit(errors::optional_requires_bare_handle(span));
                }
                self.check_handle_body(body);
                self.validate_handle_terminator(body, inner_ty);
                inner_ty
            }
            _ => {
                self.sink.emit(errors::handle_requires_result_or_optional(
                    &self.type_name(target_ty),
                    span,
                ));
                self.check_handle_body(body);
                self.validate_handle_terminator(body, TypeInterner::ERROR);
                TypeInterner::ERROR
            }
        }
    }

    fn check_handle_body(&mut self, body: &Block) {
        self.handle_body_depth += 1;
        self.check_block(body);
        self.handle_body_depth -= 1;
    }

    fn validate_handle_terminator(&mut self, body: &Block, success_ty: TypeId) {
        let Some(last_stmt) = body.stmts.last() else {
            self.sink
                .emit(errors::handle_block_requires_return_or_default(body.span));
            return;
        };

        match last_stmt {
            Stmt::Return(_) => {}
            Stmt::Expr(expr_stmt) => {
                if matches!(expr_stmt.expr, Expr::Default(_, _)) {
                    if success_ty != TypeInterner::ERROR {
                        let default_ty = self
                            .type_map
                            .get(&expr_stmt.expr.span())
                            .copied()
                            .unwrap_or(TypeInterner::ERROR);
                        if !self.types_compatible(success_ty, default_ty) {
                            self.sink.emit(errors::type_mismatch(
                                &self.type_name(success_ty),
                                &self.type_name(default_ty),
                                expr_stmt.expr.span(),
                            ));
                        }
                    }
                } else {
                    self.sink
                        .emit(errors::handle_block_requires_return_or_default(
                            expr_stmt.span,
                        ));
                }
            }
            _ => self
                .sink
                .emit(errors::handle_block_requires_return_or_default(stmt_span(
                    last_stmt,
                ))),
        }
    }

    // ------------------------------------------------------------------
    // Type expression resolution (AST TypeExpr → TypeId)
    // ------------------------------------------------------------------

    pub fn resolve_type_expr(&mut self, type_expr: &TypeExpr) -> TypeId {
        match type_expr {
            TypeExpr::Named(ident) => self.resolve_named_type(&ident.name, ident.span),
            TypeExpr::Generic(name, args, _span) => {
                self.resolve_generic_type(&name.name, args, name.span)
            }
            TypeExpr::View(inner, _span) => {
                // View types are transparent for type checking purposes.
                self.resolve_type_expr(inner)
            }
            TypeExpr::Function(param_types, return_type, _span) => {
                let params = param_types
                    .iter()
                    .map(|t| self.resolve_type_expr(t))
                    .collect();
                let ret = self.resolve_type_expr(return_type);
                self.interner.intern(Type::Function {
                    params,
                    return_type: ret,
                })
            }
        }
    }

    fn resolve_named_type(&mut self, name: &str, span: Span) -> TypeId {
        // Type variable substitution takes priority (active during monomorphization).
        if let Some(&ty) = self.type_var_subst.get(name) {
            return ty;
        }
        let lookup_name = self.resolved_or_expanded_name(name, span);
        match name {
            "int8" => TypeInterner::INT8,
            "int16" => TypeInterner::INT16,
            "int32" => TypeInterner::INT32,
            "int64" => TypeInterner::INT64,
            "uint8" => TypeInterner::UINT8,
            "uint16" => TypeInterner::UINT16,
            "uint32" => TypeInterner::UINT32,
            "uint64" => TypeInterner::UINT64,
            "float32" => TypeInterner::FLOAT32,
            "float64" => TypeInterner::FLOAT64,
            "string" => TypeInterner::STRING,
            "bool" => TypeInterner::BOOL,
            "bytes" => TypeInterner::BYTES,
            "nothing" => TypeInterner::NOTHING,
            "JsonValue" => TypeInterner::JSON_VALUE,
            "TypeConstruction" => TypeInterner::TYPE_CONSTRUCTION,
            _ if self.named_types.contains_key(&lookup_name) => self.named_types[&lookup_name],
            _ if self.type_aliases.contains_key(&lookup_name) => {
                self.resolve_type_alias(&lookup_name, span)
            }
            _ if self.named_types.contains_key(name) => self.named_types[name],
            _ if self.type_aliases.contains_key(name) => self.resolve_type_alias(name, span),
            // Capability types are recognised but opaque — no further type
            // checking is performed on values of these types.
            _ if capability::is_capability_type(name) => TypeInterner::ERROR,
            _ => {
                self.sink.emit(errors::unknown_type(name, span));
                TypeInterner::ERROR
            }
        }
    }

    fn resolve_type_alias(&mut self, name: &str, span: Span) -> TypeId {
        if let Some(&ty) = self.named_types.get(name) {
            return ty;
        }

        if !self.resolving_type_aliases.insert(name.to_string()) {
            self.sink.emit(errors::unknown_type(name, span));
            return TypeInterner::ERROR;
        }

        let alias = self
            .type_aliases
            .get(name)
            .cloned()
            .expect("type alias existence checked before resolution");
        let base_ty = self.resolve_type_expr(&alias.base_type);
        let alias_ty = if alias.constraint.is_some() {
            self.interner.intern(Type::Refinement {
                name: name.to_string(),
                base: base_ty,
            })
        } else {
            base_ty
        };

        self.named_types.insert(name.to_string(), alias_ty);
        self.resolving_type_aliases.remove(name);
        alias_ty
    }

    fn resolve_generic_type(&mut self, name: &str, args: &[TypeExpr], span: Span) -> TypeId {
        let lookup_name = self.resolved_or_expanded_name(name, span);
        match name {
            "list" => {
                if args.len() == 1 {
                    let inner = self.resolve_type_expr(&args[0]);
                    self.interner.intern(Type::List(inner))
                } else {
                    self.sink.emit(errors::unknown_type(
                        &format!("list (expected 1 type argument, got {})", args.len()),
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            "map" => {
                if args.len() == 2 {
                    let key = self.resolve_type_expr(&args[0]);
                    let val = self.resolve_type_expr(&args[1]);
                    self.interner.intern(Type::Map(key, val))
                } else {
                    self.sink.emit(errors::unknown_type(
                        &format!("map (expected 2 type arguments, got {})", args.len()),
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            "set" => {
                if args.len() == 1 {
                    let inner = self.resolve_type_expr(&args[0]);
                    self.interner.intern(Type::Set(inner))
                } else {
                    self.sink.emit(errors::unknown_type(
                        &format!("set (expected 1 type argument, got {})", args.len()),
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            "optional" => {
                if args.len() == 1 {
                    let inner = self.resolve_type_expr(&args[0]);
                    self.interner.intern(Type::Optional(inner))
                } else {
                    self.sink.emit(errors::unknown_type(
                        &format!("optional (expected 1 type argument, got {})", args.len()),
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            "result" => {
                if args.len() == 2 {
                    let ok = self.resolve_type_expr(&args[0]);
                    let err = self.resolve_type_expr(&args[1]);
                    self.interner.intern(Type::Result(ok, err))
                } else {
                    self.sink.emit(errors::unknown_type(
                        &format!("result (expected 2 type arguments, got {})", args.len()),
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            "secret" => {
                if args.len() == 1 {
                    let inner = self.resolve_type_expr(&args[0]);
                    self.interner.intern(Type::Secret(inner))
                } else {
                    self.sink.emit(errors::unknown_type(
                        &format!("secret (expected 1 type argument, got {})", args.len()),
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            _ => {
                // Check if this is a user-defined generic struct.
                if self.generic_struct_templates.contains_key(&lookup_name) {
                    let concrete_args: Vec<TypeId> =
                        args.iter().map(|a| self.resolve_type_expr(a)).collect();
                    return self.monomorphize_struct(&lookup_name, &concrete_args, span);
                }
                if self.generic_struct_templates.contains_key(name) {
                    let concrete_args: Vec<TypeId> =
                        args.iter().map(|a| self.resolve_type_expr(a)).collect();
                    return self.monomorphize_struct(name, &concrete_args, span);
                }
                self.sink.emit(errors::unknown_type(name, span));
                TypeInterner::ERROR
            }
        }
    }

    /// Monomorphize a generic struct with the given concrete type arguments.
    ///
    /// Returns the `TypeId` of the resulting `Type::Struct`, creating a new
    /// `StructId` on first use and caching it for subsequent calls with the
    /// same `(name, type_args)` key.
    fn monomorphize_struct(&mut self, name: &str, type_args: &[TypeId], span: Span) -> TypeId {
        // Check the cache first.
        let cache_key = (name.to_string(), type_args.to_vec());
        if let Some(&cached) = self.monomorphized_structs.get(&cache_key) {
            return cached;
        }

        let template = match self.generic_struct_templates.get(name).cloned() {
            Some(t) => t,
            None => return TypeInterner::ERROR,
        };

        if template.type_params.len() != type_args.len() {
            self.sink.emit(errors::unknown_type(
                &format!(
                    "{} (expected {} type argument(s), got {})",
                    name,
                    template.type_params.len(),
                    type_args.len()
                ),
                span,
            ));
            return TypeInterner::ERROR;
        }

        // Build substitution map: type param name → concrete TypeId.
        let substitution: HashMap<String, TypeId> = template
            .type_params
            .iter()
            .zip(type_args.iter())
            .map(|(param, &ty)| (param.name.clone(), ty))
            .collect();

        // Install the substitution, resolve fields, then restore.
        let old_subst = std::mem::replace(&mut self.type_var_subst, substitution);

        let fields: Vec<(String, TypeId)> = template
            .fields
            .iter()
            .map(|field| {
                let field_ty = self.resolve_type_expr(&field.ty);
                (field.name.name.clone(), field_ty)
            })
            .collect();

        let methods: Vec<FunctionSig> = template
            .methods
            .iter()
            .map(|method| self.method_signature(method))
            .collect();

        self.type_var_subst = old_subst;

        // Build a mangled name, e.g. "Pair[int64, string]".
        let type_arg_names: Vec<String> = type_args.iter().map(|&ty| self.type_name(ty)).collect();
        let mono_name = format!("{}[{}]", name, type_arg_names.join(", "));
        let reflection_fields = self.reflection_fields_for_resolved_struct(&template, &fields);

        let sid = self.interner.add_struct(TypeStructDef {
            name: mono_name.clone(),
            fields,
            methods,
        });
        let ty = self.interner.intern(Type::Struct(sid));

        self.reflection_fields
            .insert(mono_name.clone(), reflection_fields.clone());
        self.reflection_fields_by_id
            .insert(ty, (mono_name, reflection_fields));
        self.monomorphized_structs.insert(cache_key, ty);
        ty
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn binop_str(op: BinOp) -> &'static str {
        match op {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Modulo => "modulo",
            BinOp::Eq => "==",
            BinOp::NotEq => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::LtEq => "<=",
            BinOp::GtEq => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }
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
        Stmt::Respond(r) => r.span,
        Stmt::Break(span) | Stmt::Continue(span) => *span,
    }
}

fn comptime_type_info_binding(expr: &Expr) -> Option<&TypeExpr> {
    let Expr::GenericCall(callee, type_args, args, _) = expr else {
        return None;
    };
    if type_args.len() != 1 || !args.is_empty() || !is_type_info_callee(callee) {
        return None;
    }
    type_args.first()
}

fn comptime_type_arg_binding(expr: &Expr) -> Option<(&TypeExpr, usize)> {
    let Expr::GenericCall(callee, type_args, args, _) = expr else {
        return None;
    };
    if type_args.len() != 1 || args.len() != 1 || !is_type_arg_callee(callee) {
        return None;
    }
    let arg = args.first()?;
    if arg.name.is_some() {
        return None;
    }
    let Expr::IntLiteral(index, _) = &arg.value else {
        return None;
    };
    Some((type_args.first()?, usize::try_from(*index).ok()?))
}

fn comptime_type_fields_binding(expr: &Expr) -> Option<&TypeExpr> {
    let Expr::GenericCall(callee, type_args, args, _) = expr else {
        return None;
    };
    if type_args.len() != 1 || !args.is_empty() || !is_type_fields_callee(callee) {
        return None;
    }
    type_args.first()
}

fn comptime_type_variants_binding(expr: &Expr) -> Option<&TypeExpr> {
    let Expr::GenericCall(callee, type_args, args, _) = expr else {
        return None;
    };
    if type_args.len() != 1 || !args.is_empty() || !is_type_variants_callee(callee) {
        return None;
    }
    type_args.first()
}

fn comptime_type_variant_value_binding(expr: &Expr) -> Option<&TypeExpr> {
    let Expr::GenericCall(callee, type_args, args, _) = expr else {
        return None;
    };
    if type_args.len() != 1 || args.len() != 1 || !is_type_variant_value_callee(callee) {
        return None;
    }
    type_args.first()
}

fn comptime_type_variant_fields_binding(expr: &Expr) -> Option<&TypeExpr> {
    let Expr::FieldAccess(base, field, _) = expr else {
        return None;
    };
    if field.name != "fields" {
        return None;
    }
    comptime_type_variant_value_binding(base)
}

fn reflected_variant_fields_binding(expr: &Expr) -> Option<&str> {
    let Expr::FieldAccess(base, field, _) = expr else {
        return None;
    };
    if field.name != "fields" {
        return None;
    }
    let Expr::Ident(ident) = base.as_ref() else {
        return None;
    };
    Some(&ident.name)
}

fn reflected_field_type_info_binding(expr: &Expr) -> Option<&str> {
    let Expr::FieldAccess(base, field, _) = expr else {
        return None;
    };
    if field.name != "type_info" {
        return None;
    }
    let Expr::Ident(ident) = base.as_ref() else {
        return None;
    };
    Some(&ident.name)
}

enum ReflectedTypeInfoSource<'a> {
    Direct(&'a TypeExpr),
    Field(&'a str),
    TypeInfo(&'a str),
}

fn reflected_type_info_args_source(expr: &Expr) -> Option<ReflectedTypeInfoSource<'_>> {
    let Expr::FieldAccess(base, field, _) = expr else {
        return None;
    };
    if field.name != "args" {
        return None;
    }
    if let Some(ty) = comptime_type_info_binding(base) {
        return Some(ReflectedTypeInfoSource::Direct(ty));
    }
    if let Some(field_name) = reflected_field_type_info_binding(base) {
        return Some(ReflectedTypeInfoSource::Field(field_name));
    }
    reflected_type_info_binding(base).map(ReflectedTypeInfoSource::TypeInfo)
}

fn reflected_type_info_binding(expr: &Expr) -> Option<&str> {
    let Expr::Ident(ident) = expr else {
        return None;
    };
    Some(&ident.name)
}

fn is_type_info_callee(callee: &Expr) -> bool {
    let Expr::FieldAccess(base, field, _) = callee else {
        return false;
    };
    field.name == "info" && matches!(base.as_ref(), Expr::Ident(ident) if ident.name == "type")
}

fn is_type_arg_callee(callee: &Expr) -> bool {
    let Expr::FieldAccess(base, field, _) = callee else {
        return false;
    };
    field.name == "arg" && matches!(base.as_ref(), Expr::Ident(ident) if ident.name == "type")
}

fn is_type_fields_callee(callee: &Expr) -> bool {
    let Expr::FieldAccess(base, field, _) = callee else {
        return false;
    };
    field.name == "fields" && matches!(base.as_ref(), Expr::Ident(ident) if ident.name == "type")
}

fn is_type_variants_callee(callee: &Expr) -> bool {
    let Expr::FieldAccess(base, field, _) = callee else {
        return false;
    };
    field.name == "variants" && matches!(base.as_ref(), Expr::Ident(ident) if ident.name == "type")
}

fn is_type_variant_value_callee(callee: &Expr) -> bool {
    let Expr::FieldAccess(base, field, _) = callee else {
        return false;
    };
    field.name == "variant_value"
        && matches!(base.as_ref(), Expr::Ident(ident) if ident.name == "type")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::{FileId, Span};
    use jett_parser::{ast::*, parse};
    use jett_resolve::resolver::ResolveResult;
    use jett_resolve::scope::{DefKind, ScopeTable};

    /// Helper to create a span for tests.
    fn sp(start: u32, end: u32) -> Span {
        Span::new(FileId::new(0), start, end)
    }

    /// Helper: build a resolve result manually for testing.
    struct TestEnv {
        scope_table: ScopeTable,
        resolutions: HashMap<Span, DefId>,
    }

    impl TestEnv {
        fn new() -> Self {
            Self {
                scope_table: ScopeTable::new(),
                resolutions: HashMap::new(),
            }
        }

        fn def_var(&mut self, name: &str, span: Span) -> DefId {
            let def_id = self
                .scope_table
                .new_def(name.to_string(), DefKind::Variable, span);
            self.resolutions.insert(span, def_id);
            def_id
        }

        fn def_param(&mut self, name: &str, span: Span) -> DefId {
            let def_id = self
                .scope_table
                .new_def(name.to_string(), DefKind::Param, span);
            self.resolutions.insert(span, def_id);
            def_id
        }

        fn def_func(&mut self, name: &str, span: Span) -> DefId {
            let def_id = self
                .scope_table
                .new_def(name.to_string(), DefKind::Function, span);
            self.resolutions.insert(span, def_id);
            def_id
        }

        /// Also map an identifier reference span to a DefId.
        fn reference(&mut self, span: Span, def_id: DefId) {
            self.resolutions.insert(span, def_id);
        }

        fn into_resolve_result(self) -> ResolveResult {
            ResolveResult {
                scope_table: self.scope_table,
                resolutions: self.resolutions,
                namespace_aliases: HashMap::new(),
                diagnostics: Vec::new(),
            }
        }
    }

    fn ident(name: &str, span: Span) -> Ident {
        Ident {
            name: name.to_string(),
            span,
        }
    }

    fn check_source_result(source: &str) -> CheckResult {
        let file_id = FileId::new(0);
        let parse_result = parse(source, file_id);
        assert!(
            parse_result.errors.is_empty(),
            "unexpected parse errors: {:?}",
            parse_result.errors
        );

        let resolve_result = jett_resolve::resolve(&parse_result.module);
        let resolve_errors: Vec<_> = resolve_result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(
            resolve_errors.is_empty(),
            "unexpected resolve errors: {:?}",
            resolve_result.diagnostics
        );

        check(&parse_result.module, &resolve_result)
    }

    fn check_source_errors(source: &str) -> Vec<Diagnostic> {
        check_source_result(source)
            .diagnostics
            .into_iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect()
    }

    // ---------------------------------------------------------------
    // Test: simple function with parameters and return type
    // ---------------------------------------------------------------

    #[test]
    fn simple_function_params_and_return() {
        // function add(a: int64, b: int64) returns int64:
        //     return a + b
        let fn_name_span = sp(0, 3);
        let param_a_span = sp(4, 5);
        let param_b_span = sp(6, 7);
        let ref_a_span = sp(10, 11);
        let ref_b_span = sp(12, 13);
        let binop_span = sp(10, 13);
        let ret_span = sp(8, 13);
        let body_span = sp(8, 14);
        let func_span = sp(0, 14);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("add", fn_name_span);
        let a_def_id = env.def_param("a", param_a_span);
        let b_def_id = env.def_param("b", param_b_span);
        env.reference(ref_a_span, a_def_id);
        env.reference(ref_b_span, b_def_id);
        // Also reference fn name for self-registration
        env.reference(fn_name_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("add", fn_name_span),
                type_params: vec![],
                params: vec![
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("a", param_a_span),
                        ty: TypeExpr::Named(ident("int64", sp(100, 105))),
                        span: param_a_span,
                    },
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("b", param_b_span),
                        ty: TypeExpr::Named(ident("int64", sp(106, 111))),
                        span: param_b_span,
                    },
                ],
                return_type: Some(TypeExpr::Named(ident("int64", sp(112, 117)))),
                body: Block {
                    stmts: vec![Stmt::Return(ReturnStmt {
                        value: Some(Expr::Binary(
                            Box::new(Expr::Ident(ident("a", ref_a_span))),
                            BinOp::Add,
                            Box::new(Expr::Ident(ident("b", ref_b_span))),
                            binop_span,
                        )),
                        span: ret_span,
                    })],
                    span: body_span,
                },
                exported: false,
                span: func_span,
            })],
            span: sp(0, 14),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        // No errors expected.
        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

        // The binary expression should be typed as int64.
        assert_eq!(result.type_map[&binop_span], TypeInterner::INT64);
    }

    // ---------------------------------------------------------------
    // Test: type mismatch error (int64 + string)
    // ---------------------------------------------------------------

    #[test]
    fn type_mismatch_int_plus_string() {
        // a: int64, b: string  →  a + b  should emit an error
        let fn_name_span = sp(0, 3);
        let param_a_span = sp(4, 5);
        let param_b_span = sp(6, 7);
        let ref_a_span = sp(10, 11);
        let ref_b_span = sp(12, 13);
        let binop_span = sp(10, 13);
        let body_span = sp(8, 14);
        let func_span = sp(0, 14);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("bad", fn_name_span);
        let a_def_id = env.def_param("a", param_a_span);
        let b_def_id = env.def_param("b", param_b_span);
        env.reference(ref_a_span, a_def_id);
        env.reference(ref_b_span, b_def_id);
        env.reference(fn_name_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("bad", fn_name_span),
                type_params: vec![],
                params: vec![
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("a", param_a_span),
                        ty: TypeExpr::Named(ident("int64", sp(100, 105))),
                        span: param_a_span,
                    },
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("b", param_b_span),
                        ty: TypeExpr::Named(ident("string", sp(106, 112))),
                        span: param_b_span,
                    },
                ],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(113, 120)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Binary(
                            Box::new(Expr::Ident(ident("a", ref_a_span))),
                            BinOp::Add,
                            Box::new(Expr::Ident(ident("b", ref_b_span))),
                            binop_span,
                        ),
                        span: binop_span,
                    })],
                    span: body_span,
                },
                exported: false,
                span: func_span,
            })],
            span: sp(0, 14),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 301); // binary_op_mismatch
        assert!(errors[0].message.contains("int64"));
        assert!(errors[0].message.contains("string"));
    }

    // ---------------------------------------------------------------
    // Test: binary operator type checking (arithmetic, comparison, logic)
    // ---------------------------------------------------------------

    #[test]
    fn binary_operators_arithmetic_returns_same_type() {
        // 10 + 20 → int64
        let binop_span = sp(0, 5);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                type_params: vec![],
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Binary(
                            Box::new(Expr::IntLiteral(10, sp(0, 2))),
                            BinOp::Add,
                            Box::new(Expr::IntLiteral(20, sp(3, 5))),
                            binop_span,
                        ),
                        span: binop_span,
                    })],
                    span: sp(0, 5),
                },
                exported: false,
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        assert!(
            result
                .diagnostics
                .iter()
                .all(|d| d.severity != jett_diagnostics::Severity::Error),
            "unexpected errors: {:?}",
            result.diagnostics
        );
        assert_eq!(result.type_map[&binop_span], TypeInterner::INT64);
    }

    #[test]
    fn binary_operators_comparison_returns_bool() {
        // 10 < 20 → bool
        let binop_span = sp(0, 5);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                type_params: vec![],
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Binary(
                            Box::new(Expr::IntLiteral(10, sp(0, 2))),
                            BinOp::Lt,
                            Box::new(Expr::IntLiteral(20, sp(3, 5))),
                            binop_span,
                        ),
                        span: binop_span,
                    })],
                    span: sp(0, 5),
                },
                exported: false,
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        assert!(
            result
                .diagnostics
                .iter()
                .all(|d| d.severity != jett_diagnostics::Severity::Error),
            "unexpected errors: {:?}",
            result.diagnostics
        );
        assert_eq!(result.type_map[&binop_span], TypeInterner::BOOL);
    }

    #[test]
    fn binary_operators_logic_requires_bool() {
        // true && false → bool (ok)
        let binop_span = sp(0, 10);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                type_params: vec![],
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Binary(
                            Box::new(Expr::BoolLiteral(true, sp(0, 4))),
                            BinOp::And,
                            Box::new(Expr::BoolLiteral(false, sp(5, 10))),
                            binop_span,
                        ),
                        span: binop_span,
                    })],
                    span: sp(0, 10),
                },
                exported: false,
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        assert!(
            result
                .diagnostics
                .iter()
                .all(|d| d.severity != jett_diagnostics::Severity::Error),
            "unexpected errors: {:?}",
            result.diagnostics
        );
        assert_eq!(result.type_map[&binop_span], TypeInterner::BOOL);
    }

    #[test]
    fn binary_operators_logic_error_on_non_bool() {
        // 42 && true → error
        let binop_span = sp(0, 10);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                type_params: vec![],
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Binary(
                            Box::new(Expr::IntLiteral(42, sp(0, 2))),
                            BinOp::And,
                            Box::new(Expr::BoolLiteral(true, sp(5, 10))),
                            binop_span,
                        ),
                        span: binop_span,
                    })],
                    span: sp(0, 10),
                },
                exported: false,
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code.code(), 301);
    }

    // ---------------------------------------------------------------
    // Test: variable declaration type matching
    // ---------------------------------------------------------------

    #[test]
    fn var_decl_type_match_ok() {
        // int64 x = 42
        let var_name_span = sp(6, 7);
        let var_span = sp(0, 10);

        let mut env = TestEnv::new();
        env.def_var("x", var_name_span);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                type_params: vec![],
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::VarDecl(VarDecl {
                        mutable: false,
                        ty: TypeExpr::Named(ident("int64", sp(0, 5))),
                        name: ident("x", var_name_span),
                        value: Expr::IntLiteral(42, sp(8, 10)),
                        span: var_span,
                    })],
                    span: sp(0, 10),
                },
                exported: false,
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn var_decl_type_mismatch() {
        // string x = 42   →  error E0311
        let var_name_span = sp(7, 8);
        let var_span = sp(0, 12);

        let mut env = TestEnv::new();
        env.def_var("x", var_name_span);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                type_params: vec![],
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::VarDecl(VarDecl {
                        mutable: false,
                        ty: TypeExpr::Named(ident("string", sp(0, 6))),
                        name: ident("x", var_name_span),
                        value: Expr::IntLiteral(42, sp(9, 11)),
                        span: var_span,
                    })],
                    span: sp(0, 12),
                },
                exported: false,
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 311);
        assert!(errors[0].message.contains("string"));
        assert!(errors[0].message.contains("int64"));
    }

    // ---------------------------------------------------------------
    // Test: function call argument type checking
    // ---------------------------------------------------------------

    #[test]
    fn function_call_correct_args() {
        // function add(a: int64, b: int64) returns int64
        // add(1, 2)  →  no error, result is int64
        let fn_name_span = sp(0, 3);
        let param_a_span = sp(4, 5);
        let param_b_span = sp(6, 7);
        let call_ref_span = sp(20, 23);
        let call_span = sp(20, 30);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("add", fn_name_span);
        env.def_param("a", param_a_span);
        env.def_param("b", param_b_span);
        env.reference(call_ref_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("add", fn_name_span),
                type_params: vec![],
                params: vec![
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("a", param_a_span),
                        ty: TypeExpr::Named(ident("int64", sp(100, 105))),
                        span: param_a_span,
                    },
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("b", param_b_span),
                        ty: TypeExpr::Named(ident("int64", sp(106, 111))),
                        span: param_b_span,
                    },
                ],
                return_type: Some(TypeExpr::Named(ident("int64", sp(112, 117)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Call(
                            Box::new(Expr::Ident(ident("add", call_ref_span))),
                            vec![
                                CallArg {
                                    name: None,
                                    value: Expr::IntLiteral(1, sp(24, 25)),
                                    span: sp(24, 25),
                                },
                                CallArg {
                                    name: None,
                                    value: Expr::IntLiteral(2, sp(27, 28)),
                                    span: sp(27, 28),
                                },
                            ],
                            call_span,
                        ),
                        span: call_span,
                    })],
                    span: sp(20, 30),
                },
                exported: false,
                span: sp(0, 30),
            })],
            span: sp(0, 30),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
        assert_eq!(result.type_map[&call_span], TypeInterner::INT64);
    }

    #[test]
    fn function_call_wrong_arg_type() {
        // function add(a: int64, b: int64) returns int64
        // add(1, "hello")  →  error E0304
        let fn_name_span = sp(0, 3);
        let param_a_span = sp(4, 5);
        let param_b_span = sp(6, 7);
        let call_ref_span = sp(20, 23);
        let call_span = sp(20, 35);
        let bad_arg_span = sp(27, 34);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("add", fn_name_span);
        env.def_param("a", param_a_span);
        env.def_param("b", param_b_span);
        env.reference(call_ref_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("add", fn_name_span),
                type_params: vec![],
                params: vec![
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("a", param_a_span),
                        ty: TypeExpr::Named(ident("int64", sp(100, 105))),
                        span: param_a_span,
                    },
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("b", param_b_span),
                        ty: TypeExpr::Named(ident("int64", sp(106, 111))),
                        span: param_b_span,
                    },
                ],
                return_type: Some(TypeExpr::Named(ident("int64", sp(112, 117)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Call(
                            Box::new(Expr::Ident(ident("add", call_ref_span))),
                            vec![
                                CallArg {
                                    name: None,
                                    value: Expr::IntLiteral(1, sp(24, 25)),
                                    span: sp(24, 25),
                                },
                                CallArg {
                                    name: None,
                                    value: Expr::StringLiteral("hello".to_string(), bad_arg_span),
                                    span: bad_arg_span,
                                },
                            ],
                            call_span,
                        ),
                        span: call_span,
                    })],
                    span: sp(20, 35),
                },
                exported: false,
                span: sp(0, 35),
            })],
            span: sp(0, 35),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 304); // argument_type_mismatch
        assert!(errors[0].message.contains("int64"));
        assert!(errors[0].message.contains("string"));
    }

    #[test]
    fn function_call_wrong_arg_count() {
        // function add(a: int64, b: int64) returns int64
        // add(1)  →  error E0303
        let fn_name_span = sp(0, 3);
        let param_a_span = sp(4, 5);
        let param_b_span = sp(6, 7);
        let call_ref_span = sp(20, 23);
        let call_span = sp(20, 28);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("add", fn_name_span);
        env.def_param("a", param_a_span);
        env.def_param("b", param_b_span);
        env.reference(call_ref_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("add", fn_name_span),
                type_params: vec![],
                params: vec![
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("a", param_a_span),
                        ty: TypeExpr::Named(ident("int64", sp(100, 105))),
                        span: param_a_span,
                    },
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("b", param_b_span),
                        ty: TypeExpr::Named(ident("int64", sp(106, 111))),
                        span: param_b_span,
                    },
                ],
                return_type: Some(TypeExpr::Named(ident("int64", sp(112, 117)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Call(
                            Box::new(Expr::Ident(ident("add", call_ref_span))),
                            vec![CallArg {
                                name: None,
                                value: Expr::IntLiteral(1, sp(24, 25)),
                                span: sp(24, 25),
                            }],
                            call_span,
                        ),
                        span: call_span,
                    })],
                    span: sp(20, 28),
                },
                exported: false,
                span: sp(0, 28),
            })],
            span: sp(0, 28),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 303); // argument_count_mismatch
    }

    // ---------------------------------------------------------------
    // Test: if condition must be bool
    // ---------------------------------------------------------------

    #[test]
    fn if_condition_must_be_bool() {
        // if 42:   →  error E0306
        let cond_span = sp(3, 5);
        let if_span = sp(0, 10);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                type_params: vec![],
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::If(IfStmt {
                        condition: Expr::IntLiteral(42, cond_span),
                        then_block: Block {
                            stmts: vec![],
                            span: sp(6, 10),
                        },
                        else_ifs: vec![],
                        else_block: None,
                        span: if_span,
                    })],
                    span: sp(0, 10),
                },
                exported: false,
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 306); // condition_not_bool
        assert!(errors[0].message.contains("int64"));
    }

    #[test]
    fn if_condition_bool_ok() {
        // if true:   →  no error
        let cond_span = sp(3, 7);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                type_params: vec![],
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::If(IfStmt {
                        condition: Expr::BoolLiteral(true, cond_span),
                        then_block: Block {
                            stmts: vec![],
                            span: sp(8, 10),
                        },
                        else_ifs: vec![],
                        else_block: None,
                        span: sp(0, 10),
                    })],
                    span: sp(0, 10),
                },
                exported: false,
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    // ---------------------------------------------------------------
    // Test: return type checking
    // ---------------------------------------------------------------

    #[test]
    fn return_type_mismatch() {
        // function foo() returns int64:
        //     return "hello"
        // → error E0305
        let fn_name_span = sp(0, 3);
        let ret_span = sp(10, 25);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("foo", fn_name_span);
        env.reference(fn_name_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("foo", fn_name_span),
                type_params: vec![],
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("int64", sp(100, 105)))),
                body: Block {
                    stmts: vec![Stmt::Return(ReturnStmt {
                        value: Some(Expr::StringLiteral("hello".to_string(), sp(17, 24))),
                        span: ret_span,
                    })],
                    span: sp(10, 25),
                },
                exported: false,
                span: sp(0, 25),
            })],
            span: sp(0, 25),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 305); // return_type_mismatch
        assert!(errors[0].message.contains("int64"));
        assert!(errors[0].message.contains("string"));
    }

    #[test]
    fn return_type_correct() {
        // function foo() returns int64:
        //     return 42
        let fn_name_span = sp(0, 3);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("foo", fn_name_span);
        env.reference(fn_name_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("foo", fn_name_span),
                type_params: vec![],
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("int64", sp(100, 105)))),
                body: Block {
                    stmts: vec![Stmt::Return(ReturnStmt {
                        value: Some(Expr::IntLiteral(42, sp(17, 19))),
                        span: sp(10, 19),
                    })],
                    span: sp(10, 19),
                },
                exported: false,
                span: sp(0, 19),
            })],
            span: sp(0, 19),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    // ---------------------------------------------------------------
    // Test: unary operators
    // ---------------------------------------------------------------

    #[test]
    fn unary_not_requires_bool() {
        // not 42  →  error
        let unary_span = sp(0, 6);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                type_params: vec![],
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Unary(
                            UnaryOp::Not,
                            Box::new(Expr::IntLiteral(42, sp(4, 6))),
                            unary_span,
                        ),
                        span: unary_span,
                    })],
                    span: sp(0, 6),
                },
                exported: false,
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code.code(), 302); // unary_op_mismatch
    }

    // ---------------------------------------------------------------
    // Test: for loop iterable check
    // ---------------------------------------------------------------

    #[test]
    fn for_loop_requires_list() {
        // for x in 42:  →  error E0307
        let var_span = sp(4, 5);
        let iterable_span = sp(9, 11);
        let for_span = sp(0, 15);

        let mut env = TestEnv::new();
        env.def_var("x", var_span);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                type_params: vec![],
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::For(ForStmt {
                        variable: ident("x", var_span),
                        value_variable: None,
                        view: false,
                        iterable: Expr::IntLiteral(42, iterable_span),
                        body: Block {
                            stmts: vec![],
                            span: sp(12, 15),
                        },
                        span: for_span,
                    })],
                    span: sp(0, 15),
                },
                exported: false,
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 307);
    }

    // ---------------------------------------------------------------
    // Test: assignment type mismatch
    // ---------------------------------------------------------------

    #[test]
    fn assignment_type_mismatch() {
        // int64 x = 42
        // x = "hello"  →  error E0312
        let var_name_span = sp(6, 7);
        let var_span = sp(0, 10);
        let ref_x_span = sp(15, 16);
        let assign_span = sp(15, 26);

        let mut env = TestEnv::new();
        let x_def = env.def_var("x", var_name_span);
        env.reference(ref_x_span, x_def);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                type_params: vec![],
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![
                        Stmt::VarDecl(VarDecl {
                            mutable: true,
                            ty: TypeExpr::Named(ident("int64", sp(0, 5))),
                            name: ident("x", var_name_span),
                            value: Expr::IntLiteral(42, sp(8, 10)),
                            span: var_span,
                        }),
                        Stmt::Assign(AssignStmt {
                            target: Expr::Ident(ident("x", ref_x_span)),
                            value: Expr::StringLiteral("hello".to_string(), sp(19, 26)),
                            span: assign_span,
                        }),
                    ],
                    span: sp(0, 26),
                },
                exported: false,
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 312); // assign_type_mismatch
    }

    // ---------------------------------------------------------------
    // Test: resolve_type_expr for generic types
    // ---------------------------------------------------------------

    #[test]
    fn resolve_generic_types() {
        // Verify that type expressions like list[int64], result[string, int64] resolve correctly.
        let env = TestEnv::new();
        let resolve = env.into_resolve_result();
        let mut checker = TypeChecker::new(&resolve);

        let list_int = checker.resolve_type_expr(&TypeExpr::Generic(
            ident("list", sp(0, 4)),
            vec![TypeExpr::Named(ident("int64", sp(5, 10)))],
            sp(0, 11),
        ));
        assert_eq!(
            *checker.interner.resolve(list_int),
            Type::List(TypeInterner::INT64)
        );

        let result_type = checker.resolve_type_expr(&TypeExpr::Generic(
            ident("result", sp(0, 6)),
            vec![
                TypeExpr::Named(ident("string", sp(7, 13))),
                TypeExpr::Named(ident("int64", sp(15, 20))),
            ],
            sp(0, 21),
        ));
        assert_eq!(
            *checker.interner.resolve(result_type),
            Type::Result(TypeInterner::STRING, TypeInterner::INT64)
        );
    }

    // ---------------------------------------------------------------
    // Test: capability-based purity enforcement
    // ---------------------------------------------------------------

    /// Helper: create a minimal function definition.
    fn make_function(name: &str, name_span: Span, params: Vec<Param>, body: Block) -> FunctionDef {
        FunctionDef {
            name: ident(name, name_span),
            type_params: vec![],
            params,
            return_type: Some(TypeExpr::Named(ident("nothing", sp(200, 207)))),
            body,
            exported: false,
            span: Span::new(name_span.file, name_span.start, name_span.start + 50),
        }
    }

    #[test]
    fn pure_function_calling_pure_function_ok() {
        // function helper() returns nothing:
        //     return nothing
        // function caller() returns nothing:
        //     helper()
        let helper_name_span = sp(0, 6);
        let caller_name_span = sp(100, 106);
        let call_ref_span = sp(110, 116);
        let call_span = sp(110, 118);

        let mut env = TestEnv::new();
        let helper_def = env.def_func("helper", helper_name_span);
        let _caller_def = env.def_func("caller", caller_name_span);
        env.reference(helper_name_span, helper_def);
        env.reference(caller_name_span, _caller_def);
        env.reference(call_ref_span, helper_def);

        let module = Module {
            items: vec![
                Item::Function(make_function(
                    "helper",
                    helper_name_span,
                    vec![],
                    Block {
                        stmts: vec![],
                        span: sp(10, 20),
                    },
                )),
                Item::Function(make_function(
                    "caller",
                    caller_name_span,
                    vec![],
                    Block {
                        stmts: vec![Stmt::Expr(ExprStmt {
                            expr: Expr::Call(
                                Box::new(Expr::Ident(ident("helper", call_ref_span))),
                                vec![],
                                call_span,
                            ),
                            span: call_span,
                        })],
                        span: sp(107, 120),
                    },
                )),
            ],
            span: sp(0, 200),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn pure_function_calling_impure_function_error() {
        // function writer(view out: Stdout) returns nothing:
        //     return nothing
        // function caller() returns nothing:
        //     writer()          ← E0500
        let writer_name_span = sp(0, 6);
        let writer_param_span = sp(7, 10);
        let caller_name_span = sp(100, 106);
        let call_ref_span = sp(110, 116);
        let call_span = sp(110, 118);

        let mut env = TestEnv::new();
        let writer_def = env.def_func("writer", writer_name_span);
        let _caller_def = env.def_func("caller", caller_name_span);
        env.def_param("out", writer_param_span);
        env.reference(writer_name_span, writer_def);
        env.reference(caller_name_span, _caller_def);
        env.reference(call_ref_span, writer_def);

        let module = Module {
            items: vec![
                Item::Function(make_function(
                    "writer",
                    writer_name_span,
                    vec![Param {
                        view: true,
                        mutable: false,
                        name: ident("out", writer_param_span),
                        ty: TypeExpr::View(
                            Box::new(TypeExpr::Named(ident("Stdout", sp(11, 17)))),
                            sp(7, 17),
                        ),
                        span: writer_param_span,
                    }],
                    Block {
                        stmts: vec![],
                        span: sp(20, 30),
                    },
                )),
                Item::Function(make_function(
                    "caller",
                    caller_name_span,
                    vec![],
                    Block {
                        stmts: vec![Stmt::Expr(ExprStmt {
                            expr: Expr::Call(
                                Box::new(Expr::Ident(ident("writer", call_ref_span))),
                                vec![],
                                call_span,
                            ),
                            span: call_span,
                        })],
                        span: sp(107, 120),
                    },
                )),
            ],
            span: sp(0, 200),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        // Expect E0500 (pure calls impure) and E0303 (arg count mismatch — caller
        // passes 0 args but writer expects 1).  We only assert E0500 exists.
        let purity_errors: Vec<_> = errors.iter().filter(|d| d.code.code() == 500).collect();
        assert_eq!(
            purity_errors.len(),
            1,
            "expected 1 purity error (E0500), got: {:?}",
            purity_errors
        );
        assert!(purity_errors[0].message.contains("caller"));
        assert!(purity_errors[0].message.contains("writer"));
    }

    #[test]
    fn impure_function_calling_impure_function_ok() {
        // function writer(view out: Stdout) returns nothing:
        //     return nothing
        // function caller(view out: Stdout) returns nothing:
        //     writer()          ← ok, caller is also impure
        let writer_name_span = sp(0, 6);
        let writer_param_span = sp(7, 10);
        let caller_name_span = sp(100, 106);
        let caller_param_span = sp(107, 110);
        let call_ref_span = sp(150, 156);
        let call_span = sp(150, 158);

        let mut env = TestEnv::new();
        let writer_def = env.def_func("writer", writer_name_span);
        let _caller_def = env.def_func("caller", caller_name_span);
        env.def_param("out", writer_param_span);
        env.def_param("out2", caller_param_span);
        env.reference(writer_name_span, writer_def);
        env.reference(caller_name_span, _caller_def);
        env.reference(call_ref_span, writer_def);

        let module = Module {
            items: vec![
                Item::Function(make_function(
                    "writer",
                    writer_name_span,
                    vec![Param {
                        view: true,
                        mutable: false,
                        name: ident("out", writer_param_span),
                        ty: TypeExpr::View(
                            Box::new(TypeExpr::Named(ident("Stdout", sp(11, 17)))),
                            sp(7, 17),
                        ),
                        span: writer_param_span,
                    }],
                    Block {
                        stmts: vec![],
                        span: sp(20, 30),
                    },
                )),
                Item::Function(make_function(
                    "caller",
                    caller_name_span,
                    vec![Param {
                        view: true,
                        mutable: false,
                        name: ident("out2", caller_param_span),
                        ty: TypeExpr::View(
                            Box::new(TypeExpr::Named(ident("Stdout", sp(111, 117)))),
                            sp(107, 117),
                        ),
                        span: caller_param_span,
                    }],
                    Block {
                        stmts: vec![Stmt::Expr(ExprStmt {
                            expr: Expr::Call(
                                Box::new(Expr::Ident(ident("writer", call_ref_span))),
                                vec![],
                                call_span,
                            ),
                            span: call_span,
                        })],
                        span: sp(140, 160),
                    },
                )),
            ],
            span: sp(0, 300),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        // No purity errors expected (E0500).
        let purity_errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code.code() == 500)
            .collect();
        assert!(
            purity_errors.is_empty(),
            "unexpected purity errors: {:?}",
            purity_errors
        );
    }

    #[test]
    fn function_with_stdout_param_is_impure() {
        // function printer(view out: Stdout) returns nothing
        // This is a unit-level check: the purity map marks this function as impure.
        let fn_name_span = sp(0, 7);
        let param_span = sp(8, 11);

        let mut env = TestEnv::new();
        let fn_def = env.def_func("printer", fn_name_span);
        env.def_param("out", param_span);
        env.reference(fn_name_span, fn_def);

        let func = make_function(
            "printer",
            fn_name_span,
            vec![Param {
                view: true,
                mutable: false,
                name: ident("out", param_span),
                ty: TypeExpr::View(
                    Box::new(TypeExpr::Named(ident("Stdout", sp(12, 18)))),
                    sp(8, 18),
                ),
                span: param_span,
            }],
            Block {
                stmts: vec![],
                span: sp(20, 30),
            },
        );

        let module = Module {
            items: vec![Item::Function(func)],
            span: sp(0, 100),
        };

        let resolve = env.into_resolve_result();
        let mut checker = TypeChecker::new(&resolve);
        checker.check_module(&module);

        // The purity map should mark "printer" as impure.
        assert_eq!(
            checker.purity_map.get("printer").copied(),
            Some(false),
            "function with Stdout param should be impure"
        );
    }

    #[test]
    fn function_without_capability_params_is_pure() {
        // function add(a: int64, b: int64) returns nothing
        let fn_name_span = sp(0, 3);
        let param_a_span = sp(4, 5);
        let param_b_span = sp(6, 7);

        let mut env = TestEnv::new();
        let fn_def = env.def_func("add", fn_name_span);
        env.def_param("a", param_a_span);
        env.def_param("b", param_b_span);
        env.reference(fn_name_span, fn_def);

        let func = make_function(
            "add",
            fn_name_span,
            vec![
                Param {
                    view: false,
                    mutable: false,
                    name: ident("a", param_a_span),
                    ty: TypeExpr::Named(ident("int64", sp(100, 105))),
                    span: param_a_span,
                },
                Param {
                    view: false,
                    mutable: false,
                    name: ident("b", param_b_span),
                    ty: TypeExpr::Named(ident("int64", sp(106, 111))),
                    span: param_b_span,
                },
            ],
            Block {
                stmts: vec![],
                span: sp(10, 20),
            },
        );

        let module = Module {
            items: vec![Item::Function(func)],
            span: sp(0, 100),
        };

        let resolve = env.into_resolve_result();
        let mut checker = TypeChecker::new(&resolve);
        checker.check_module(&module);

        assert_eq!(
            checker.purity_map.get("add").copied(),
            Some(true),
            "function without capability params should be pure"
        );
    }

    #[test]
    fn verify_block_calling_impure_function_error() {
        // function writer(view out: Stdout) returns nothing:
        //     return nothing
        // verify test_writer:
        //     writer()          ← E0501
        let writer_name_span = sp(0, 6);
        let writer_param_span = sp(7, 10);
        let call_ref_span = sp(110, 116);
        let call_span = sp(110, 118);
        let verify_span = sp(100, 130);

        let mut env = TestEnv::new();
        let writer_def = env.def_func("writer", writer_name_span);
        env.def_param("out", writer_param_span);
        env.reference(writer_name_span, writer_def);
        env.reference(call_ref_span, writer_def);

        let module = Module {
            items: vec![
                Item::Function(make_function(
                    "writer",
                    writer_name_span,
                    vec![Param {
                        view: true,
                        mutable: false,
                        name: ident("out", writer_param_span),
                        ty: TypeExpr::View(
                            Box::new(TypeExpr::Named(ident("Stdout", sp(11, 17)))),
                            sp(7, 17),
                        ),
                        span: writer_param_span,
                    }],
                    Block {
                        stmts: vec![],
                        span: sp(20, 30),
                    },
                )),
                Item::Verify(VerifyBlock {
                    name: ident("test_writer", sp(100, 111)),
                    body: Block {
                        stmts: vec![Stmt::Expr(ExprStmt {
                            expr: Expr::Call(
                                Box::new(Expr::Ident(ident("writer", call_ref_span))),
                                vec![],
                                call_span,
                            ),
                            span: call_span,
                        })],
                        span: sp(112, 130),
                    },
                    span: verify_span,
                }),
            ],
            span: sp(0, 200),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let verify_errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code.code() == 501)
            .collect();
        assert_eq!(
            verify_errors.len(),
            1,
            "expected 1 verify purity error (E0501), got: {:?}",
            verify_errors
        );
        assert!(verify_errors[0].message.contains("test_writer"));
        assert!(verify_errors[0].message.contains("writer"));
    }

    #[test]
    fn verify_block_calling_pure_function_ok() {
        // function helper() returns nothing:
        //     return nothing
        // verify test_helper:
        //     helper()          ← ok
        let helper_name_span = sp(0, 6);
        let call_ref_span = sp(110, 116);
        let call_span = sp(110, 118);
        let verify_span = sp(100, 130);

        let mut env = TestEnv::new();
        let helper_def = env.def_func("helper", helper_name_span);
        env.reference(helper_name_span, helper_def);
        env.reference(call_ref_span, helper_def);

        let module = Module {
            items: vec![
                Item::Function(make_function(
                    "helper",
                    helper_name_span,
                    vec![],
                    Block {
                        stmts: vec![],
                        span: sp(10, 20),
                    },
                )),
                Item::Verify(VerifyBlock {
                    name: ident("test_helper", sp(100, 111)),
                    body: Block {
                        stmts: vec![Stmt::Expr(ExprStmt {
                            expr: Expr::Call(
                                Box::new(Expr::Ident(ident("helper", call_ref_span))),
                                vec![],
                                call_span,
                            ),
                            span: call_span,
                        })],
                        span: sp(112, 130),
                    },
                    span: verify_span,
                }),
            ],
            span: sp(0, 200),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let purity_errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code.code() == 500 || d.code.code() == 501)
            .collect();
        assert!(
            purity_errors.is_empty(),
            "unexpected purity errors: {:?}",
            purity_errors
        );
    }

    #[test]
    fn mutual_block_allows_mutual_recursion() {
        let result = check_source_result(
            "\
mutual:
    function is_even(n: int64) returns bool
    function is_odd(n: int64) returns bool

function is_even(n: int64) returns bool:
    if n == 0:
        return true
    return is_odd(n - 1)

function is_odd(n: int64) returns bool:
    if n == 0:
        return false
    return is_even(n - 1)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn mutual_block_missing_definition_reports_error() {
        let errors = check_source_errors(
            "\
mutual:
    function is_even(n: int64) returns bool

function main() returns nothing:
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 325),
            "expected E0325, got: {:?}",
            errors
        );
    }

    #[test]
    fn mutual_block_signature_mismatch_reports_error() {
        let errors = check_source_errors(
            "\
mutual:
    function is_even(n: int64) returns bool

function is_even(value: string) returns bool:
    return true
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 326),
            "expected E0326, got: {:?}",
            errors
        );
    }

    #[test]
    fn user_defined_struct_constructor_and_field_access_typecheck_cleanly() {
        let result = check_source_result(
            "\
struct Point:
    x: int64
    y: int64

function sum(view point: Point) returns int64:
    return point.x + point.y

function main() returns int64:
    Point point = Point(x: 1, y: 2)
    return sum(view point)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn struct_method_call_typechecks_cleanly() {
        let result = check_source_result(
            "\
struct Point:
    x: int64
    y: int64

    function total(view self: Point) returns int64:
        return self.x + self.y

function main() returns int64:
    Point point = Point(x: 1, y: 2)
    return Point.total(view point)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn struct_constructor_missing_field_reports_error() {
        let errors = check_source_errors(
            "\
struct Point:
    x: int64
    y: int64

function main() returns Point:
    return Point(x: 1)
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 321),
            "expected E0321, got: {:?}",
            errors
        );
    }

    #[test]
    fn unknown_struct_field_reports_error() {
        let errors = check_source_errors(
            "\
struct Point:
    x: int64
    y: int64

function main(view point: Point) returns int64:
    return point.z
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 319),
            "expected E0319, got: {:?}",
            errors
        );
    }

    #[test]
    fn enum_variant_construction_and_match_typecheck_cleanly() {
        let result = check_source_result(
            "\
enum Shape:
    circle(radius: int64)
    rect(width: int64, height: int64)

function area(shape: Shape) returns int64:
    match shape:
        circle(radius):
            return radius * radius
        rect(width, height):
            return width * height

function main() returns int64:
    Shape shape = Shape.circle(3)
    return area(shape)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn non_exhaustive_enum_match_reports_error() {
        let errors = check_source_errors(
            "\
enum Color:
    red
    blue

function describe(color: Color) returns int64:
    match color:
        red:
            return 1
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 324),
            "expected E0324, got: {:?}",
            errors
        );
    }

    #[test]
    fn enum_pattern_binding_count_mismatch_reports_error() {
        let errors = check_source_errors(
            "\
enum Shape:
    rect(width: int64, height: int64)

function area(shape: Shape) returns int64:
    match shape:
        rect(width):
            return width
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 323),
            "expected E0323, got: {:?}",
            errors
        );
    }

    #[test]
    fn result_handle_requires_error_keyword() {
        let errors = check_source_errors(
            "\
function main() returns int64:
    int64 parsed = int64.from_string(\"42\") handle:
        default 0
    return parsed
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 316),
            "expected E0316, got: {:?}",
            errors
        );
    }

    #[test]
    fn optional_handle_rejects_error_keyword() {
        let errors = check_source_errors(
            "\
function main() returns int64:
    int64 value = some(1) handle error:
        default 0
    return value
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 317),
            "expected E0317, got: {:?}",
            errors
        );
    }

    #[test]
    fn handle_block_requires_explicit_terminator() {
        let errors = check_source_errors(
            "\
function main() returns int64:
    int64 parsed = int64.from_string(\"oops\") handle error:
        int64 fallback = 0
    return parsed
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 318),
            "expected E0318, got: {:?}",
            errors
        );
    }

    #[test]
    fn handle_default_must_match_unwrapped_type() {
        let errors = check_source_errors(
            "\
function main() returns int64:
    int64 parsed = int64.from_string(\"oops\") handle error:
        default \"bad\"
    return parsed
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 300),
            "expected E0300, got: {:?}",
            errors
        );
    }

    #[test]
    fn handle_with_builtin_result_type_checks_cleanly() {
        let errors = check_source_errors(
            "\
function main() returns int64:
    int64 parsed = int64.from_string(\"42\") handle error:
        default 0
    return parsed
",
        );

        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn interface_implement_call_typechecks_cleanly() {
        let result = check_source_result(
            "\
interface Speaker:
    function speak(view self: Speaker) returns string

struct Dog:
    name: string

implement Speaker for Dog:
    function speak(view self: Dog) returns string:
        return self.name

function main() returns string:
    Dog dog = Dog(name: \"woof\")
    return Speaker.speak(view dog)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn implement_block_missing_method_reports_error() {
        let errors = check_source_errors(
            "\
interface Speaker:
    function speak(view self: Speaker) returns string
    function growl(view self: Speaker) returns string

struct Dog:
    name: string

implement Speaker for Dog:
    function speak(view self: Dog) returns string:
        return self.name
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 331),
            "expected E0331, got: {:?}",
            errors
        );
    }

    #[test]
    fn implement_block_signature_mismatch_reports_error() {
        let errors = check_source_errors(
            "\
interface Speaker:
    function speak(view self: Speaker) returns string

struct Dog:
    name: string

implement Speaker for Dog:
    function speak(self: Dog) returns int64:
        return 1
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 330),
            "expected E0330, got: {:?}",
            errors
        );
    }

    #[test]
    fn string_interpolation_accepts_user_defined_displayable_types() {
        let result = check_source_result(
            "\
interface Displayable:
    function display(view self: Displayable) returns string

struct User:
    name: string

implement Displayable for User:
    function display(view self: User) returns string:
        return self.name

function main() returns string:
    User user = User(name: \"Ada\")
    return \"user: {user}\"
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn secret_value_can_be_declassified() {
        let result = check_source_result(
            "\
function reveal(key: secret[string]) returns string:
    return declassify key

function main() returns string:
    secret[string] api_key = \"abc\"
    return reveal(api_key)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn declassify_requires_secret_type() {
        let errors = check_source_errors(
            "\
function main() returns string:
    return declassify \"abc\"
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 601),
            "expected E0601, got: {:?}",
            errors
        );
    }

    #[test]
    fn stdout_write_rejects_secret_values() {
        let errors = check_source_errors(
            "\
function main(view stdout: Stdout) returns nothing:
    secret[string] api_key = \"abc\"
    Stdout.write(view stdout, api_key)
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 600),
            "expected E0600, got: {:?}",
            errors
        );
    }

    #[test]
    fn secret_values_are_not_displayable() {
        let errors = check_source_errors(
            "\
function main() returns string:
    secret[string] api_key = \"abc\"
    return \"api key: {api_key}\"
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 332),
            "expected E0332, got: {:?}",
            errors
        );
    }

    #[test]
    fn pure_call_with_secret_argument_returns_secret() {
        let result = check_source_result(
            "\
function main() returns nothing:
    secret[string] api_key = \"abc\"
    secret[string] upper = string.upper(api_key)
    return nothing
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn pure_call_with_secret_argument_cannot_be_assigned_to_public_type() {
        let errors = check_source_errors(
            "\
function main() returns nothing:
    secret[string] api_key = \"abc\"
    string upper = string.upper(api_key)
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 311),
            "expected E0311, got: {:?}",
            errors
        );
    }

    #[test]
    fn secret_redact_returns_public_string() {
        let result = check_source_result(
            "\
function main() returns string:
    secret[string] api_key = \"abc\"
    string masked = secret.redact(api_key)
    return masked
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn secret_compare_returns_bool() {
        let result = check_source_result(
            "\
function main() returns bool:
    secret[string] stored = \"abc\"
    secret[string] computed = \"abc\"
    return secret.compare(stored, computed)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn secret_compare_requires_matching_secret_types() {
        let errors = check_source_errors(
            "\
function main() returns bool:
    secret[string] stored = \"abc\"
    secret[int64] computed = 1
    return secret.compare(stored, computed)
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 304),
            "expected E0304, got: {:?}",
            errors
        );
    }

    #[test]
    fn secret_redact_requires_secret_argument() {
        let errors = check_source_errors(
            "\
function main() returns string:
    return secret.redact(\"abc\")
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 602),
            "expected E0602, got: {:?}",
            errors
        );
    }

    #[test]
    fn field_access_on_secret_struct_stays_secret() {
        let result = check_source_result(
            "\
struct User:
    name: string

function main() returns nothing:
    secret[User] user = User(name: \"Ada\")
    secret[string] name = user.name
    return nothing
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn mixed_secret_and_public_list_becomes_secret_element_list() {
        let result = check_source_result(
            "\
function main() returns nothing:
    secret[string] api_key = \"abc\"
    list[secret[string]] items = list(\"prefix\", api_key, \"suffix\")
    return nothing
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn string_join_rejects_list_of_secret_strings() {
        let errors = check_source_errors(
            "\
function main() returns string:
    secret[string] api_key = \"abc\"
    list[secret[string]] items = list(\"prefix\", api_key, \"suffix\")
    return string.join(items, \"-\")
",
        );

        assert!(
            errors
                .iter()
                .any(|d| d.code.code() == 304 || d.code.code() == 305),
            "expected argument/return type mismatch, got: {:?}",
            errors
        );
    }

    #[test]
    fn filesystem_write_file_rejects_secret_string() {
        let errors = check_source_errors(
            "\
function main(view fs: Filesystem) returns nothing:
    secret[string] api_key = \"abc\"
    Filesystem.write_file(view fs, \"secret.txt\", api_key) handle error:
        return nothing
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 600),
            "expected E0600, got: {:?}",
            errors
        );
    }

    #[test]
    fn json_serialize_blocks_struct_with_secret_fields() {
        let errors = check_source_errors(
            "\
struct User:
    id: string
    api_key: secret[string]

function main(view user: User) returns string:
    return json.serialize[User](view user)
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 603),
            "expected E0603, got: {:?}",
            errors
        );
    }

    #[test]
    fn json_serialize_public_allows_struct_with_secret_fields() {
        let result = check_source_result(
            "\
struct User:
    id: string
    api_key: secret[string]

function main(view user: User) returns string:
    return json.serialize_public[User](view user)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn json_serialize_public_rejects_secret_wrapped_value() {
        let errors = check_source_errors(
            "\
struct User:
    id: string

function main() returns string:
    secret[User] user = User(id: \"1\")
    return json.serialize_public[User](user)
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 600),
            "expected E0600, got: {:?}",
            errors
        );
    }

    #[test]
    fn filesystem_read_file_returns_result_string() {
        let result = check_source_result(
            "\
function main(view fs: Filesystem) returns string:
    string raw = Filesystem.read_file(view fs, \"config.json\") handle error:
        default \"\"
    return raw
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn pure_function_lifts_secret_argument() {
        let result = check_source_result(
            "\
function upper(value: string) returns string:
    return string.upper(value)

function main() returns secret[string]:
    secret[string] api_key = \"abc\"
    return upper(api_key)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn impure_function_rejects_secret_argument_without_secret_param() {
        let errors = check_source_errors(
            "\
function emit(view stdout: Stdout, value: string) returns nothing:
    Stdout.write(view stdout, value)
    return nothing

function main(view stdout: Stdout) returns nothing:
    secret[string] api_key = \"abc\"
    emit(view stdout, api_key)
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 600),
            "expected E0600, got: {:?}",
            errors
        );
    }

    #[test]
    fn impure_function_accepts_explicit_secret_param() {
        let result = check_source_result(
            "\
function send_secret(view stdout: Stdout, value: secret[string]) returns nothing:
    string redacted = secret.redact(value)
    Stdout.write(view stdout, redacted)
    return nothing

function main(view stdout: Stdout) returns nothing:
    secret[string] api_key = \"abc\"
    send_secret(view stdout, api_key)
    return nothing
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn refinement_type_assignment_requires_handle_error() {
        let errors = check_source_errors(
            "\
type Port = int64 where value >= 1 && value <= 65535

function main() returns nothing:
    Port port = 8080
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 333),
            "expected E0333, got: {:?}",
            errors
        );
    }

    #[test]
    fn refinement_type_assignment_with_handle_and_coarsen_typechecks() {
        let result = check_source_result(
            "\
type Port = int64 where value >= 1 && value <= 65535

function main() returns int64:
    Port port = 8080 handle error:
        return 80
    return coarsen port
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn simple_type_alias_behaves_like_base_type() {
        let result = check_source_result(
            "\
type UserId = int64

function id(value: UserId) returns UserId:
    return value

function main() returns int64:
    UserId user_id = 42
    return id(user_id)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn coarsen_can_target_refinement_ancestors() {
        let result = check_source_result(
            "\
type NonEmpty = string where string.char_count(value) > 0
type Password = NonEmpty where string.char_count(value) > 8

function main() returns string:
    Password password = \"hunter42!\" handle error:
        return \"\"
    NonEmpty non_empty = coarsen password
    return coarsen non_empty
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn refinement_constraint_must_return_bool() {
        let errors = check_source_errors(
            "\
type Broken = int64 where 42

function main() returns nothing:
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 335),
            "expected E0335, got: {:?}",
            errors
        );
    }

    #[test]
    fn struct_constructor_with_refinement_field_requires_handle() {
        let errors = check_source_errors(
            "\
type Age = int64 where value >= 0 && value < 150

struct User:
    age: Age

function main() returns nothing:
    User user = User(age: 42)
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 316),
            "expected E0316, got: {:?}",
            errors
        );
    }

    #[test]
    fn struct_constructor_with_refinement_field_handle_typechecks() {
        let result = check_source_result(
            "\
type Age = int64 where value >= 0 && value < 150

struct User:
    age: Age

function main() returns nothing:
    User user = User(age: 42) handle error:
        return nothing
    return nothing
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn refined_return_type_accepts_base_expression() {
        let result = check_source_result(
            "\
type Percentage = float64 where value >= 0.0 && value <= 100.0

function calculate_grade(score: int64, total: int64) returns Percentage:
    float64 score_f = float64.from_int64(score)
    float64 total_f = float64.from_int64(total)
    return score_f / total_f * 100.0
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn refined_return_type_still_requires_handle_for_result_values() {
        let errors = check_source_errors(
            "\
type Port = int64 where value >= 1 && value <= 65535

function parse_port(raw: string) returns Port:
    return int64.from_string(raw)
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 316),
            "expected E0316, got: {:?}",
            errors
        );
    }

    #[test]
    fn function_call_into_refinement_parameter_reports_boundary_error() {
        let errors = check_source_errors(
            "\
type Password = string where string.char_count(value) > 8

function create_user(password: Password) returns nothing:
    return nothing

function main() returns nothing:
    create_user(\"hunter42!\")
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 333),
            "expected E0333, got: {:?}",
            errors
        );
    }

    #[test]
    fn bitfield_constructor_with_literals_typechecks() {
        let result = check_source_result(
            "\
bitfield TcpFlags:
    syn: 1 bit
    ack: 1 bit

function main() returns int64:
    TcpFlags flags = TcpFlags(syn: 0, ack: 1)
    return flags.ack
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn bitfield_constructor_with_dynamic_int_requires_handle() {
        let errors = check_source_errors(
            "\
bitfield TcpFlags:
    syn: 1 bit
    ack: 1 bit

function main(bit: int64) returns nothing:
    TcpFlags flags = TcpFlags(syn: bit, ack: 0)
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 316),
            "expected E0316, got: {:?}",
            errors
        );
    }

    #[test]
    fn bitfield_literal_out_of_range_reports_error() {
        let errors = check_source_errors(
            "\
bitfield ColorChannel:
    red: 8 bits
    green: 8 bits

function main() returns nothing:
    ColorChannel color = ColorChannel(red: 300, green: 1)
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 337),
            "expected E0337, got: {:?}",
            errors
        );
    }

    #[test]
    fn bitfield_payload_must_be_list_of_uint8() {
        let errors = check_source_errors(
            "\
bitfield Packet:
    header: 8 bits
    payload: bytes

function main() returns nothing:
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 336),
            "expected E0336, got: {:?}",
            errors
        );
    }

    #[test]
    fn bitfield_binary_roundtrip_typechecks() {
        let result = check_source_result(
            "\
bitfield network IpHeader:
    version: 4 bits
    header_length: 4 bits
    total_length: 16 bits

function main() returns int64:
    IpHeader header = IpHeader(version: 4, header_length: 5, total_length: 500)
    bytes raw = IpHeader.to_bytes(header)
    IpHeader decoded = IpHeader.from_bytes(raw) handle error:
        return 0
    return decoded.total_length
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn bitfield_from_bytes_requires_handle() {
        let errors = check_source_errors(
            "\
bitfield network IpHeader:
    version: 4 bits
    header_length: 4 bits
    total_length: 16 bits

function main() returns nothing:
    bytes raw = IpHeader.to_bytes(IpHeader(version: 4, header_length: 5, total_length: 500))
    IpHeader decoded = IpHeader.from_bytes(raw)
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 316),
            "expected E0316, got: {:?}",
            errors
        );
    }

    #[test]
    fn trace_statement_reads_variable_without_error() {
        let result = check_source_result(
            "\
function main() returns nothing:
    int64 total = 42
    trace total
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn enum_with_explicit_discriminants_typechecks() {
        let result = check_source_result(
            "\
enum IpProtocol:
    icmp = 1
    tcp = 6
    udp = 17

bitfield network IpHeader:
    protocol: 8 bits as IpProtocol

function main() returns nothing:
    IpHeader header = IpHeader(protocol: IpProtocol.tcp)
    bytes raw = IpHeader.to_bytes(header)
    IpHeader decoded = IpHeader.from_bytes(raw) handle error:
        return nothing
    IpProtocol protocol = decoded.protocol
    return nothing
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn duplicate_enum_discriminant_reports_error() {
        let errors = check_source_errors(
            "\
enum Bad:
    first = 1
    second = 1

function main() returns nothing:
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 339),
            "expected E0339, got: {:?}",
            errors
        );
    }

    #[test]
    fn enum_discriminant_requires_unit_variant() {
        let errors = check_source_errors(
            "\
enum Bad:
    named(value: int64) = 1

function main() returns nothing:
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 338),
            "expected E0338, got: {:?}",
            errors
        );
    }
}
