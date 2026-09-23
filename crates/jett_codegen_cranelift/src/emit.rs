mod values;
use jett_mir::move_values::{MoveValuePlan, is_linear, is_string, representation_type};
use jett_runtime::native_abi::values::NativeLeaf;
use std::str::FromStr;

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::immediates::{Ieee32, Ieee64};
use cranelift_codegen::ir::{self, AbiParam, InstBuilder, TrapCode, Value};
use cranelift_codegen::{Context, isa, settings};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{FuncId, Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};
use jett_common::Span;
use jett_hir::{BinaryOp, Expression, ExpressionKind, FunctionId, UnaryOp};
use jett_mir::{
    ControlFlowGraph, Function, Program, Statement, StatementKind, Terminator, TerminatorKind,
};
use jett_types::{Type, TypeId, TypeInterner};
use target_lexicon::{HOST, Triple};

use crate::CodegenError;
use crate::verify::{ScalarKind, VerifiedProgram, scalar_kind, verify_program};

/// One emitted target object and its deterministic defined symbol table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectArtifact {
    pub target: String,
    pub symbols: Vec<String>,
    pub bytes: Vec<u8>,
}

/// Version 1 exported C symbol used by a launcher to enter one AOT Jett object.
pub const JETT_AOT_ENTRY_SYMBOL_V1: &str = "jett_aot_v1_entry";

/// Success status returned by [`JETT_AOT_ENTRY_SYMBOL_V1`].
pub const JETT_AOT_ENTRY_SUCCESS_V1: u32 = 0;

struct DeclaredFunction {
    mir_id: FunctionId,
    native_id: FuncId,
    symbol: String,
    signature: ir::Signature,
    modes: Vec<jett_mir::ParamMode>,
}

/// Sparse original-MIR-index mapping. Only backend-reachable functions have a
/// slot, and every lookup checks the preserved `FunctionId` before returning a
/// Cranelift identity.
struct DeclaredFunctions {
    ordered: Vec<DeclaredFunction>,
    by_mir_index: Vec<Option<usize>>,
}

impl DeclaredFunctions {
    fn new(function_count: usize, reachable_count: usize) -> Self {
        Self {
            ordered: Vec::with_capacity(reachable_count),
            by_mir_index: vec![None; function_count],
        }
    }

    fn insert(&mut self, function: DeclaredFunction) -> Result<(), CodegenError> {
        let index = usize::try_from(function.mir_id.index()).map_err(|_| {
            CodegenError::Backend("MIR function index does not fit this host".to_string())
        })?;
        let Some(slot) = self.by_mir_index.get_mut(index) else {
            return Err(CodegenError::Backend(
                "reachable MIR function index is absent from the sparse declaration table"
                    .to_string(),
            ));
        };
        if slot.is_some() {
            return Err(CodegenError::Backend(
                "reachable MIR function was declared more than once".to_string(),
            ));
        }
        *slot = Some(self.ordered.len());
        self.ordered.push(function);
        Ok(())
    }

    fn get(&self, id: FunctionId) -> Option<&DeclaredFunction> {
        let index = usize::try_from(id.index()).ok()?;
        let declared_index = self.by_mir_index.get(index).copied().flatten()?;
        self.ordered
            .get(declared_index)
            .filter(|function| function.mir_id == id)
    }

    fn iter(&self) -> impl Iterator<Item = &DeclaredFunction> {
        self.ordered.iter()
    }

    fn symbols(&self) -> Vec<String> {
        self.ordered
            .iter()
            .map(|function| function.symbol.clone())
            .collect()
    }
}

/// The one target accepted by this host-only backend slice.
pub fn host_target() -> Triple {
    HOST
}

/// Emit an object for the compiler host target.
pub fn emit_host_object(
    program: &Program,
    types: &TypeInterner,
) -> Result<ObjectArtifact, CodegenError> {
    emit_for_triple(program, types, HOST, None)
}

/// Emit a host object with one exported versioned program-entry wrapper.
///
/// `entry` is an exact checked MIR identity; this API never selects an entry by
/// source name. The wrapper has the target C ABI `uint32_t(void *context)`,
/// forwards its opaque runtime context to a `nothing`-returning Jett function,
/// supplies explicit tokens for checked Stdout parameters, and returns the
/// terminal failure status (or [`JETT_AOT_ENTRY_SUCCESS_V1`]).
pub fn emit_host_program_object(
    program: &Program,
    types: &TypeInterner,
    entry: FunctionId,
) -> Result<ObjectArtifact, CodegenError> {
    emit_for_triple(program, types, HOST, Some(entry))
}

/// Emit an object only when `requested_target` exactly matches the compiler
/// host. Cross-target layouts and linker inputs are intentionally not guessed.
pub fn emit_object_for_target(
    program: &Program,
    types: &TypeInterner,
    requested_target: &str,
) -> Result<ObjectArtifact, CodegenError> {
    let requested =
        Triple::from_str(requested_target).map_err(|error| CodegenError::InvalidTarget {
            target: requested_target.to_string(),
            message: error.to_string(),
        })?;
    if requested != HOST {
        return Err(CodegenError::UnsupportedTarget {
            requested: requested.to_string(),
            supported: HOST.to_string(),
        });
    }
    emit_for_triple(program, types, requested, None)
}

/// Emit a host-target object with an explicit checked MIR program entry.
///
/// Cross-target layouts remain unsupported, matching
/// [`emit_object_for_target`].
pub fn emit_program_object_for_target(
    program: &Program,
    types: &TypeInterner,
    requested_target: &str,
    entry: FunctionId,
) -> Result<ObjectArtifact, CodegenError> {
    let requested =
        Triple::from_str(requested_target).map_err(|error| CodegenError::InvalidTarget {
            target: requested_target.to_string(),
            message: error.to_string(),
        })?;
    if requested != HOST {
        return Err(CodegenError::UnsupportedTarget {
            requested: requested.to_string(),
            supported: HOST.to_string(),
        });
    }
    emit_for_triple(program, types, requested, Some(entry))
}

fn emit_for_triple(
    program: &Program,
    types: &TypeInterner,
    target: Triple,
    entry: Option<FunctionId>,
) -> Result<ObjectArtifact, CodegenError> {
    if let Some(entry) = entry {
        validate_program_entry_contract(program, types, entry)?;
    }
    jett_mir::validate(program).map_err(CodegenError::InvalidMir)?;
    let mut prepared = program.clone();
    jett_mir::prepare_native_sequences(&mut prepared, types);
    let program = &prepared;
    let verified = verify_program(program, types)?;
    if let Some(entry) = entry
        && verified.get(entry).is_none()
    {
        return Err(CodegenError::UnreachableProgramEntry {
            function_id: entry.index(),
        });
    }
    let flag_builder = settings::builder();
    let flags = settings::Flags::new(flag_builder);
    let isa_builder = isa::lookup(target.clone()).map_err(|error| {
        CodegenError::Backend(format!(
            "Cranelift does not support target `{target}`: {error}"
        ))
    })?;
    let isa = isa_builder.finish(flags).map_err(|error| {
        CodegenError::Backend(format!(
            "failed to configure Cranelift target `{target}`: {error}"
        ))
    })?;
    let builder =
        ObjectBuilder::new(isa, b"jett".to_vec(), default_libcall_names()).map_err(|error| {
            CodegenError::Backend(format!("failed to create object module: {error}"))
        })?;
    let mut module = ObjectModule::new(builder);

    let declarations = declare_reachable_functions(&mut module, program, types, &verified)?;
    for declaration in declarations.iter() {
        let function = program_function(program, declaration.mir_id)?;
        let mut context = module.make_context();
        context.func.signature = declaration.signature.clone();
        translate_function(
            &mut module,
            &declarations,
            program,
            function,
            types,
            &declaration.symbol,
            &mut context,
        )?;
        module
            .define_function(declaration.native_id, &mut context)
            .map_err(|error| {
                CodegenError::Backend(format!(
                    "failed to define native function `{}`: {error}; details: {error:?}",
                    declaration.symbol
                ))
            })?;
    }

    if let Some(entry) = entry {
        define_program_entry_wrapper(&mut module, &declarations, entry)?;
    }

    let mut symbols = declarations.symbols();
    if entry.is_some() {
        symbols.push(JETT_AOT_ENTRY_SYMBOL_V1.to_string());
    }
    let bytes = module
        .finish()
        .emit()
        .map_err(|error| CodegenError::Backend(format!("failed to serialize object: {error}")))?;
    Ok(ObjectArtifact {
        target: target.to_string(),
        symbols,
        bytes,
    })
}

fn declare_reachable_functions(
    module: &mut ObjectModule,
    program: &Program,
    types: &TypeInterner,
    verified: &VerifiedProgram,
) -> Result<DeclaredFunctions, CodegenError> {
    let mut declarations =
        DeclaredFunctions::new(program.functions.len(), verified.functions().len());
    for verified_function in verified.functions() {
        let function = program_function(program, verified_function.mir_id)?;
        let signature = signature(module, function, types)?;
        let native_id = module
            .declare_function(&verified_function.symbol, Linkage::Local, &signature)
            .map_err(|error| {
                CodegenError::Backend(format!(
                    "failed to declare native function `{}`: {error}",
                    verified_function.symbol
                ))
            })?;
        declarations.insert(DeclaredFunction {
            mir_id: verified_function.mir_id,
            native_id,
            symbol: verified_function.symbol.clone(),
            signature,
            modes: function.params.iter().map(|p| p.mode).collect(),
        })?;
    }
    Ok(declarations)
}

fn program_function(program: &Program, id: FunctionId) -> Result<&Function, CodegenError> {
    let index = usize::try_from(id.index()).map_err(|_| {
        CodegenError::Backend("MIR function index does not fit this host".to_string())
    })?;
    program
        .functions
        .get(index)
        .filter(|function| function.id == id)
        .ok_or_else(|| {
            CodegenError::Backend(
                "reachable function is absent from the original MIR function table".to_string(),
            )
        })
}

fn validate_program_entry_contract(
    program: &Program,
    types: &TypeInterner,
    entry: FunctionId,
) -> Result<(), CodegenError> {
    let mut matches = program
        .functions
        .iter()
        .filter(|function| function.id == entry);
    let Some(function) = matches.next() else {
        return Err(CodegenError::MissingProgramEntry {
            function_id: entry.index(),
        });
    };
    if matches.next().is_some() {
        return Err(CodegenError::DuplicateProgramEntry {
            function_id: entry.index(),
        });
    }
    if function.params.iter().any(|p| p.ty != TypeInterner::STDOUT) {
        return Err(CodegenError::IncompatibleProgramEntry {
            function_id: entry.index(),
            message: format!("expected no parameters, found {}", function.params.len()),
        });
    }

    let type_count = u32::try_from(types.len()).unwrap_or(u32::MAX);
    let returns_nothing = function.return_type.index() < type_count
        && matches!(types.resolve(function.return_type), Type::Nothing);
    if !returns_nothing {
        let return_type = if function.return_type.index() < type_count {
            types.type_name(function.return_type)
        } else {
            format!("<invalid type {}>", function.return_type.index())
        };
        return Err(CodegenError::IncompatibleProgramEntry {
            function_id: entry.index(),
            message: format!("expected return type `nothing`, found `{return_type}`"),
        });
    }
    Ok(())
}

fn define_program_entry_wrapper(
    module: &mut ObjectModule,
    declarations: &DeclaredFunctions,
    entry: FunctionId,
) -> Result<(), CodegenError> {
    let entry_function =
        declarations
            .get(entry)
            .ok_or_else(|| CodegenError::UnreachableProgramEntry {
                function_id: entry.index(),
            })?;
    if module.get_name(JETT_AOT_ENTRY_SYMBOL_V1).is_some() {
        return Err(CodegenError::DuplicateSymbol(
            JETT_AOT_ENTRY_SYMBOL_V1.to_string(),
        ));
    }

    let signature = program_entry_signature(module);
    let wrapper_id = module
        .declare_function(JETT_AOT_ENTRY_SYMBOL_V1, Linkage::Export, &signature)
        .map_err(|error| {
            CodegenError::Backend(format!(
                "failed to declare program entry `{JETT_AOT_ENTRY_SYMBOL_V1}`: {error}"
            ))
        })?;

    let mut context = module.make_context();
    context.func.signature = signature;
    translate_program_entry_wrapper(module, entry_function, entry, &mut context)?;

    module
        .define_function(wrapper_id, &mut context)
        .map_err(|error| {
            CodegenError::Backend(format!(
                "failed to define program entry `{JETT_AOT_ENTRY_SYMBOL_V1}`: {error}; details: {error:?}"
            ))
        })?;
    Ok(())
}

fn program_entry_signature(module: &ObjectModule) -> ir::Signature {
    let mut signature = module.make_signature();
    signature.params.push(runtime_context_abi_param(module));
    signature.returns.push(AbiParam::new(ir::types::I32));
    signature
}

fn runtime_context_abi_param(module: &ObjectModule) -> AbiParam {
    AbiParam::new(module.target_config().pointer_type())
}

fn translate_program_entry_wrapper(
    module: &mut ObjectModule,
    entry_function: &DeclaredFunction,
    entry: FunctionId,
    context: &mut Context,
) -> Result<(), CodegenError> {
    let mut builder_context = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    let runtime_context = builder
        .block_params(block)
        .first()
        .copied()
        .ok_or_else(|| CodegenError::IncompatibleProgramEntry {
            function_id: entry.index(),
            message: "exported entry wrapper is missing its runtime-context pointer".to_string(),
        })?;
    let entry_reference = module.declare_func_in_func(entry_function.native_id, builder.func);
    let mut args = vec![runtime_context];
    if entry_function.signature.params.len() > 1 {
        let leaf = crate::values::declare_leaf(module, NativeLeaf::GrantStdout)?;
        let leaf = module.declare_func_in_func(leaf, builder.func);
        let call = builder.ins().call(leaf, &[runtime_context]);
        let authority = builder.func.dfg.inst_results(call)[0];
        args.extend(std::iter::repeat_n(
            authority,
            entry_function.signature.params.len() - 1,
        ));
    }
    builder.ins().call(entry_reference, &args);
    let leaf = crate::values::declare_leaf(module, NativeLeaf::Status)?;
    let leaf = module.declare_func_in_func(leaf, builder.func);
    let call = builder.ins().call(leaf, &[runtime_context]);
    let status = builder.func.dfg.inst_results(call)[0];
    builder.ins().return_(&[status]);
    builder.seal_all_blocks();
    builder.finalize();
    Ok(())
}

fn signature(
    module: &ObjectModule,
    function: &Function,
    types: &TypeInterner,
) -> Result<ir::Signature, CodegenError> {
    let mut signature = module.make_signature();
    signature.params.push(runtime_context_abi_param(module));
    for parameter in &function.params {
        if let Some(ty) = clif_type(types, parameter.ty, "function parameter")? {
            signature.params.push(AbiParam::new(ty));
        }
    }
    if let Some(ty) = clif_type(types, function.return_type, "function return")? {
        signature.returns.push(AbiParam::new(ty));
    }
    Ok(signature)
}

fn clif_type(
    types: &TypeInterner,
    ty: TypeId,
    context: &str,
) -> Result<Option<ir::Type>, CodegenError> {
    let ty = match scalar_kind(types, ty, context)? {
        ScalarKind::SignedInteger(8) | ScalarKind::UnsignedInteger(8) | ScalarKind::Bool => {
            Some(ir::types::I8)
        }
        ScalarKind::SignedInteger(16) | ScalarKind::UnsignedInteger(16) => Some(ir::types::I16),
        ScalarKind::SignedInteger(32) | ScalarKind::UnsignedInteger(32) => Some(ir::types::I32),
        ScalarKind::SignedInteger(64) | ScalarKind::UnsignedInteger(64) => Some(ir::types::I64),
        ScalarKind::Float(32) => Some(ir::types::F32),
        ScalarKind::Float(64) => Some(ir::types::F64),
        ScalarKind::Nothing => None,
        ScalarKind::String
        | ScalarKind::Bytes
        | ScalarKind::Sum
        | ScalarKind::List
        | ScalarKind::Set
        | ScalarKind::Map
        | ScalarKind::Struct
        | ScalarKind::Enum
        | ScalarKind::Bitfield
        | ScalarKind::Stdout => Some(ir::types::I64),
        ScalarKind::SignedInteger(bits)
        | ScalarKind::UnsignedInteger(bits)
        | ScalarKind::Float(bits) => {
            return Err(CodegenError::UnsupportedType {
                type_name: format!("{bits}-bit scalar"),
                context: context.to_string(),
            });
        }
    };
    Ok(ty)
}

fn translate_function(
    module: &mut ObjectModule,
    declarations: &DeclaredFunctions,
    program: &Program,
    function: &Function,
    types: &TypeInterner,
    symbol: &str,
    context: &mut Context,
) -> Result<(), CodegenError> {
    let mut builder_context = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_context);
    let blocks = function
        .blocks
        .iter()
        .map(|_| builder.create_block())
        .collect::<Vec<_>>();
    let entry = block_for(&blocks, function.entry.index(), function.span, symbol)?;
    builder.append_block_params_for_function_params(entry);

    let runtime_context = builder.declare_var(module.target_config().pointer_type());
    let ownership = MoveValuePlan::analyze(program, function, types)
        .map_err(|message| contract_error(symbol, function.span, message))?;
    let local_slots = function
        .locals
        .iter()
        .map(|l| {
            if ownership.owned_locals.contains(&(l.id.index() as usize)) {
                Some(builder.create_sized_stack_slot(ir::StackSlotData::new(
                    ir::StackSlotKind::ExplicitSlot,
                    8,
                    3,
                )))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let temporary_slots = (0..ownership.temporary_slots)
        .map(|_| {
            builder.create_sized_stack_slot(ir::StackSlotData::new(
                ir::StackSlotKind::ExplicitSlot,
                8,
                3,
            ))
        })
        .collect::<Vec<_>>();
    let failure_block = builder.create_block();

    let mut variables = Vec::with_capacity(function.locals.len());
    for local in &function.locals {
        variables
            .push(clif_type(types, local.ty, "function local")?.map(|ty| builder.declare_var(ty)));
    }

    let control_flow = ControlFlowGraph::analyze(function).map_err(|errors| {
        let details = errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ");
        contract_error(
            symbol,
            function.span,
            format!("cannot analyze MIR control flow: {details}"),
        )
    })?;
    for block_id in control_flow.reverse_postorder() {
        let block_index = usize::try_from(block_id.index()).map_err(|_| {
            contract_error(symbol, function.span, "block index does not fit this host")
        })?;
        let block = function.blocks.get(block_index).ok_or_else(|| {
            contract_error(symbol, function.span, "reachable block is absent from MIR")
        })?;
        let native_block = block_for(&blocks, block.id.index(), function.span, symbol)?;
        builder.switch_to_block(native_block);
        if block.id == function.entry {
            let incoming = builder.block_params(native_block).to_vec();
            let context_value = incoming.first().copied().ok_or_else(|| {
                contract_error(
                    symbol,
                    function.span,
                    "missing hidden native runtime-context parameter",
                )
            })?;
            builder
                .try_def_var(runtime_context, context_value)
                .map_err(|error| {
                    contract_error(
                        symbol,
                        function.span,
                        format!("cannot bind native runtime context: {error}"),
                    )
                })?;
            let zero = builder.ins().iconst(ir::types::I64, 0);
            for slot in local_slots.iter().flatten().chain(&temporary_slots) {
                builder.ins().stack_store(zero, *slot, 0);
            }
            let mut incoming_index = 1_usize;
            for parameter in &function.params {
                let variable =
                    variable_for(&variables, parameter.local.index(), parameter.span, symbol)?;
                if let Some(variable) = variable {
                    let value = incoming.get(incoming_index).copied().ok_or_else(|| {
                        contract_error(symbol, parameter.span, "missing native function parameter")
                    })?;
                    builder.try_def_var(variable, value).map_err(|error| {
                        contract_error(
                            symbol,
                            parameter.span,
                            format!("cannot bind native function parameter: {error}"),
                        )
                    })?;
                    incoming_index += 1;
                }
            }
        }

        let mut translator = Translator {
            builder: &mut builder,
            module,
            declarations,
            blocks: &blocks,
            variables: &variables,
            runtime_context,
            types,
            symbol,
            local_types: &function.locals,
            local_slots: &local_slots,
            temporary_slots: &temporary_slots,
            next_temporary: 0,
            failure_block,
        };
        if block.id == function.entry {
            for parameter in &function.params {
                if let Some(slot) = local_slots[parameter.local.index() as usize] {
                    let v = translator
                        .builder
                        .use_var(variables[parameter.local.index() as usize].unwrap());
                    let owned = if is_linear(types, parameter.ty) {
                        v
                    } else {
                        translator.leaf(NativeLeaf::Retain, &[v], true)?
                    };
                    translator.builder.ins().stack_store(owned, slot, 0);
                }
            }
        }
        translator.drop_dead_locals(&ownership.live_in[block_index])?;
        for (index, statement) in block.statements.iter().enumerate() {
            translator.statement(statement)?;
            translator.drop_temporaries()?;
            translator.drop_dead_locals(&ownership.live_after_statement[block_index][index])?;
        }
        translator.terminator(&block.terminator)?;
    }

    builder.switch_to_block(failure_block);
    let mut translator = Translator {
        builder: &mut builder,
        module,
        declarations,
        blocks: &blocks,
        variables: &variables,
        runtime_context,
        types,
        symbol,
        local_types: &function.locals,
        local_slots: &local_slots,
        temporary_slots: &temporary_slots,
        next_temporary: temporary_slots.len(),
        failure_block,
    };
    translator.drop_all()?;
    let results = match clif_type(types, function.return_type, "failure return")? {
        None => vec![],
        Some(ir::types::F32) => vec![translator.builder.ins().f32const(0.0)],
        Some(ir::types::F64) => vec![translator.builder.ins().f64const(0.0)],
        Some(t) => vec![translator.builder.ins().iconst(t, 0)],
    };
    translator.builder.ins().return_(&results);
    builder.seal_all_blocks();
    builder.finalize();
    Ok(())
}

#[derive(Clone, Copy)]
enum LoweredValue {
    Scalar(Value),
    Owned(Value, ir::StackSlot),
    Nothing,
}

struct Translator<'a, 'builder> {
    builder: &'a mut FunctionBuilder<'builder>,
    module: &'a mut ObjectModule,
    declarations: &'a DeclaredFunctions,
    blocks: &'a [ir::Block],
    variables: &'a [Option<Variable>],
    runtime_context: Variable,
    types: &'a TypeInterner,
    symbol: &'a str,
    local_types: &'a [jett_mir::Local],
    local_slots: &'a [Option<ir::StackSlot>],
    temporary_slots: &'a [ir::StackSlot],
    next_temporary: usize,
    failure_block: ir::Block,
}

impl Translator<'_, '_> {
    fn statement(&mut self, statement: &Statement) -> Result<(), CodegenError> {
        match &statement.kind {
            StatementKind::IterationBorrow { .. } => Ok(()),
            StatementKind::SequenceLength { source, target }
            | StatementKind::SequenceGet { source, target, .. } => {
                let ty = self.local_types[source.index() as usize].ty;
                let source_expr = Expression {
                    kind: ExpressionKind::Local(*source),
                    ty,
                    span: statement.span,
                };
                let value = self.argument(&source_expr, true)?;
                let value = self.scalar(value, statement.span)?;
                let set = matches!(self.types.resolve(ty), Type::Set(_));
                let map = matches!(self.types.resolve(ty), Type::Map(..));
                let output = if let StatementKind::SequenceGet {
                    index,
                    consume,
                    part,
                    ..
                } = statement.kind
                {
                    let index = self
                        .builder
                        .use_var(self.variables[index.index() as usize].unwrap());
                    let leaf = if map {
                        match (part, consume) {
                            (jett_mir::SequencePart::Key, true) => NativeLeaf::MapKeyTake,
                            (jett_mir::SequencePart::Key, false) => NativeLeaf::MapKeyClone,
                            (jett_mir::SequencePart::Value, true) => NativeLeaf::MapValueTake,
                            (jett_mir::SequencePart::Value, false) => NativeLeaf::MapValueClone,
                            _ => {
                                return Err(
                                    self.unsupported(statement.span, "map iteration projection")
                                );
                            }
                        }
                    } else if set {
                        if consume {
                            NativeLeaf::SetElementTake
                        } else {
                            NativeLeaf::SetElementClone
                        }
                    } else if consume {
                        NativeLeaf::ListElementTake
                    } else {
                        NativeLeaf::ListElementClone
                    };
                    let bits = self.leaf(leaf, &[value, index], true)?;
                    self.unpack_payload(
                        bits,
                        self.local_types[target.index() as usize].ty,
                        statement.span,
                    )?
                } else {
                    let leaf = if map {
                        NativeLeaf::MapLength
                    } else if set {
                        NativeLeaf::SetLength
                    } else {
                        NativeLeaf::ListLength
                    };
                    LoweredValue::Scalar(self.leaf(leaf, &[value], true)?)
                };
                self.define_local(*target, output, statement.span)
            }

            StatementKind::SumTag { source, target } => {
                let slot = self.local_slots[source.index() as usize]
                    .ok_or_else(|| self.unsupported(statement.span, "sum view tag place"))?;
                let v = self.builder.ins().stack_load(ir::types::I64, slot, 0);
                let tag = self.leaf(NativeLeaf::SumTag, &[v], true)?;
                let flag = self.builder.ins().ireduce(ir::types::I8, tag);
                self.define_local(*target, LoweredValue::Scalar(flag), statement.span)
            }
            StatementKind::SumTake {
                source,
                target,
                success,
            } => {
                let slot = self.local_slots[source.index() as usize]
                    .ok_or_else(|| self.unsupported(statement.span, "take borrowed sum"))?;
                let v = self.builder.ins().stack_load(ir::types::I64, slot, 0);
                let tag = self
                    .builder
                    .ins()
                    .iconst(ir::types::I32, i64::from(*success));
                let bits = self.leaf(NativeLeaf::SumTake, &[v, tag], true)?;
                self.clear_slot(slot);
                let ty = self.local_types[target.index() as usize].ty;
                let value = self.unpack_payload(bits, ty, statement.span)?;
                self.define_local(*target, value, statement.span)
            }

            StatementKind::Let { local, value } => {
                let value = self.expression(value)?;
                self.define_local(*local, value, statement.span)
            }
            StatementKind::Assign { target, value } => {
                let ExpressionKind::Local(local) = &target.kind else {
                    return Err(self.unsupported(statement.span, "non-local assignment"));
                };
                let value = self.expression(value)?;
                self.define_local(*local, value, statement.span)
            }
            StatementKind::Evaluate(value) => {
                self.expression(value)?;
                Ok(())
            }
            StatementKind::HandleDefault(_) => {
                Err(self.unsupported(statement.span, "handle default"))
            }
            StatementKind::Assert { .. } => Err(self.unsupported(statement.span, "assert")),
            StatementKind::Trace(local) => {
                let id = local.index() as usize;
                let local = &self.local_types[id];
                let variable = variable_for(
                    self.variables,
                    local.id.index(),
                    statement.span,
                    self.symbol,
                )?
                .ok_or_else(|| self.unsupported(statement.span, "trace without a scalar local"))?;
                let value = self.builder.try_use_var(variable).map_err(|error| {
                    contract_error(
                        self.symbol,
                        statement.span,
                        format!("cannot trace native local: {error}"),
                    )
                })?;
                let prefix = format!("trace {}: int64 = ", local.name);
                let length = i64::try_from(prefix.len())
                    .map_err(|_| self.unsupported(statement.span, "trace label length"))?;
                let item = self
                    .module
                    .declare_anonymous_data(false, false)
                    .map_err(|error| CodegenError::Backend(error.to_string()))?;
                let mut description = cranelift_module::DataDescription::new();
                description.define(prefix.into_bytes().into_boxed_slice());
                self.module
                    .define_data(item, &description)
                    .map_err(|error| CodegenError::Backend(error.to_string()))?;
                let reference = self.module.declare_data_in_func(item, self.builder.func);
                let pointer = self.builder.ins().global_value(ir::types::I64, reference);
                let length = self.builder.ins().iconst(ir::types::I64, length);
                self.leaf(NativeLeaf::TraceInt64, &[pointer, length, value], true)?;
                Ok(())
            }
            StatementKind::Breakpoint(_) => Err(self.unsupported(statement.span, "breakpoint")),
        }
    }

    fn terminator(&mut self, terminator: &Terminator) -> Result<(), CodegenError> {
        match &terminator.kind {
            TerminatorKind::Return(value) => {
                let mut result = match value {
                    Some(v) => self.expression(v)?,
                    None => LoweredValue::Nothing,
                };
                if value.as_ref().is_some_and(|v| is_string(self.types, v.ty)) {
                    let v = self.scalar(result, terminator.span)?;
                    result = LoweredValue::Scalar(self.leaf(NativeLeaf::Retain, &[v], true)?);
                }
                if let LoweredValue::Owned(v, slot) = result {
                    self.clear_slot(slot);
                    result = LoweredValue::Scalar(v);
                }
                self.drop_all()?;
                match result {
                    LoweredValue::Scalar(v) | LoweredValue::Owned(v, _) => {
                        self.builder.ins().return_(&[v]);
                    }
                    LoweredValue::Nothing => {
                        self.builder.ins().return_(&[]);
                    }
                }
                Ok(())
            }
            TerminatorKind::Goto(target) => {
                self.drop_temporaries()?;
                let target = block_for(self.blocks, target.index(), terminator.span, self.symbol)?;
                self.builder.ins().jump(target, &[]);
                Ok(())
            }
            TerminatorKind::Branch {
                condition,
                then_block,
                else_block,
            } => {
                let lowered_condition = self.expression(condition)?;
                let condition = self.scalar(lowered_condition, condition.span)?;
                self.drop_temporaries()?;
                let then_block = block_for(
                    self.blocks,
                    then_block.index(),
                    terminator.span,
                    self.symbol,
                )?;
                let else_block = block_for(
                    self.blocks,
                    else_block.index(),
                    terminator.span,
                    self.symbol,
                )?;
                self.builder
                    .ins()
                    .brif(condition, then_block, &[], else_block, &[]);
                Ok(())
            }
            TerminatorKind::Unreachable => {
                self.builder.ins().trap(TrapCode::unwrap_user(1));
                Ok(())
            }
            TerminatorKind::Respond(_) => Err(self.unsupported(terminator.span, "actor response")),
            TerminatorKind::Switch {
                scrutinee,
                variants,
                otherwise,
            } => {
                let lowered = self.expression(scrutinee)?;
                let handle = self.scalar(lowered, scrutinee.span)?;
                let zero = self.builder.ins().iconst(ir::types::I64, 0);
                let tag = self.leaf(NativeLeaf::StructField, &[handle, zero], true)?;
                let switch_temporaries = self.next_temporary;
                for (variant, target, bindings) in variants {
                    let match_tag =
                        self.builder
                            .ins()
                            .icmp_imm(IntCC::Equal, tag, i64::from(variant.index()));
                    let target =
                        block_for(self.blocks, target.index(), terminator.span, self.symbol)?;
                    let selected = self.builder.create_block();
                    let next = self.builder.create_block();
                    self.builder.ins().brif(match_tag, selected, &[], next, &[]);
                    self.builder.switch_to_block(selected);
                    for (index, binding) in bindings.iter().enumerate() {
                        let field = self.builder.ins().iconst(ir::types::I64, index as i64 + 1);
                        let bits = self.leaf(NativeLeaf::StructTake, &[handle, field], true)?;
                        let ty = self.local_types[binding.index() as usize].ty;
                        let value = self.unpack_payload(bits, ty, terminator.span)?;
                        self.define_local(*binding, value, terminator.span)?;
                    }
                    self.drop_temporaries()?;
                    self.builder.ins().jump(target, &[]);
                    self.next_temporary = switch_temporaries;
                    self.builder.switch_to_block(next);
                }
                self.drop_temporaries()?;
                if let Some(otherwise) = otherwise {
                    let target =
                        block_for(self.blocks, otherwise.index(), terminator.span, self.symbol)?;
                    self.builder.ins().jump(target, &[]);
                } else {
                    self.builder.ins().trap(TrapCode::unwrap_user(1));
                }
                Ok(())
            }
            TerminatorKind::ForEach { .. } => {
                Err(self.unsupported(terminator.span, "for-each loop"))
            }
            TerminatorKind::ReflectedTypeDispatch { .. } => {
                Err(self.unsupported(terminator.span, "reflected type dispatch"))
            }
        }
    }

    fn expression(&mut self, expression: &Expression) -> Result<LoweredValue, CodegenError> {
        let kind = scalar_kind(self.types, expression.ty, "native expression")?;
        match &expression.kind {
            ExpressionKind::Int(value) => {
                let ty = self.required_clif_type(expression.ty, expression.span)?;
                let immediate = integer_immediate(*value, kind).ok_or_else(|| {
                    contract_error(
                        self.symbol,
                        expression.span,
                        "integer literal is out of range",
                    )
                })?;
                Ok(LoweredValue::Scalar(
                    self.builder.ins().iconst(ty, immediate),
                ))
            }
            ExpressionKind::Float(value) => {
                let value = match kind {
                    ScalarKind::Float(32) => self
                        .builder
                        .ins()
                        .f32const(Ieee32::with_float(*value as f32)),
                    ScalarKind::Float(64) => {
                        self.builder.ins().f64const(Ieee64::with_float(*value))
                    }
                    _ => {
                        return Err(contract_error(
                            self.symbol,
                            expression.span,
                            "float literal has a non-float type",
                        ));
                    }
                };
                Ok(LoweredValue::Scalar(value))
            }
            ExpressionKind::Bool(value) => Ok(LoweredValue::Scalar(
                self.builder.ins().iconst(ir::types::I8, i64::from(*value)),
            )),
            ExpressionKind::Nothing => Ok(LoweredValue::Nothing),
            ExpressionKind::Local(local) => {
                let variable =
                    variable_for(self.variables, local.index(), expression.span, self.symbol)?;
                let Some(variable) = variable else {
                    return Ok(LoweredValue::Nothing);
                };
                if let Some(slot) = self.local_slots[local.index() as usize] {
                    let v = self.builder.ins().stack_load(ir::types::I64, slot, 0);
                    if is_linear(self.types, expression.ty) {
                        self.clear_slot(slot);
                        return self.own_linear(v);
                    }
                    let v = self.leaf(NativeLeaf::Retain, &[v], true)?;
                    return self.own(v);
                }
                let value = self.builder.try_use_var(variable).map_err(|error| {
                    contract_error(
                        self.symbol,
                        expression.span,
                        format!("cannot read native local: {error}"),
                    )
                })?;
                Ok(LoweredValue::Scalar(value))
            }
            ExpressionKind::FunctionRef(_) => {
                Err(self.unsupported(expression.span, "function value"))
            }
            ExpressionKind::Unary { op, value } => {
                let lowered_value = self.expression(value)?;
                let value = self.scalar(lowered_value, value.span)?;
                let value = match op {
                    UnaryOp::Not => self.builder.ins().bxor_imm(value, 1),
                    UnaryOp::Negate if kind.is_integer() => self.builder.ins().ineg(value),
                    UnaryOp::Negate => self.builder.ins().fneg(value),
                };
                Ok(LoweredValue::Scalar(value))
            }
            ExpressionKind::Binary { left, op, right } => {
                let lowered_left = self.expression(left)?;
                let left_value = self.scalar(lowered_left, left.span)?;
                let operand_kind = scalar_kind(self.types, left.ty, "binary operand")?;
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    let value = self.short_circuit_boolean(
                        left_value,
                        *op,
                        right,
                        operand_kind,
                        expression.span,
                    )?;
                    return Ok(LoweredValue::Scalar(value));
                }
                let lowered_right = self.expression(right)?;
                let right_value = self.scalar(lowered_right, right.span)?;
                if operand_kind == ScalarKind::String {
                    let value = self.leaf(NativeLeaf::Equal, &[left_value, right_value], true)?;
                    let value = self.builder.ins().ireduce(ir::types::I8, value);
                    let value = if *op == BinaryOp::NotEqual {
                        self.builder.ins().bxor_imm(value, 1)
                    } else {
                        value
                    };
                    return Ok(LoweredValue::Scalar(value));
                }
                if operand_kind == ScalarKind::Enum {
                    let zero = self.builder.ins().iconst(ir::types::I64, 0);
                    let left_tag = self.leaf(NativeLeaf::StructField, &[left_value, zero], true)?;
                    let right_tag =
                        self.leaf(NativeLeaf::StructField, &[right_value, zero], true)?;
                    let condition = if *op == BinaryOp::Equal {
                        IntCC::Equal
                    } else {
                        IntCC::NotEqual
                    };
                    let value = self.builder.ins().icmp(condition, left_tag, right_tag);
                    return Ok(LoweredValue::Scalar(value));
                }
                let value =
                    self.binary(left_value, *op, right_value, operand_kind, expression.span)?;
                Ok(LoweredValue::Scalar(value))
            }
            ExpressionKind::Call {
                function,
                args,
                evaluation_order,
            } => self.call(*function, args, evaluation_order, expression),
            ExpressionKind::Comptime(_) => {
                Err(self.unsupported(expression.span, "unbaked comptime expression"))
            }
            ExpressionKind::View(value) => self.argument(value, true),
            ExpressionKind::Clone(value) if is_linear(self.types, value.ty) => {
                let borrowed = self.argument(value, true)?;
                let v = self.scalar(borrowed, value.span)?;
                let leaf = match self
                    .types
                    .resolve(representation_type(self.types, value.ty))
                {
                    Type::Bytes => NativeLeaf::BytesClone,
                    Type::List(_) => NativeLeaf::ListClone,
                    Type::Set(_) => NativeLeaf::SetClone,
                    Type::Map(..) => NativeLeaf::MapClone,
                    Type::Struct(_) => NativeLeaf::StructClone,
                    Type::Enum(_) => NativeLeaf::StructClone,
                    Type::Bitfield(_) => NativeLeaf::StructClone,
                    _ => NativeLeaf::SumClone,
                };
                let cloned = self.leaf(leaf, &[v], true)?;
                self.own_linear(cloned)
            }
            ExpressionKind::Clone(value) => self.expression(value),
            ExpressionKind::String(text) => self.literal(text),
            ExpressionKind::Intrinsic {
                intrinsic,
                args,
                evaluation_order,
                ..
            } => self.intrinsic(
                *intrinsic,
                args,
                evaluation_order,
                expression.ty,
                expression.span,
            ),
            ExpressionKind::IndirectCall { .. } => {
                Err(self.unsupported(expression.span, "indirect call"))
            }
            ExpressionKind::StructConstruct {
                fields,
                evaluation_order,
                ..
            } => self.construct_struct(fields, evaluation_order, expression.span),
            ExpressionKind::BitfieldConstruct {
                fields,
                evaluation_order,
                ..
            } => self.construct_struct(fields, evaluation_order, expression.span),
            ExpressionKind::MachineConstruct { .. } => {
                Err(self.unsupported(expression.span, "machine construction"))
            }
            ExpressionKind::MachineTransition { .. } => {
                Err(self.unsupported(expression.span, "machine transition"))
            }
            ExpressionKind::ListConstruct { elements } => {
                self.construct_list(elements, expression.ty, expression.span)
            }
            ExpressionKind::MapConstruct { entries } => {
                self.construct_map(entries, expression.ty, expression.span)
            }
            ExpressionKind::ResultOk(value) | ExpressionKind::OptionalSome(value) => {
                self.construct_sum(true, Some(value), expression.span)
            }
            ExpressionKind::ResultFail(value) => {
                self.construct_sum(false, Some(value), expression.span)
            }
            ExpressionKind::OptionalNone => self.construct_sum(false, None, expression.span),
            ExpressionKind::Handle { .. } => {
                Err(self.unsupported(expression.span, "failure handler"))
            }
            ExpressionKind::EnumConstruct {
                variant, payloads, ..
            } => self.construct_enum(*variant, payloads, expression.span),
            ExpressionKind::StringInterpolation(segments) => {
                self.interpolate(segments, expression.span)
            }
            ExpressionKind::Declassify(_) | ExpressionKind::Coarsen(_) => {
                Err(self.unsupported(expression.span, "secret operation"))
            }
            ExpressionKind::StateIs { .. } => {
                Err(self.unsupported(expression.span, "machine state test"))
            }
            ExpressionKind::Run(_) | ExpressionKind::Join(_) | ExpressionKind::Cancel(_) => {
                Err(self.unsupported(expression.span, "task operation"))
            }
            ExpressionKind::InlineFunction { .. } => {
                Err(self.unsupported(expression.span, "inline function"))
            }
            ExpressionKind::ActorSpawn { .. } | ExpressionKind::ActorMessage { .. } => {
                Err(self.unsupported(expression.span, "actor operation"))
            }
            ExpressionKind::Field { base, field, .. } => {
                self.struct_field(base, field.index(), expression.ty, expression.span)
            }
        }
    }

    fn call(
        &mut self,
        function: jett_mir::FunctionId,
        args: &[Expression],
        evaluation_order: &[usize],
        expression: &Expression,
    ) -> Result<LoweredValue, CodegenError> {
        let invalid_order = || {
            contract_error(
                self.symbol,
                expression.span,
                "call evaluation order is not a permutation",
            )
        };
        let modes = self
            .declarations
            .get(function)
            .ok_or_else(invalid_order)?
            .modes
            .clone();
        let indexed = args.iter().enumerate().collect::<Vec<_>>();
        let evaluated = reordered_map(
            &indexed,
            evaluation_order,
            |(index, argument)| {
                if modes[*index] == jett_mir::ParamMode::Owned
                    && is_linear(self.types, argument.ty)
                    && matches!(argument.kind, ExpressionKind::View(_))
                {
                    let cloned = Expression {
                        kind: ExpressionKind::Clone(Box::new((*argument).clone())),
                        ty: argument.ty,
                        span: argument.span,
                    };
                    self.expression(&cloned)
                } else {
                    self.argument(argument, modes[*index] == jett_mir::ParamMode::View)
                }
            },
            invalid_order,
        )?;
        let runtime_context = self
            .builder
            .try_use_var(self.runtime_context)
            .map_err(|error| {
                contract_error(
                    self.symbol,
                    expression.span,
                    format!("cannot read native runtime context: {error}"),
                )
            })?;
        let mut native_args = Vec::with_capacity(evaluated.len() + 1);
        native_args.push(runtime_context);
        for (index, (argument, value)) in args.iter().zip(evaluated).enumerate() {
            match value {
                LoweredValue::Scalar(value) => native_args.push(value),
                LoweredValue::Owned(value, slot) => {
                    if modes[index] == jett_mir::ParamMode::Owned
                        && is_linear(self.types, argument.ty)
                    {
                        self.clear_slot(slot);
                    }
                    native_args.push(value);
                }
                LoweredValue::Nothing => {
                    if scalar_kind(self.types, argument.ty, "nothing argument")?
                        != ScalarKind::Nothing
                    {
                        return Err(contract_error(
                            self.symbol,
                            argument.span,
                            "non-nothing argument produced no native value",
                        ));
                    }
                }
            }
        }
        let function_id = self
            .declarations
            .get(function)
            .map(|function| function.native_id)
            .ok_or_else(|| {
                contract_error(
                    self.symbol,
                    expression.span,
                    "direct call target is absent from the reachable function table",
                )
            })?;
        let reference = self
            .module
            .declare_func_in_func(function_id, self.builder.func);
        let call = self.builder.ins().call(reference, &native_args);
        let results = self.builder.func.dfg.inst_results(call).to_vec();
        self.check_failure()?;
        if scalar_kind(self.types, expression.ty, "call result")? == ScalarKind::Nothing {
            if results.is_empty() {
                Ok(LoweredValue::Nothing)
            } else {
                Err(contract_error(
                    self.symbol,
                    expression.span,
                    "nothing call unexpectedly produced a native value",
                ))
            }
        } else {
            let value = results.first().copied().ok_or_else(|| {
                contract_error(
                    self.symbol,
                    expression.span,
                    "value-returning call produced no native value",
                )
            })?;
            if is_linear(self.types, expression.ty) {
                self.own_linear(value)
            } else if is_string(self.types, expression.ty) {
                self.own(value)
            } else {
                Ok(LoweredValue::Scalar(value))
            }
        }
    }

    fn binary(
        &mut self,
        left: Value,
        op: BinaryOp,
        right: Value,
        kind: ScalarKind,
        span: Span,
    ) -> Result<Value, CodegenError> {
        let value = match kind {
            ScalarKind::SignedInteger(_) => match op {
                BinaryOp::Add => self.builder.ins().iadd(left, right),
                BinaryOp::Subtract => self.builder.ins().isub(left, right),
                BinaryOp::Multiply => self.builder.ins().imul(left, right),
                BinaryOp::Divide => self.signed_division(left, right),
                BinaryOp::Modulo => self.signed_remainder(left, right),
                BinaryOp::Equal => self.builder.ins().icmp(IntCC::Equal, left, right),
                BinaryOp::NotEqual => self.builder.ins().icmp(IntCC::NotEqual, left, right),
                BinaryOp::Less => self.builder.ins().icmp(IntCC::SignedLessThan, left, right),
                BinaryOp::Greater => self
                    .builder
                    .ins()
                    .icmp(IntCC::SignedGreaterThan, left, right),
                BinaryOp::LessEqual => {
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedLessThanOrEqual, left, right)
                }
                BinaryOp::GreaterEqual => {
                    self.builder
                        .ins()
                        .icmp(IntCC::SignedGreaterThanOrEqual, left, right)
                }
                BinaryOp::And | BinaryOp::Or => {
                    return Err(contract_error(
                        self.symbol,
                        span,
                        "boolean operator has a signed integer operand",
                    ));
                }
            },
            ScalarKind::UnsignedInteger(_) => match op {
                BinaryOp::Add => self.builder.ins().iadd(left, right),
                BinaryOp::Subtract => self.builder.ins().isub(left, right),
                BinaryOp::Multiply => self.builder.ins().imul(left, right),
                BinaryOp::Divide => self.builder.ins().udiv(left, right),
                BinaryOp::Modulo => self.builder.ins().urem(left, right),
                BinaryOp::Equal => self.builder.ins().icmp(IntCC::Equal, left, right),
                BinaryOp::NotEqual => self.builder.ins().icmp(IntCC::NotEqual, left, right),
                BinaryOp::Less => self
                    .builder
                    .ins()
                    .icmp(IntCC::UnsignedLessThan, left, right),
                BinaryOp::Greater => {
                    self.builder
                        .ins()
                        .icmp(IntCC::UnsignedGreaterThan, left, right)
                }
                BinaryOp::LessEqual => {
                    self.builder
                        .ins()
                        .icmp(IntCC::UnsignedLessThanOrEqual, left, right)
                }
                BinaryOp::GreaterEqual => {
                    self.builder
                        .ins()
                        .icmp(IntCC::UnsignedGreaterThanOrEqual, left, right)
                }
                BinaryOp::And | BinaryOp::Or => {
                    return Err(contract_error(
                        self.symbol,
                        span,
                        "boolean operator has an unsigned integer operand",
                    ));
                }
            },
            ScalarKind::Float(_) => match op {
                BinaryOp::Add => self.builder.ins().fadd(left, right),
                BinaryOp::Subtract => self.builder.ins().fsub(left, right),
                BinaryOp::Multiply => self.builder.ins().fmul(left, right),
                BinaryOp::Divide => self.builder.ins().fdiv(left, right),
                BinaryOp::Equal => self.builder.ins().fcmp(FloatCC::Equal, left, right),
                BinaryOp::NotEqual => self.builder.ins().fcmp(FloatCC::NotEqual, left, right),
                BinaryOp::Less => self.builder.ins().fcmp(FloatCC::LessThan, left, right),
                BinaryOp::Greater => self.builder.ins().fcmp(FloatCC::GreaterThan, left, right),
                BinaryOp::LessEqual => {
                    self.builder
                        .ins()
                        .fcmp(FloatCC::LessThanOrEqual, left, right)
                }
                BinaryOp::GreaterEqual => {
                    self.builder
                        .ins()
                        .fcmp(FloatCC::GreaterThanOrEqual, left, right)
                }
                BinaryOp::Modulo | BinaryOp::And | BinaryOp::Or => {
                    return Err(contract_error(
                        self.symbol,
                        span,
                        "unsupported operator has a floating-point operand",
                    ));
                }
            },
            ScalarKind::Bool => match op {
                BinaryOp::Equal => self.builder.ins().icmp(IntCC::Equal, left, right),
                BinaryOp::NotEqual => self.builder.ins().icmp(IntCC::NotEqual, left, right),
                BinaryOp::And | BinaryOp::Or => {
                    return Err(contract_error(
                        self.symbol,
                        span,
                        "short-circuit boolean operator reached eager binary lowering",
                    ));
                }
                _ => {
                    return Err(contract_error(
                        self.symbol,
                        span,
                        "arithmetic or ordering operator has a bool operand",
                    ));
                }
            },
            ScalarKind::Nothing
            | ScalarKind::String
            | ScalarKind::Bytes
            | ScalarKind::Sum
            | ScalarKind::List
            | ScalarKind::Set
            | ScalarKind::Map
            | ScalarKind::Struct
            | ScalarKind::Enum
            | ScalarKind::Bitfield
            | ScalarKind::Stdout => {
                return Err(contract_error(
                    self.symbol,
                    span,
                    "binary operator has a nothing operand",
                ));
            }
        };
        Ok(value)
    }

    fn short_circuit_boolean(
        &mut self,
        left: Value,
        op: BinaryOp,
        right: &Expression,
        kind: ScalarKind,
        span: Span,
    ) -> Result<Value, CodegenError> {
        if kind != ScalarKind::Bool {
            return Err(contract_error(
                self.symbol,
                span,
                "short-circuit boolean operator has a non-bool operand",
            ));
        }

        let right_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        let result = self.builder.append_block_param(merge_block, ir::types::I8);
        let short_value = match op {
            BinaryOp::And => self.builder.ins().iconst(ir::types::I8, 0),
            BinaryOp::Or => self.builder.ins().iconst(ir::types::I8, 1),
            _ => {
                return Err(contract_error(
                    self.symbol,
                    span,
                    "non-short-circuit operator reached boolean CFG lowering",
                ));
            }
        };

        match op {
            BinaryOp::And => {
                self.builder
                    .ins()
                    .brif(left, right_block, &[], merge_block, &[short_value.into()])
            }
            BinaryOp::Or => {
                self.builder
                    .ins()
                    .brif(left, merge_block, &[short_value.into()], right_block, &[])
            }
            _ => unreachable!("operator checked above"),
        };

        self.builder.switch_to_block(right_block);
        let lowered_right = self.expression(right)?;
        let right_value = self.scalar(lowered_right, right.span)?;
        self.builder.ins().jump(merge_block, &[right_value.into()]);
        self.builder.switch_to_block(merge_block);
        Ok(result)
    }

    fn signed_division(&mut self, dividend: Value, divisor: Value) -> Value {
        let ty = self.builder.func.dfg.value_type(dividend);
        let zero = self.builder.ins().iconst(ty, 0);
        let dividend_negative = self
            .builder
            .ins()
            .icmp(IntCC::SignedLessThan, dividend, zero);
        let divisor_negative = self
            .builder
            .ins()
            .icmp(IntCC::SignedLessThan, divisor, zero);
        let negated_dividend = self.builder.ins().ineg(dividend);
        let negated_divisor = self.builder.ins().ineg(divisor);
        let dividend_magnitude =
            self.builder
                .ins()
                .select(dividend_negative, negated_dividend, dividend);
        let divisor_magnitude =
            self.builder
                .ins()
                .select(divisor_negative, negated_divisor, divisor);
        let magnitude = self
            .builder
            .ins()
            .udiv(dividend_magnitude, divisor_magnitude);
        let negative = self.builder.ins().bxor(dividend_negative, divisor_negative);
        let negated = self.builder.ins().ineg(magnitude);
        self.builder.ins().select(negative, negated, magnitude)
    }

    fn signed_remainder(&mut self, dividend: Value, divisor: Value) -> Value {
        let ty = self.builder.func.dfg.value_type(dividend);
        let zero = self.builder.ins().iconst(ty, 0);
        let dividend_negative = self
            .builder
            .ins()
            .icmp(IntCC::SignedLessThan, dividend, zero);
        let divisor_negative = self
            .builder
            .ins()
            .icmp(IntCC::SignedLessThan, divisor, zero);
        let negated_dividend = self.builder.ins().ineg(dividend);
        let negated_divisor = self.builder.ins().ineg(divisor);
        let dividend_magnitude =
            self.builder
                .ins()
                .select(dividend_negative, negated_dividend, dividend);
        let divisor_magnitude =
            self.builder
                .ins()
                .select(divisor_negative, negated_divisor, divisor);
        let magnitude = self
            .builder
            .ins()
            .urem(dividend_magnitude, divisor_magnitude);
        let negated = self.builder.ins().ineg(magnitude);
        self.builder
            .ins()
            .select(dividend_negative, negated, magnitude)
    }

    fn define_local(
        &mut self,
        local: jett_mir::LocalId,
        value: LoweredValue,
        span: Span,
    ) -> Result<(), CodegenError> {
        if let Some(slot) = self.local_slots[local.index() as usize] {
            let owned = if is_linear(self.types, self.local_types[local.index() as usize].ty) {
                let LoweredValue::Owned(v, source) = value else {
                    return Err(self.unsupported(span, "borrow escaping into owner"));
                };
                self.clear_slot(source);
                v
            } else {
                let value = self.scalar(value, span)?;
                self.leaf(NativeLeaf::Retain, &[value], true)?
            };
            self.drop_slot(slot)?;
            self.builder.ins().stack_store(owned, slot, 0);
            return Ok(());
        }
        let variable = variable_for(self.variables, local.index(), span, self.symbol)?;
        match (variable, value) {
            (Some(variable), LoweredValue::Scalar(value)) => {
                self.builder.try_def_var(variable, value).map_err(|error| {
                    contract_error(
                        self.symbol,
                        span,
                        format!("cannot define native local: {error}"),
                    )
                })
            }
            (None, LoweredValue::Nothing) => Ok(()),
            _ => Err(contract_error(
                self.symbol,
                span,
                "native local and value representations differ",
            )),
        }
    }

    fn required_clif_type(&self, ty: TypeId, span: Span) -> Result<ir::Type, CodegenError> {
        clif_type(self.types, ty, "native expression")?.ok_or_else(|| {
            contract_error(
                self.symbol,
                span,
                "nothing expression requires a native scalar value",
            )
        })
    }

    fn scalar(&self, value: LoweredValue, span: Span) -> Result<Value, CodegenError> {
        match value {
            LoweredValue::Scalar(value) | LoweredValue::Owned(value, _) => Ok(value),
            LoweredValue::Nothing => Err(contract_error(
                self.symbol,
                span,
                "nothing value used as a native scalar",
            )),
        }
    }

    fn unsupported(&self, span: Span, construct: &str) -> CodegenError {
        CodegenError::UnsupportedMir {
            function: self.symbol.to_string(),
            span,
            construct: construct.to_string(),
        }
    }
}

fn integer_immediate(value: i128, kind: ScalarKind) -> Option<i64> {
    match kind {
        ScalarKind::SignedInteger(bits) => {
            let value = i64::try_from(value).ok()?;
            let encoded = u64::from_ne_bytes(value.to_ne_bytes());
            let mask = match bits {
                1..=63 => (1_u64 << u32::from(bits)) - 1,
                64 => u64::MAX,
                _ => return None,
            };
            Some(i64::from_ne_bytes((encoded & mask).to_ne_bytes()))
        }
        ScalarKind::UnsignedInteger(_) => {
            let value = u64::try_from(value).ok()?;
            Some(i64::from_ne_bytes(value.to_ne_bytes()))
        }
        _ => None,
    }
}

fn variable_for(
    variables: &[Option<Variable>],
    index: u32,
    span: Span,
    symbol: &str,
) -> Result<Option<Variable>, CodegenError> {
    let index = usize::try_from(index)
        .map_err(|_| contract_error(symbol, span, "local index does not fit this host"))?;
    variables
        .get(index)
        .copied()
        .ok_or_else(|| contract_error(symbol, span, "local is absent from the native local table"))
}

fn block_for(
    blocks: &[ir::Block],
    index: u32,
    span: Span,
    symbol: &str,
) -> Result<ir::Block, CodegenError> {
    let index = usize::try_from(index)
        .map_err(|_| contract_error(symbol, span, "block index does not fit this host"))?;
    blocks
        .get(index)
        .copied()
        .ok_or_else(|| contract_error(symbol, span, "block is absent from the native block table"))
}

fn contract_error(symbol: &str, span: Span, message: impl Into<String>) -> CodegenError {
    CodegenError::InvalidMirContract {
        function: symbol.to_string(),
        span,
        message: message.into(),
    }
}

fn reordered_map<T, U, E>(
    items: &[T],
    order: &[usize],
    mut map: impl FnMut(&T) -> Result<U, E>,
    invalid_order: impl Fn() -> E,
) -> Result<Vec<U>, E> {
    if order.len() != items.len() {
        return Err(invalid_order());
    }
    let mut values = (0..items.len()).map(|_| None).collect::<Vec<Option<U>>>();
    for &index in order {
        let Some(item) = items.get(index) else {
            return Err(invalid_order());
        };
        if values[index].is_some() {
            return Err(invalid_order());
        }
        values[index] = Some(map(item)?);
    }
    values
        .into_iter()
        .map(|value| value.ok_or_else(&invalid_order))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use jett_common::{FileId, SourceOrigin};

    use super::*;
    use jett_mir::copy_values::CopyValuePlan;

    fn lower_source(source: &str) -> (Program, TypeInterner) {
        let file = FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        assert!(
            parsed.errors.is_empty(),
            "parse errors: {:?}",
            parsed.errors
        );
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
            "check errors: {:?}",
            checked.diagnostics
        );
        let hir = jett_hir::lower(
            &parsed.module,
            &resolved,
            &checked,
            &HashMap::from([(file, SourceOrigin::Project)]),
        )
        .expect("HIR lowering");
        let mir = jett_mir::lower(&hir).expect("MIR lowering");
        (mir, checked.interner)
    }

    fn test_object_module() -> ObjectModule {
        let flags = settings::Flags::new(settings::builder());
        let isa = isa::lookup(HOST)
            .expect("host ISA builder")
            .finish(flags)
            .expect("host ISA");
        let object_builder =
            ObjectBuilder::new(isa, b"jett-test".to_vec(), default_libcall_names())
                .expect("object builder");
        ObjectModule::new(object_builder)
    }

    fn declared_program(source: &str) -> (Program, TypeInterner, ObjectModule, DeclaredFunctions) {
        let (program, types) = lower_source(source);
        let verified = verify_program(&program, &types).expect("supported scalar MIR");
        let mut module = test_object_module();
        let declarations = declare_reachable_functions(&mut module, &program, &types, &verified)
            .expect("function declarations");
        (program, types, module, declarations)
    }

    fn translated_clif(source: &str) -> HashMap<String, String> {
        let (program, types, mut module, declarations) = declared_program(source);

        let mut translated = HashMap::new();
        for declaration in declarations.iter() {
            let function = program_function(&program, declaration.mir_id).expect("MIR function");
            let mut context = module.make_context();
            context.func.signature = declaration.signature.clone();
            translate_function(
                &mut module,
                &declarations,
                &program,
                function,
                &types,
                &declaration.symbol,
                &mut context,
            )
            .expect("function translation");
            translated.insert(
                function.identity.declaration.name.clone(),
                context.func.display().to_string(),
            );
        }
        translated
    }

    fn translated_function(source: &str, function_name: &str) -> ir::Function {
        let (program, types, mut module, declarations) = declared_program(source);
        let function = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == function_name)
            .expect("requested MIR function");
        let declaration = declarations
            .get(function.id)
            .expect("requested native declaration");
        let mut context = module.make_context();
        context.func.signature = declaration.signature.clone();
        translate_function(
            &mut module,
            &declarations,
            &program,
            function,
            &types,
            &declaration.symbol,
            &mut context,
        )
        .expect("function translation");
        context.func
    }

    fn direct_call_arguments(function: &ir::Function) -> Vec<Vec<Value>> {
        function
            .layout
            .blocks()
            .flat_map(|block| function.layout.block_insts(block))
            .filter(|instruction| function.dfg.insts[*instruction].opcode() == ir::Opcode::Call)
            .map(|instruction| function.dfg.inst_args(instruction).to_vec())
            .collect()
    }

    #[test]
    fn evaluates_arguments_in_lexical_order_and_returns_parameter_order() {
        let arguments = ["first parameter", "second parameter"];
        let mut evaluated = Vec::new();
        let values = reordered_map(
            &arguments,
            &[1, 0],
            |argument| {
                evaluated.push(*argument);
                Ok::<_, ()>(*argument)
            },
            || (),
        )
        .expect("valid permutation");

        assert_eq!(evaluated, ["second parameter", "first parameter"]);
        assert_eq!(values, ["first parameter", "second parameter"]);
    }

    #[test]
    fn signed_minimum_immediates_use_width_correct_bit_patterns() {
        assert_eq!(
            integer_immediate(-128, ScalarKind::SignedInteger(8)),
            Some(128)
        );
        assert_eq!(
            integer_immediate(-32_768, ScalarKind::SignedInteger(16)),
            Some(32_768)
        );
        assert_eq!(
            integer_immediate(-2_147_483_648, ScalarKind::SignedInteger(32)),
            Some(2_147_483_648)
        );
        assert_eq!(
            integer_immediate(i128::from(i64::MIN), ScalarKind::SignedInteger(64)),
            Some(i64::MIN)
        );
    }

    #[test]
    fn emitted_jett_signatures_prepend_one_hidden_runtime_context_pointer() {
        let (program, _types, module, declarations) = declared_program(
            r#"namespace app
function leaf(value: int64, enabled: bool) returns int64:
    if enabled:
        return value
    return 0
function root() returns int64:
    return leaf(7, true)
"#,
        );
        let pointer_type = module.target_config().pointer_type();

        for declaration in declarations.iter() {
            let function = program_function(&program, declaration.mir_id).expect("MIR function");
            assert_eq!(
                declaration.signature.params.len(),
                function.params.len() + 1,
                "{} must gain exactly one native-only parameter",
                function.identity.declaration.name
            );
            assert_eq!(
                declaration.signature.params[0],
                AbiParam::new(pointer_type),
                "{} must receive an ordinary runtime-context pointer first",
                function.identity.declaration.name
            );
        }

        let leaf = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "leaf")
            .expect("leaf MIR function");
        let root = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "root")
            .expect("root MIR function");
        assert_eq!(leaf.params.len(), 2, "source parameters stay unchanged");
        assert!(root.params.is_empty(), "source parameters stay unchanged");
    }

    #[test]
    fn direct_jett_calls_forward_the_exact_current_runtime_context() {
        let function = translated_function(
            r#"namespace app
function leaf(value: int64) returns int64:
    return value
function caller(value: int64, choose_original: bool) returns int64:
    if choose_original:
        return leaf(value)
    return leaf(value + 1)
"#,
            "caller",
        );
        let entry = function.layout.entry_block().expect("native entry block");
        let runtime_context = function.dfg.block_params(entry)[0];
        let calls = direct_call_arguments(&function);

        assert_eq!(calls.len(), 4, "two Jett calls and two failure checks");
        assert_eq!(calls.iter().filter(|args| args.len() == 2).count(), 2);
        for arguments in calls {
            assert!(
                matches!(arguments.len(), 1 | 2),
                "status receives context; Jett call also receives source argument"
            );
            assert_eq!(
                function.dfg.resolve_aliases(arguments[0]),
                runtime_context,
                "direct calls must forward the caller's exact context value"
            );
        }
    }

    #[test]
    fn unreachable_direct_call_blocks_are_not_emitted_with_an_undefined_context() {
        let (mut program, types) = lower_source(
            r#"namespace app
function leaf(value: int64) returns int64:
    return value
function caller(value: int64, choose_original: bool) returns int64:
    if choose_original:
        return leaf(value)
    return leaf(value + 1)
"#,
        );
        let caller = program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "caller")
            .expect("caller MIR function");
        let mut immediate_return = caller
            .blocks
            .iter()
            .find_map(|block| match &block.terminator.kind {
                TerminatorKind::Return(Some(expression))
                    if matches!(expression.kind, ExpressionKind::Call { .. }) =>
                {
                    Some(expression.clone())
                }
                _ => None,
            })
            .expect("branch-local direct call");
        immediate_return.kind = ExpressionKind::Int(0);
        let entry_index =
            usize::try_from(caller.entry.index()).expect("entry index fits this host");
        caller.blocks[entry_index].terminator.kind = TerminatorKind::Return(Some(immediate_return));

        let control_flow = ControlFlowGraph::analyze(caller).expect("valid mutated control flow");
        assert_eq!(control_flow.reverse_postorder(), [caller.entry]);
        assert!(
            caller.blocks.iter().any(|block| {
                matches!(
                    &block.terminator.kind,
                    TerminatorKind::Return(Some(Expression {
                        kind: ExpressionKind::Call { .. },
                        ..
                    }))
                )
            }),
            "the test must retain a direct call in a disconnected MIR block"
        );

        let verified = verify_program(&program, &types).expect("valid mutated scalar MIR");
        let mut module = test_object_module();
        let declarations = declare_reachable_functions(&mut module, &program, &types, &verified)
            .expect("function declarations");
        let caller = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "caller")
            .expect("caller MIR function");
        let declaration = declarations.get(caller.id).expect("caller declaration");
        let mut context = module.make_context();
        context.func.signature = declaration.signature.clone();
        translate_function(
            &mut module,
            &declarations,
            &program,
            caller,
            &types,
            &declaration.symbol,
            &mut context,
        )
        .expect("reachable-only function translation");

        assert_eq!(context.func.layout.blocks().count(), 2); // plus terminal-failure epilogue
        assert!(direct_call_arguments(&context.func).is_empty());
    }

    #[test]
    fn exported_entry_wrapper_forwards_its_incoming_runtime_context() {
        let (program, _types, mut module, declarations) = declared_program(
            r#"namespace app
function selected_entry() returns nothing:
    return nothing
"#,
        );
        let entry = program.functions[0].id;
        let entry_function = declarations.get(entry).expect("native entry declaration");
        let mut context = module.make_context();
        context.func.signature = program_entry_signature(&module);
        translate_program_entry_wrapper(&mut module, entry_function, entry, &mut context)
            .expect("entry wrapper translation");

        let block = context
            .func
            .layout
            .entry_block()
            .expect("wrapper entry block");
        let incoming_context = context.func.dfg.block_params(block)[0];
        let calls = direct_call_arguments(&context.func);
        assert_eq!(calls, [vec![incoming_context], vec![incoming_context]]); // entry and failure status
        assert_eq!(
            context.func.signature.params,
            [AbiParam::new(module.target_config().pointer_type())]
        );
        assert!(program.functions[0].params.is_empty());
    }

    #[test]
    fn boolean_and_or_branch_before_emitting_rhs_calls() {
        let functions = translated_clif(
            r#"namespace app
function rhs() returns bool:
    return true
function and_value(left: bool) returns bool:
    return left && rhs()
function or_value(left: bool) returns bool:
    return left || rhs()
"#,
        );

        for name in ["and_value", "or_value"] {
            let clif = functions.get(name).expect("translated test function");
            let branch = clif.find("brif ").expect("short-circuit branch");
            let call = clif.find(" = call ").expect("RHS call");
            assert!(
                branch < call,
                "{name} eagerly emits its RHS before branching:\n{clif}"
            );
            assert!(
                !clif.contains("band ") && !clif.contains("bor "),
                "{name} used eager boolean arithmetic:\n{clif}"
            );
        }
    }

    #[test]
    fn native_ownership_rejects_read_before_initialization() {
        let (mut program, types) = lower_source(
            r#"
function selected_entry() returns string:
    string value = "live"
    return value
"#,
        );
        program.functions[0].blocks[0].statements.clear();
        let error =
            emit_host_object(&program, &types).expect_err("missing initializer is invalid MIR");
        assert!(
            error.to_string().contains("definite initialization"),
            "{error}"
        );
    }

    #[test]
    fn native_ownership_plan_keeps_loop_carried_strings_live() {
        let (program, types) = lower_source(
            r#"
function selected_entry(keep: bool) returns string:
    mutable string value = "live"
    while keep:
        value = "next"
    return value
"#,
        );
        let function = &program.functions[0];
        let plan = CopyValuePlan::analyze(function, &types).unwrap();
        let local = function
            .locals
            .iter()
            .find(|l| l.name == "value")
            .unwrap()
            .id
            .index() as usize;
        assert!(plan.owned_locals.contains(&local));
        for block in &function.blocks {
            if matches!(block.terminator.kind, TerminatorKind::Branch { .. }) {
                assert!(plan.live_in[block.id.index() as usize].contains(&local));
                assert!(plan.live_out[block.id.index() as usize].contains(&local));
            }
        }
    }

    #[test]
    fn copy_value_plan_does_not_claim_move_only_ownership() {
        let (program, types) = lower_source(
            r#"
function discard(view value: bytes) returns nothing:
    return nothing
"#,
        );
        let error = CopyValuePlan::analyze(&program.functions[0], &types)
            .expect_err("bytes needs move/borrow/drop analysis");
        assert!(error.contains("move/borrow/drop"), "{error}");
    }
    #[test]
    fn native_linear_plan_rejects_move_across_loop_backedge() {
        let (program, types) = lower_source(
            r#"
function consume(value: bytes) returns nothing:
    return nothing
function loop_move(value: bytes, repeat: bool) returns nothing:
    while repeat:
        consume(value)
"#,
        );
        let error = emit_host_object(&program, &types)
            .expect_err("loop reads moved place on next iteration");
        assert!(
            error.to_string().contains("moved or uninitialized"),
            "{error}"
        );
    }
    #[test]
    fn native_linear_plan_rejects_borrow_escape_and_argument_alias() {
        let (mut program, types) = lower_source(
            r#"
function identity(value: bytes) returns bytes:
    return value
"#,
        );
        program.functions[0].params[0].mode = jett_mir::ParamMode::View;
        let error =
            emit_host_object(&program, &types).expect_err("view cannot be returned as owner");
        assert!(
            error.to_string().contains("cannot move borrowed"),
            "{error}"
        );
        let (program, types) = lower_source(
            r#"
function mixed(view first: bytes, second: bytes) returns nothing:
    return nothing
function alias(value: bytes) returns nothing:
    mixed(view value, value)
"#,
        );
        let error = emit_host_object(&program, &types)
            .expect_err("loan must survive all argument evaluation");
        assert!(error.to_string().contains("while borrowed"), "{error}");
    }
    #[test]
    fn iteration_loan_survives_nested_loop_and_rejects_owner_escape() {
        let (program, types) = lower_source(
            r#"
function escaped(items: list[int64]) returns list[int64]:
    for item in view items:
        for other in view items:
            break
        return items
    return items
"#,
        );
        let error =
            emit_host_object(&program, &types).expect_err("outer iteration view remains active");
        assert!(error.to_string().contains("while borrowed"), "{error}");
    }
    #[test]
    fn consuming_sequence_cannot_take_from_a_borrowed_parameter() {
        let (mut program, types) = lower_source(
            r#"
function visit(items: list[int64]) returns nothing:
    for item in view items:
        break
"#,
        );
        jett_mir::prepare_native_sequences(&mut program, &types);
        program.functions[0].params[0].mode = jett_mir::ParamMode::View;
        for block in &mut program.functions[0].blocks {
            for statement in &mut block.statements {
                if let StatementKind::SequenceGet { consume, .. } = &mut statement.kind {
                    *consume = true;
                }
            }
        }
        let error = emit_host_object(&program, &types).expect_err("cannot take a borrowed element");
        assert!(
            error
                .to_string()
                .contains("cannot take element from borrowed"),
            "{error}"
        );
    }
    #[test]
    fn sequence_preheader_is_selected_by_cfg_not_block_position() {
        let (mut program, types) = lower_source(
            r#"
function visit(items: list[int64]) returns int64:
    mutable int64 total = 0
    for item in items:
        total = total + item
    return total
"#,
        );
        let function = &mut program.functions[0];
        let old_entry = function.entry;
        let last = function.blocks.last().unwrap().id;
        let remap = |id| {
            if id == old_entry {
                last
            } else if id == last {
                old_entry
            } else {
                id
            }
        };
        function
            .blocks
            .swap(old_entry.index() as usize, last.index() as usize);
        function.entry = remap(old_entry);
        for block in &mut function.blocks {
            block.id = remap(block.id);
            match &mut block.terminator.kind {
                TerminatorKind::Goto(target) => *target = remap(*target),
                TerminatorKind::ForEach { body, exit, .. } => {
                    *body = remap(*body);
                    *exit = remap(*exit);
                }
                TerminatorKind::Return(_) | TerminatorKind::Unreachable => {}
                other => panic!("unexpected fixture terminator {other:?}"),
            }
        }
        emit_host_object(&program, &types).expect("late-numbered unique loop preheader");
    }
}
