//! Private logical owners and nonowning tickets. Public Value remains unchanged.
use super::receipts::{ProducerReceipt, checked_path, copyable, producer_receipt, unparen};
use super::*;
use jett_parser::ast::{Block, Expr, Stmt};
use jett_typecheck::CheckedBindingMode;

pub(in crate::interpreter) type ActivationId = u64;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct AliasTicket {
    pub(super) owner_activation: ActivationId,
    pub(super) owner_index: usize,
    pub(super) whole: TypeId,
    pub(super) path: Vec<Projection>,
    pub(super) endpoint: TypeId,
}

pub(super) struct InstalledAlias {
    /// Dynamic alias identity is (function activation, original binding DefId).
    pub(super) activation: ActivationId,
    pub(super) fact: CheckedBindingFact,
    pub(super) name: String,
    pub(super) ticket: AliasTicket,
}

pub(super) enum OwnerState {
    Vacant,
    Live(Value),
    Retired,
}
pub(super) struct OwnerSlot {
    pub(super) receipt: ProducerReceipt,
    pub(super) state: OwnerState,
}
pub(super) struct Activation {
    pub(super) id: ActivationId,
    pub(super) function: FunctionReceipt,
    pub(super) root_active: bool,
    pub(super) root_scope: Option<usize>,
    pub(super) owners: Vec<OwnerSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AuditEvent {
    Reserved {
        activation: ActivationId,
        owner: usize,
    },
    Acquired {
        activation: ActivationId,
        owner: usize,
    },
    AliasPublished {
        scope: usize,
        definition: DefId,
        alias_activation: ActivationId,
        activation: ActivationId,
        owner: usize,
    },
    AliasEnded {
        scope: usize,
        definition: DefId,
        alias_activation: ActivationId,
        activation: ActivationId,
        owner: usize,
    },
    OwnerRetired {
        activation: ActivationId,
        owner: usize,
    },
    ExplicitClone {
        activation: ActivationId,
        owner: usize,
    },
    Output {
        activation: ActivationId,
    },
}

pub(in crate::interpreter) struct Context {
    pub(super) program: Arc<jett_typecheck::CheckedResourceProgram>,
    pub(super) entry_allowed: bool,
    pub(super) activations: Vec<Activation>,
    pub(super) aliases: Vec<HashMap<DefId, InstalledAlias>>,
    pub(super) next_identity: Option<u64>,
    pub(super) audit: Vec<AuditEvent>,
}

impl Context {
    pub(in crate::interpreter) fn needed(
        program: &Arc<jett_typecheck::CheckedResourceProgram>,
    ) -> bool {
        Self::body_needed(program, None)
    }

    pub(super) fn body_needed(
        program: &Arc<jett_typecheck::CheckedResourceProgram>,
        body: Option<Span>,
    ) -> bool {
        let checked = program.checked();
        temporary_facts(program, &checked.binding_facts, body)
            || scoped_temporary(program, &checked.comptime_type_bindings, body)
            || checked.actor_handlers.values().any(|handler| {
                temporary_facts(program, &handler.body().binding_facts, body)
                    || scoped_temporary(program, &handler.body().comptime_type_bindings, body)
            })
            || checked
                .generic_function_instantiations
                .iter()
                .any(|instance| {
                    temporary_facts(program, &instance.binding_facts, body)
                        || scoped_temporary(program, &instance.comptime_type_bindings, body)
                })
    }

    pub(in crate::interpreter) fn new(
        program: Arc<jett_typecheck::CheckedResourceProgram>,
        depth: usize,
    ) -> Self {
        Self {
            program,
            entry_allowed: false,
            activations: Vec::new(),
            aliases: (0..depth).map(|_| HashMap::new()).collect(),
            next_identity: Some(1),
            audit: Vec::new(),
        }
    }

    pub(in crate::interpreter) fn push_scope(&mut self) {
        self.aliases.push(HashMap::new());
    }

    pub(in crate::interpreter) fn pop_scope(&mut self) {
        // Exactly paired with Interpreter::push_scope; owner cleanup runs
        // separately before this infallible bookkeeping operation.
        if let Some(scope) = self.aliases.len().checked_sub(1)
            && let Some(aliases) = self.aliases.pop()
        {
            self.end_aliases(scope, aliases);
        }
    }

    fn end_aliases(&mut self, scope: usize, aliases: HashMap<DefId, InstalledAlias>) {
        // Alias end events have no owning cleanup order; owner retirement does.
        let mut aliases: Vec<_> = aliases.into_iter().collect();
        aliases.sort_by_key(|(definition, _)| definition.index());
        for (definition, alias) in aliases {
            self.audit.push(AuditEvent::AliasEnded {
                scope,
                definition,
                alias_activation: alias.activation,
                activation: alias.ticket.owner_activation,
                owner: alias.ticket.owner_index,
            });
        }
    }

    pub(super) fn enter(&mut self, function: FunctionReceipt) -> Result<ActivationId, String> {
        if !Arc::ptr_eq(&self.program, &function.program)
            || (self.activations.is_empty() && !self.entry_allowed)
        {
            return Err("temporary local view has no checked invocation entry".into());
        }
        function.original()?;
        let id = self
            .next_identity
            .ok_or("temporary local view activation identities exhausted")?;
        self.next_identity = id.checked_add(1);
        self.entry_allowed = false;
        self.activations.push(Activation {
            id,
            function,
            root_active: false,
            root_scope: None,
            owners: Vec::new(),
        });
        Ok(id)
    }

    pub(super) fn leave(&mut self, id: ActivationId) -> Result<(), String> {
        let current = self
            .current()
            .ok_or("temporary local view lost its activation")?;
        if current.id != id
            || current.root_active
            || current
                .owners
                .iter()
                .any(|owner| matches!(owner.state, OwnerState::Live(_)))
        {
            return Err("temporary local view activation still has live obligations".into());
        }
        self.activations.pop();
        Ok(())
    }

    pub(super) fn begin(
        &mut self,
        block: &Block,
        scope: usize,
    ) -> Result<Option<ActivationId>, String> {
        let Some(current) = self.current() else {
            return Ok(None);
        };
        if !std::ptr::eq(&current.function.original()?.body, block) {
            return Ok(None);
        }
        if current.root_active || current.root_scope.is_some() || scope >= self.aliases.len() {
            return Err("temporary local view root activation was reused".into());
        }
        let current = self
            .activations
            .last_mut()
            .ok_or("temporary local view lost its activation")?;
        current.root_active = true;
        current.root_scope = Some(scope);
        Ok(Some(current.id))
    }

    pub(super) fn complete(&mut self, id: ActivationId) -> Result<(), String> {
        let position = self
            .activations
            .len()
            .checked_sub(1)
            .ok_or("temporary local view lost its activation")?;
        let current = &self.activations[position];
        if current.id != id || !current.root_active {
            return Err("temporary local view root completion is stale".into());
        }
        let scope = current
            .root_scope
            .ok_or("temporary local view lost its root scope")?;
        // Scopes below the exact source root belong to parameters or callers.
        for index in scope..self.aliases.len() {
            let aliases = std::mem::take(&mut self.aliases[index]);
            self.end_aliases(index, aliases);
        }
        for owner in (0..self.activations[position].owners.len()).rev() {
            self.retire(position, owner);
        }
        self.activations[position].root_active = false;
        Ok(())
    }

    fn retire(&mut self, activation: usize, owner: usize) {
        let state = std::mem::replace(
            &mut self.activations[activation].owners[owner].state,
            OwnerState::Retired,
        );
        if matches!(state, OwnerState::Live(_)) {
            self.audit.push(AuditEvent::OwnerRetired {
                activation: self.activations[activation].id,
                owner,
            });
        }
        drop(state);
    }

    pub(super) fn current(&self) -> Option<&Activation> {
        self.activations.last()
    }

    pub(super) fn alias_for(
        &self,
        function: &FunctionReceipt,
        expression: &Expr,
        floor: usize,
    ) -> Result<Option<AliasTicket>, String> {
        let current = self
            .current()
            .ok_or("temporary local view has no active function")?;
        if !current.root_active
            || !same_function(&current.function, function)?
            || floor > self.aliases.len()
        {
            return Err("temporary local view read has a foreign activation".into());
        }
        let mut selected = unparen(expression);
        if let Expr::View(inner, _) = selected {
            selected = unparen(inner);
        }
        let mut root = selected;
        while let Expr::FieldAccess(base, _, _) = root {
            root = unparen(base);
        }
        let Expr::Ident(name) = root else {
            return Ok(None);
        };
        let Some(definition) = self.program.resolved().resolutions.get(&name.span) else {
            return Ok(None);
        };
        let Some(alias) = self.aliases[floor..]
            .iter()
            .rev()
            .find_map(|scope| scope.get(definition))
        else {
            return Ok(None);
        };
        function.expression(expression)?;
        let facts = function.facts()?;
        let fact = facts
            .binding_facts
            .get(&alias.fact.declaration_span)
            .ok_or("temporary local view lost its checked alias fact")?;
        if alias.activation != current.id
            || fact != &alias.fact
            || fact.definition != *definition
            || fact.ty != alias.ticket.endpoint
            || alias.name != name.name
            || fact.mutable
            || !matches!(fact.mode, CheckedBindingMode::View { .. })
        {
            return Err("temporary local view read changed its exact alias binding".into());
        }
        let mut extension = Vec::new();
        let endpoint = checked_path(function, selected, &mut extension)?;
        if extension
            .first()
            .is_some_and(|projection| projection.owner != alias.ticket.endpoint)
        {
            return Err("temporary local view read changed its projection root".into());
        }
        let mut ticket = alias.ticket.clone();
        ticket.path.extend(extension);
        ticket.endpoint = endpoint;
        self.observe(&ticket)?;
        Ok(Some(ticket))
    }

    pub(in crate::interpreter) fn placeholder(&self, scope: usize, name: &str) -> bool {
        self.aliases
            .get(scope)
            .is_some_and(|scope| scope.values().any(|alias| alias.name == name))
    }

    pub(super) fn reserve(
        &mut self,
        receipt: ProducerReceipt,
    ) -> Result<(ActivationId, usize), String> {
        let current = self
            .current()
            .ok_or("temporary local view has no active owner scope")?;
        if !current.root_active || !same_function(&current.function, &receipt.function)? {
            return Err("temporary local view producer has a foreign activation".into());
        }
        revalidate_receipt(&receipt)?;
        if current
            .owners
            .iter()
            .any(|slot| slot.receipt.declaration == receipt.declaration)
        {
            return Err("temporary local view declaration initialized twice".into());
        }
        let activation = current.id;
        let owner = current.owners.len();
        self.activations
            .last_mut()
            .ok_or("temporary local view lost its activation")?
            .owners
            .push(OwnerSlot {
                receipt,
                state: OwnerState::Vacant,
            });
        self.audit.push(AuditEvent::Reserved { activation, owner });
        Ok((activation, owner))
    }

    pub(super) fn acquire(
        &mut self,
        slot: (ActivationId, usize),
        value: Value,
    ) -> Result<AliasTicket, String> {
        let position = self
            .activations
            .len()
            .checked_sub(1)
            .ok_or("temporary local view has no activation")?;
        let current = &self.activations[position];
        let owner = current
            .owners
            .get(slot.1)
            .ok_or("temporary local view owner slot disappeared")?;
        if current.id != slot.0
            || !current.root_active
            || !matches!(owner.state, OwnerState::Vacant)
        {
            return Err("temporary local view owner acquisition is stale".into());
        }
        revalidate_receipt(&owner.receipt)?;
        let ticket = AliasTicket {
            owner_activation: slot.0,
            owner_index: slot.1,
            whole: owner.receipt.whole,
            path: owner.receipt.path.clone(),
            endpoint: owner.receipt.fact.ty,
        };
        self.activations[position].owners[slot.1].state = OwnerState::Live(value);
        self.audit.push(AuditEvent::Acquired {
            activation: slot.0,
            owner: slot.1,
        });
        if let Err(error) = self.observe(&ticket) {
            self.retire(position, slot.1);
            return Err(error);
        }
        Ok(ticket)
    }

    pub(super) fn publish(
        &mut self,
        fact: CheckedBindingFact,
        name: &str,
        ticket: AliasTicket,
        scope: usize,
    ) -> Result<(), String> {
        let current = self
            .current()
            .ok_or("temporary local view alias has no activation")?;
        let alias_activation = current.id;
        let source = current.function.original()?;
        let facts = current.function.facts()?;
        let info = self
            .program
            .resolved()
            .scope_table
            .definitions
            .iter()
            .find(|info| info.id == fact.definition)
            .ok_or("temporary local view alias lost its resolved declaration")?;
        let parameter = source.params.iter().any(|parameter| {
            parameter.name.span == fact.declaration_span
                && parameter.name.name == name
                && parameter.view
                && !parameter.mutable
        });
        let declaration = current.root_active && source.body.stmts.iter().any(|statement| {
            matches!(statement, Stmt::VarDecl(declaration) if declaration.name.span == fact.declaration_span
                && declaration.name.name == name && !declaration.mutable)
        });
        if facts.binding_facts.get(&fact.declaration_span) != Some(&fact)
            || info.span != fact.declaration_span
            || info.name != name
            || fact.mutable
            || fact.ty != ticket.endpoint
            || !matches!(fact.mode, CheckedBindingMode::View { .. })
            || !matches!(
                (info.kind, parameter, declaration),
                (jett_resolve::DefKind::Param, true, _)
                    | (jett_resolve::DefKind::Variable, _, true)
            )
            || scope.checked_add(1) != Some(self.aliases.len())
        {
            return Err("temporary local view alias has no exact immutable declaration".into());
        }
        self.observe(&ticket)?;
        let aliases = self
            .aliases
            .get_mut(scope)
            .ok_or("temporary local view alias scope disappeared")?;
        if aliases.contains_key(&fact.definition)
            || aliases.values().any(|alias| alias.name == name)
        {
            return Err("temporary local view alias declaration was overwritten".into());
        }
        self.audit.push(AuditEvent::AliasPublished {
            scope,
            definition: fact.definition,
            alias_activation,
            activation: ticket.owner_activation,
            owner: ticket.owner_index,
        });
        aliases.insert(
            fact.definition,
            InstalledAlias {
                activation: alias_activation,
                fact,
                name: name.to_owned(),
                ticket,
            },
        );
        Ok(())
    }

    pub(super) fn observe(&self, ticket: &AliasTicket) -> Result<&Value, String> {
        let activation = self
            .activations
            .iter()
            .find(|activation| activation.id == ticket.owner_activation)
            .ok_or("temporary local view backing activation is stale")?;
        let slot = activation
            .owners
            .get(ticket.owner_index)
            .ok_or("temporary local view backing owner disappeared")?;
        revalidate_receipt(&slot.receipt)?;
        if !activation.root_active
            || !Arc::ptr_eq(&self.program, &activation.function.program)
            || !same_function(&activation.function, &slot.receipt.function)?
            || ticket.whole != slot.receipt.whole
            || !ticket.path.starts_with(&slot.receipt.path)
        {
            return Err("temporary local view ticket changed its backing identity".into());
        }
        let OwnerState::Live(whole_value) = &slot.state else {
            return Err("temporary local view backing owner is not acquired".into());
        };
        let mut value = whole_value;
        let types = &self.program.checked().interner;
        plain_type(types, ticket.whole, &mut Vec::new())?;
        validate_value(types, ticket.whole, value)?;
        let mut owner = ticket.whole;
        for projection in &ticket.path {
            let Type::Struct(id) = types.resolve(owner) else {
                return Err("temporary local view projection owner is not a struct".into());
            };
            let structure = types.resolve_struct(*id);
            let Value::Struct {
                type_name,
                concrete_type,
                fields,
            } = value
            else {
                return Err(
                    "temporary local view projection cannot inspect wrapped or pending storage"
                        .into(),
                );
            };
            if projection.owner != owner
                || !types.nominal_type_arguments(owner).is_empty()
                || projection.name != structure.name
                || type_name != &structure.name
                || concrete_type.is_some()
                || projection.fields
                    != structure
                        .fields
                        .iter()
                        .map(|(name, _)| name.clone())
                        .collect::<Vec<_>>()
                || fields.len() != structure.fields.len()
                || structure.fields.get(projection.index).map(|(_, ty)| *ty)
                    != Some(projection.endpoint)
                || slot
                    .receipt
                    .function
                    .facts()?
                    .type_map
                    .get(&projection.span)
                    != Some(&projection.endpoint)
            {
                return Err(
                    "temporary local view projection changed its exact checked shape".into(),
                );
            }
            value = &fields
                .get(projection.index)
                .ok_or("temporary local view field storage disappeared")?
                .1;
            owner = projection.endpoint;
        }
        if owner != ticket.endpoint {
            return Err("temporary local view endpoint changed its checked type".into());
        }
        validate_value(types, owner, value)?;
        Ok(value)
    }

    pub(super) fn explicit_clone(&mut self, ticket: &AliasTicket) -> Result<Value, String> {
        let value = self.observe(ticket)?.clone();
        self.audit.push(AuditEvent::ExplicitClone {
            activation: ticket.owner_activation,
            owner: ticket.owner_index,
        });
        Ok(value)
    }
}

pub(super) fn temporary_facts(
    program: &jett_typecheck::CheckedResourceProgram,
    facts: &HashMap<Span, CheckedBindingFact>,
    body: Option<Span>,
) -> bool {
    facts.values().any(|fact| {
        if !matches!(
            fact.mode,
            CheckedBindingMode::View {
                source: CheckedViewSource::Other
            }
        ) || body.is_some_and(|body| {
            fact.declaration_span.file != body.file
                || fact.declaration_span.start < body.start
                || fact.declaration_span.end > body.end
        }) {
            return false;
        }
        let info = program.resolved().scope_table.def(fact.definition);
        if info.kind != jett_resolve::DefKind::Variable || info.span != fact.declaration_span {
            return false;
        }
        crate::value::closure_capture::original_variable_declaration(program, fact.declaration_span)
            .is_some_and(|declaration| {
                declaration.name.name == info.name
                    && !declaration.mutable
                    && !fact.mutable
                    && matches!(unparen(&declaration.value), Expr::View(_, _))
            })
    })
}

pub(super) fn scoped_temporary(
    program: &jett_typecheck::CheckedResourceProgram,
    scopes: &HashMap<Span, Vec<jett_typecheck::CheckedComptimeTypeBinding>>,
    body: Option<Span>,
) -> bool {
    scopes.values().flatten().any(|scope| {
        temporary_facts(program, &scope.body.binding_facts, body)
            || scoped_temporary(program, &scope.body.comptime_type_bindings, body)
    })
}

pub(super) fn same_function(
    left: &FunctionReceipt,
    right: &FunctionReceipt,
) -> Result<bool, String> {
    Ok(Arc::ptr_eq(&left.program, &right.program)
        && left.same(right)
        && std::ptr::eq(left.original()?, right.original()?))
}

fn revalidate_receipt(receipt: &ProducerReceipt) -> Result<(), String> {
    let declaration = match receipt
        .function
        .original()?
        .body
        .stmts
        .get(receipt.declaration)
    {
        Some(Stmt::VarDecl(declaration)) => declaration,
        _ => return Err("temporary local view receipt lost its original declaration".into()),
    };
    let current = producer_receipt(&receipt.function, declaration)?
        .ok_or("temporary local view receipt lost its checked producer")?;
    if current.fact != receipt.fact
        || current.whole != receipt.whole
        || current.path != receipt.path
    {
        return Err("temporary local view receipt changed its checked projection".into());
    }
    Ok(())
}

fn plain_type(
    types: &jett_types::TypeInterner,
    ty: TypeId,
    ancestors: &mut Vec<TypeId>,
) -> Result<(), String> {
    if ancestors.contains(&ty) {
        return Err("temporary local view recursive storage remains unsupported".into());
    }
    ancestors.push(ty);
    let accepted = match types.resolve(ty) {
        Type::Struct(id) if types.nominal_type_arguments(ty).is_empty() => types.resolve_struct(*id).fields.iter()
            .try_for_each(|(_, field)| plain_type(types, *field, ancestors)),
        Type::List(inner) | Type::Set(inner) | Type::Optional(inner) => plain_type(types, *inner, ancestors),
        Type::Map(first, second) | Type::Result(first, second) => plain_type(types, *first, ancestors)
            .and_then(|()| plain_type(types, *second, ancestors)),
        Type::Bytes => Ok(()),
        _ if copyable(types, ty) => Ok(()),
        _ => Err("temporary local view wrapped, generic or runtime-authority storage remains unsupported".into()),
    };
    ancestors.pop();
    accepted
}

fn validate_value(
    types: &jett_types::TypeInterner,
    ty: TypeId,
    value: &Value,
) -> Result<(), String> {
    let valid = match (types.resolve(ty), value) {
        (Type::Int8, Value::Int64(value)) => (-128..=127).contains(value),
        (Type::Int16, Value::Int64(value)) => (-32768..=32767).contains(value),
        (Type::Int32, Value::Int64(value)) => {
            (i64::from(i32::MIN)..=i64::from(i32::MAX)).contains(value)
        }
        (Type::Int64, Value::Int64(_))
        | (Type::Uint64, Value::Uint64(_))
        | (Type::Float32 | Type::Float64, Value::Float64(_))
        | (Type::String, Value::String(_))
        | (Type::Bool, Value::Bool(_))
        | (Type::Bytes, Value::Bytes(_))
        | (Type::Nothing, Value::Nothing) => true,
        (Type::Uint8, Value::Int64(value)) => (0..=255).contains(value),
        (Type::Uint16, Value::Int64(value)) => (0..=65535).contains(value),
        (Type::Uint32, Value::Int64(value)) => (0..=i64::from(u32::MAX)).contains(value),
        (Type::Uint8, Value::Uint64(value)) => *value <= u64::from(u8::MAX),
        (Type::Uint16, Value::Uint64(value)) => *value <= u64::from(u16::MAX),
        (Type::Uint32, Value::Uint64(value)) => *value <= u64::from(u32::MAX),
        (
            Type::Struct(id),
            Value::Struct {
                type_name,
                concrete_type,
                fields,
            },
        ) => {
            let structure = types.resolve_struct(*id);
            types.nominal_type_arguments(ty).is_empty()
                && concrete_type.is_none()
                && type_name == &structure.name
                && fields.len() == structure.fields.len()
                && fields
                    .iter()
                    .zip(&structure.fields)
                    .all(|((name, value), (expected, ty))| {
                        name == expected && validate_value(types, *ty, value).is_ok()
                    })
        }
        (Type::List(inner), Value::List(values)) | (Type::Set(inner), Value::Set(values)) => values
            .iter()
            .all(|value| validate_value(types, *inner, value).is_ok()),
        (Type::Map(key, item), Value::Map(values)) => values.iter().all(|(first, second)| {
            validate_value(types, *key, first).is_ok()
                && validate_value(types, *item, second).is_ok()
        }),
        (Type::Optional(inner), Value::OptionalSome(value)) => {
            validate_value(types, *inner, value).is_ok()
        }
        (Type::Optional(_), Value::OptionalNone) => true,
        (Type::Result(ok, _), Value::ResultOk(value)) => validate_value(types, *ok, value).is_ok(),
        (Type::Result(_, fail), Value::ResultFail(value)) => {
            validate_value(types, *fail, value).is_ok()
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("temporary local view value disagrees with its exact checked ordinary type".into())
    }
}
