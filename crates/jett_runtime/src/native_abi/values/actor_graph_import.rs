//! Private two-phase import of a compiler-sealed frozen actor graph.
//! JA1 proves carrier shapes, not nominal types, refinements, or callback selection.
//! Those remain exact checked-plan obligations of the compiler graph factory.
use super::*;
use std::collections::HashSet;

pub(super) const INVALID_IMPORT: Failure = (
    JettRuntimeStatusV1::INVALID_ARGUMENT,
    b"invalid native actor graph import ticket, shape, or field",
);

pub(super) struct Import {
    bases: Vec<u64>,
    records: HashMap<u64, NativeStruct>,
    pending: HashMap<u64, (u64, u64)>,
    // Transferred trees are absent from all ordinary leaf registries until commit.
    ordinary: NativeValues,
    roots: Vec<u64>,
    output: Option<NativeField>,
}

enum Supplement {
    Hidden(usize),
    Function {
        code: u64,
        environment: Option<usize>,
    },
    Interface {
        concrete: u64,
        child: usize,
    },
    Builder {
        owner: String,
        offset: usize,
        fields: Vec<usize>,
    },
    Resource,
    ListEntries(Vec<usize>),
    MapEntries(Vec<(usize, usize)>),
    CheckedHook {
        descriptor_operation: u32,
        hook: u32,
        signature: u32,
    },
}

struct ImportLayout {
    debug: NativeDebugLayout,
    supplements: HashMap<usize, Supplement>,
}

impl ImportLayout {
    fn parse(bytes: &[u8]) -> LeafResult<Self> {
        let mut cursor = BitfieldLayoutCursor { bytes, position: 0 };
        if cursor.take(3).map_err(|_| INVALID_IMPORT)? != b"JA\x01" {
            return Err(INVALID_IMPORT);
        }
        let length = cursor.u32().map_err(|_| INVALID_IMPORT)? as usize;
        let debug = NativeDebugLayout::parse(cursor.take(length).map_err(|_| INVALID_IMPORT)?)
            .map_err(|error| {
                if error == EXHAUSTED {
                    error
                } else {
                    INVALID_IMPORT
                }
            })?;
        let count = cursor.u32().map_err(|_| INVALID_IMPORT)? as usize;
        if count > debug.nodes.len() {
            return Err(INVALID_IMPORT);
        }
        let mut supplements = HashMap::new();
        supplements.try_reserve(count).map_err(|_| EXHAUSTED)?;
        for _ in 0..count {
            let index = cursor.u32().map_err(|_| INVALID_IMPORT)? as usize;
            let node = debug.nodes.get(index).ok_or(INVALID_IMPORT)?;
            let supplement = match cursor.byte().map_err(|_| INVALID_IMPORT)? {
                0 if matches!(node, NativeDebugNode::Redacted) => {
                    Supplement::Hidden(cursor.u32().map_err(|_| INVALID_IMPORT)? as usize)
                }
                1 if matches!(node, NativeDebugNode::Function) => {
                    let code = u64::from_le_bytes(
                        cursor
                            .take(8)
                            .map_err(|_| INVALID_IMPORT)?
                            .try_into()
                            .map_err(|_| INVALID_IMPORT)?,
                    );
                    if code == 0 {
                        return Err(INVALID_IMPORT);
                    }
                    let environment = cursor.u32().map_err(|_| INVALID_IMPORT)?;
                    Supplement::Function {
                        code,
                        environment: (environment != u32::MAX).then_some(environment as usize),
                    }
                }
                2 if matches!(node, NativeDebugNode::Interface) => {
                    let concrete = u64::from_le_bytes(
                        cursor
                            .take(8)
                            .map_err(|_| INVALID_IMPORT)?
                            .try_into()
                            .map_err(|_| INVALID_IMPORT)?,
                    );
                    let child = cursor.u32().map_err(|_| INVALID_IMPORT)? as usize;
                    Supplement::Interface { concrete, child }
                }
                3 if matches!(node, NativeDebugNode::TypeConstruction) => {
                    let owner = cursor.name().map_err(|_| INVALID_IMPORT)?;
                    let offset = cursor.u32().map_err(|_| INVALID_IMPORT)? as usize;
                    let count = cursor.u32().map_err(|_| INVALID_IMPORT)? as usize;
                    if count > bytes.len() / 4 {
                        return Err(INVALID_IMPORT);
                    }
                    let mut fields = Vec::new();
                    fields.try_reserve_exact(count).map_err(|_| EXHAUSTED)?;
                    for _ in 0..count {
                        fields.push(cursor.u32().map_err(|_| INVALID_IMPORT)? as usize);
                    }
                    Supplement::Builder {
                        owner,
                        offset,
                        fields,
                    }
                }
                4 if matches!(node, NativeDebugNode::Redacted) => Supplement::Resource,
                5 if matches!(node, NativeDebugNode::List(_)) => {
                    let count = cursor.u32().map_err(|_| INVALID_IMPORT)? as usize;
                    if count > bytes.len() / 4 {
                        return Err(INVALID_IMPORT);
                    }
                    let mut children = Vec::new();
                    children.try_reserve_exact(count).map_err(|_| EXHAUSTED)?;
                    for _ in 0..count {
                        children.push(cursor.u32().map_err(|_| INVALID_IMPORT)? as usize);
                    }
                    Supplement::ListEntries(children)
                }
                6 if matches!(node, NativeDebugNode::Map(..)) => {
                    let count = cursor.u32().map_err(|_| INVALID_IMPORT)? as usize;
                    if count > bytes.len() / 8 {
                        return Err(INVALID_IMPORT);
                    }
                    let mut children = Vec::new();
                    children.try_reserve_exact(count).map_err(|_| EXHAUSTED)?;
                    for _ in 0..count {
                        children.push((
                            cursor.u32().map_err(|_| INVALID_IMPORT)? as usize,
                            cursor.u32().map_err(|_| INVALID_IMPORT)? as usize,
                        ));
                    }
                    Supplement::MapEntries(children)
                }
                7 if matches!(node, NativeDebugNode::Function) => Supplement::CheckedHook {
                    descriptor_operation: cursor.u32().map_err(|_| INVALID_IMPORT)?,
                    hook: cursor.u32().map_err(|_| INVALID_IMPORT)?,
                    signature: cursor.u32().map_err(|_| INVALID_IMPORT)?,
                },
                _ => return Err(INVALID_IMPORT),
            };
            if supplements.insert(index, supplement).is_some() {
                return Err(INVALID_IMPORT);
            }
        }
        if cursor.position != bytes.len() {
            return Err(INVALID_IMPORT);
        }
        let result = Self { debug, supplements };
        result.validate_indices()?;
        Ok(result)
    }

    fn validate_indices(&self) -> LeafResult<()> {
        let check = |index: usize| {
            self.debug
                .nodes
                .get(index)
                .map(|_| ())
                .ok_or(INVALID_IMPORT)
        };
        for node in &self.debug.nodes {
            match node {
                NativeDebugNode::List(c)
                | NativeDebugNode::Set(c)
                | NativeDebugNode::Optional(c)
                | NativeDebugNode::Alias(c) => check(*c)?,
                NativeDebugNode::Map(a, b) | NativeDebugNode::Result(a, b) => {
                    check(*a)?;
                    check(*b)?;
                }
                NativeDebugNode::Record(_, fields) => {
                    for (_, child) in fields {
                        check(*child)?;
                    }
                }
                NativeDebugNode::Enum(_, variants) | NativeDebugNode::Machine(_, variants) => {
                    for (_, fields) in variants {
                        for child in fields {
                            check(*child)?;
                        }
                    }
                }
                _ => {}
            }
        }
        for supplement in self.supplements.values() {
            match supplement {
                Supplement::Hidden(c) | Supplement::Interface { child: c, .. } => check(*c)?,
                Supplement::Function {
                    environment: Some(c),
                    ..
                } => check(*c)?,
                Supplement::Builder { fields, .. } => {
                    for child in fields {
                        check(*child)?;
                    }
                }
                Supplement::ListEntries(children) => {
                    for child in children {
                        check(*child)?;
                    }
                }
                Supplement::MapEntries(children) => {
                    for (key, value) in children {
                        check(*key)?;
                        check(*value)?;
                    }
                }
                _ => {}
            }
        }
        // Refuse transparent cycles even in inactive arms. Unvisited type-only
        // opaque nodes need no value-specific evidence until an entry selects them.
        for start in 0..self.debug.nodes.len() {
            let mut index = start;
            let mut finished = false;
            for _ in 0..self.debug.nodes.len() {
                match self.debug.nodes.get(index).ok_or(INVALID_IMPORT)? {
                    NativeDebugNode::Alias(child) => index = *child,
                    NativeDebugNode::Redacted => match self.supplements.get(&index) {
                        Some(Supplement::Hidden(child)) => index = *child,
                        _ => {
                            finished = true;
                            break;
                        }
                    },
                    _ => {
                        finished = true;
                        break;
                    }
                }
            }
            if !finished {
                return Err(INVALID_IMPORT);
            }
        }
        Ok(())
    }

    fn resolve(&self, mut index: usize) -> LeafResult<usize> {
        for _ in 0..self.debug.nodes.len() {
            match self.debug.nodes.get(index).ok_or(INVALID_IMPORT)? {
                NativeDebugNode::Alias(child) => index = *child,
                NativeDebugNode::Redacted => match self.supplements.get(&index) {
                    Some(Supplement::Hidden(child)) => index = *child,
                    Some(Supplement::Resource) => return Ok(index),
                    _ => return Err(INVALID_IMPORT),
                },
                _ => return Ok(index),
            }
        }
        Err(INVALID_IMPORT)
    }

    fn owns(&self, index: usize) -> LeafResult<bool> {
        Ok(
            match self
                .debug
                .nodes
                .get(self.resolve(index)?)
                .ok_or(INVALID_IMPORT)?
            {
                NativeDebugNode::Primitive(kind) => *kind == NativeSortKind::String as u32,
                // These native scalar shapes carry no ordinary-owned registry value.
                // Empty/inactive carriers may mention them without containing authority.
                // Validator::value still refuses every actual visit to those shapes.
                NativeDebugNode::Nothing
                | NativeDebugNode::Actor
                | NativeDebugNode::Capability(_)
                | NativeDebugNode::Redacted
                | NativeDebugNode::Uninhabited => false,
                _ => true,
            },
        )
    }

    fn key_strings(&self, index: usize) -> LeafResult<bool> {
        match self
            .debug
            .nodes
            .get(self.resolve(index)?)
            .ok_or(INVALID_IMPORT)?
        {
            NativeDebugNode::Primitive(kind) if *kind <= 7 || *kind == 10 || *kind == 11 => {
                Ok(*kind == 11)
            }
            _ => Err(INVALID_IMPORT),
        }
    }
}

#[derive(Clone, Copy)]
enum OwnedKind {
    CheckedHook,
    String,
    Bytes,
    Struct,
    List,
    Set,
    Map,
    Sum,
}

struct Validator<'a> {
    values: &'a NativeValues,
    import: &'a Import,
    ticket: u64,
    layout: &'a ImportLayout,
    work: Vec<(usize, NativeField)>,
    owned: Vec<(u64, OwnedKind)>,
    seen: HashSet<u64>,
    string_edges: HashMap<u64, u64>,
}

impl<'a> Validator<'a> {
    fn push(&mut self, index: usize, field: NativeField) -> LeafResult<()> {
        self.work.try_reserve(1).map_err(|_| EXHAUSTED)?;
        self.work.push((index, field));
        Ok(())
    }

    fn claim(&mut self, id: u64, kind: OwnedKind) -> LeafResult<()> {
        if id <= self.ticket {
            return Err(INVALID_IMPORT);
        }
        self.seen.try_reserve(1).map_err(|_| EXHAUSTED)?;
        if !self.seen.insert(id) {
            return Err(INVALID_IMPORT);
        }
        self.owned.try_reserve(1).map_err(|_| EXHAUSTED)?;
        self.owned.push((id, kind));
        Ok(())
    }

    fn string(&mut self, id: u64) -> LeafResult<()> {
        if id <= self.ticket || !self.values.strings.contains_key(&id) {
            return Err(INVALID_IMPORT);
        }
        if let Some(edges) = self.string_edges.get_mut(&id) {
            *edges = edges.checked_add(1).ok_or(EXHAUSTED)?;
        } else {
            self.string_edges.try_reserve(1).map_err(|_| EXHAUSTED)?;
            self.claim(id, OwnedKind::String)?;
            self.string_edges.insert(id, 1);
        }
        Ok(())
    }

    fn record(&mut self, id: u64) -> LeafResult<&'a NativeStruct> {
        if self.values.actors.contains_key(&id) || self.values.equalities.contains_key(&id) {
            return Err(INVALID_IMPORT);
        }
        self.claim(id, OwnedKind::Struct)?;
        self.values.structs.get(&id).ok_or(INVALID_IMPORT)
    }

    fn actor(&self, field: NativeField) -> LeafResult<()> {
        if field.owned || field.pending_depth != 0 {
            return Err(INVALID_IMPORT);
        }
        if self.import.records.contains_key(&field.bits) {
            return Ok(());
        }
        let (base, depth) = self.import.pending.get(&field.bits).ok_or(INVALID_IMPORT)?;
        if *depth == 0 || !self.import.records.contains_key(base) {
            return Err(INVALID_IMPORT);
        }
        Ok(())
    }

    fn validate(mut self, root: NativeField) -> LeafResult<Vec<(u64, OwnedKind)>> {
        self.push(self.layout.debug.root, root)?;
        while let Some((index, field)) = self.work.pop() {
            let index = self.layout.resolve(index)?;
            if field.owned != self.layout.owns(index)? || (field.owned && field.pending_depth != 0)
            {
                return Err(INVALID_IMPORT);
            }
            self.value(index, field)?;
        }
        // Immutable strings may have internal owning aliases (for example an
        // adapter and its captured source label). No unvisited owner may remain.
        for (&id, &edges) in &self.string_edges {
            if self
                .values
                .strings
                .get(&id)
                .ok_or(INVALID_IMPORT)?
                .references
                != edges
            {
                return Err(INVALID_IMPORT);
            }
        }
        Ok(self.owned)
    }

    fn value(&mut self, index: usize, field: NativeField) -> LeafResult<()> {
        let bits = field.bits;
        let layout = self.layout;
        let values = self.values;
        match layout.debug.nodes.get(index).ok_or(INVALID_IMPORT)? {
            NativeDebugNode::Primitive(kind) => {
                let kind = NativeSortKind::from_raw(*kind).map_err(|_| INVALID_IMPORT)?;
                if !kind.valid_bits(bits) {
                    return Err(INVALID_IMPORT);
                }
                if matches!(kind, NativeSortKind::String) {
                    self.string(bits)?;
                }
            }
            NativeDebugNode::Nothing => {}
            NativeDebugNode::Actor => self.actor(field)?,
            NativeDebugNode::Bytes => {
                self.values.bytes.get(&bits).ok_or(INVALID_IMPORT)?;
                self.claim(bits, OwnedKind::Bytes)?;
            }
            NativeDebugNode::List(child) | NativeDebugNode::Set(child) => {
                self.sequence(index, bits, *child)?
            }
            NativeDebugNode::Map(key, value) => self.map(index, bits, *key, *value)?,
            NativeDebugNode::Optional(child) | NativeDebugNode::Result(child, _) => {
                let sum = values.sums.get(&bits).ok_or(INVALID_IMPORT)?;
                if sum.tag > SUM_SUCCESS {
                    return Err(INVALID_IMPORT);
                }
                self.claim(bits, OwnedKind::Sum)?;
                if sum.tag == SUM_FAILURE
                    && matches!(self.layout.debug.nodes[index], NativeDebugNode::Optional(_))
                {
                    if sum.owned || sum.bits != 0 || sum.payload_pending_depth != 0 {
                        return Err(INVALID_IMPORT);
                    }
                } else {
                    let child = match &self.layout.debug.nodes[index] {
                        NativeDebugNode::Result(_, fail) if sum.tag == SUM_FAILURE => *fail,
                        _ => *child,
                    };
                    self.push(
                        child,
                        NativeField {
                            bits: sum.bits,
                            owned: sum.owned,
                            pending_depth: sum.payload_pending_depth,
                        },
                    )?;
                }
            }
            NativeDebugNode::Record(_, fields) => {
                let record = self.record(bits)?;
                if record.fields.len() != fields.len() || self.values.builders.contains_key(&bits) {
                    return Err(INVALID_IMPORT);
                }
                let fields_to_visit = record.fields.clone();
                for ((_, child), field) in fields.iter().zip(fields_to_visit) {
                    self.push(*child, field.ok_or(INVALID_IMPORT)?)?;
                }
            }
            NativeDebugNode::Enum(_, variants) | NativeDebugNode::Machine(_, variants) => {
                let record = self.record(bits)?;
                let tag = record
                    .fields
                    .first()
                    .copied()
                    .flatten()
                    .ok_or(INVALID_IMPORT)?;
                if tag.owned || tag.pending_depth != 0 {
                    return Err(INVALID_IMPORT);
                }
                let tag = usize::try_from(tag.bits).map_err(|_| INVALID_IMPORT)?;
                let (_, fields) = variants.get(tag).ok_or(INVALID_IMPORT)?;
                if record.fields.len() != fields.len() + 1 {
                    return Err(INVALID_IMPORT);
                }
                let fields_to_visit = record.fields[1..].to_vec();
                for (child, field) in fields.iter().zip(fields_to_visit) {
                    self.push(*child, field.ok_or(INVALID_IMPORT)?)?;
                }
            }
            NativeDebugNode::Function => self.function(index, bits)?,
            NativeDebugNode::Interface => self.interface(index, bits)?,
            NativeDebugNode::TypeConstruction => self.builder(index, bits)?,
            NativeDebugNode::Capability(_)
            | NativeDebugNode::Redacted
            | NativeDebugNode::Uninhabited
            | NativeDebugNode::Alias(_) => return Err(INVALID_IMPORT),
        }
        Ok(())
    }

    fn sequence(&mut self, index: usize, bits: u64, child: usize) -> LeafResult<()> {
        let values = self.values;
        let layout = self.layout;
        let owned = self.layout.owns(child)?;
        let (elements, depths) = match &self.layout.debug.nodes[index] {
            NativeDebugNode::List(_) => {
                let list = values.lists.get(&bits).ok_or(INVALID_IMPORT)?;
                if list.owned != owned {
                    return Err(INVALID_IMPORT);
                }
                self.claim(bits, OwnedKind::List)?;
                (&list.elements, &list.element_pending_depths)
            }
            _ => {
                let set = values.sets.get(&bits).ok_or(INVALID_IMPORT)?;
                if set.strings != self.layout.key_strings(child)? {
                    return Err(INVALID_IMPORT);
                }
                self.claim(bits, OwnedKind::Set)?;
                (&set.elements, &set.element_pending_depths)
            }
        };
        if depths.keys().any(|index| *index >= elements.len()) || (owned && !depths.is_empty()) {
            return Err(INVALID_IMPORT);
        }
        let concrete = match layout.supplements.get(&index) {
            Some(Supplement::ListEntries(children)) if children.len() == elements.len() => {
                Some(children)
            }
            Some(Supplement::ListEntries(_)) => return Err(INVALID_IMPORT),
            None => None,
            _ => return Err(INVALID_IMPORT),
        };
        for (position, bits) in elements.iter().enumerate() {
            let child = concrete.map_or(child, |children| children[position]);
            self.push(
                child,
                NativeField {
                    bits: bits.ok_or(INVALID_IMPORT)?,
                    owned,
                    pending_depth: depths.get(&position).copied().unwrap_or(0),
                },
            )?;
        }
        Ok(())
    }

    fn map(&mut self, index: usize, bits: u64, key: usize, value: usize) -> LeafResult<()> {
        let values = self.values;
        let layout = self.layout;
        let map = values.maps.get(&bits).ok_or(INVALID_IMPORT)?;
        if map.key_strings != self.layout.key_strings(key)?
            || map.value_owned != self.layout.owns(value)?
        {
            return Err(INVALID_IMPORT);
        }
        self.claim(bits, OwnedKind::Map)?;
        let concrete = match layout.supplements.get(&index) {
            Some(Supplement::MapEntries(children)) if children.len() == map.entries.len() => {
                Some(children)
            }
            Some(Supplement::MapEntries(_)) => return Err(INVALID_IMPORT),
            None => None,
            _ => return Err(INVALID_IMPORT),
        };
        for (position, entry) in map.entries.iter().enumerate() {
            let entry = entry.ok_or(INVALID_IMPORT)?;
            if entry.key_taken {
                return Err(INVALID_IMPORT);
            }
            let (key, value) = concrete.map_or((key, value), |children| children[position]);
            if layout.key_strings(key)? != map.key_strings {
                return Err(INVALID_IMPORT);
            }
            self.push(
                key,
                NativeField {
                    bits: entry.key,
                    owned: map.key_strings,
                    pending_depth: entry.key_pending_depth,
                },
            )?;
            self.push(
                value,
                NativeField {
                    bits: entry.value,
                    owned: map.value_owned,
                    pending_depth: entry.value_pending_depth,
                },
            )?;
        }
        Ok(())
    }

    fn function(&mut self, index: usize, bits: u64) -> LeafResult<()> {
        let layout = self.layout;
        if let Some(Supplement::CheckedHook {
            descriptor_operation,
            hook,
            signature,
        }) = layout.supplements.get(&index)
        {
            let value = self
                .values
                .checked_graph_hooks
                .get(&bits)
                .ok_or(INVALID_IMPORT)?;
            if !value.matches_factory(self.ticket, *descriptor_operation, *hook, *signature) {
                return Err(INVALID_IMPORT);
            }
            return self.claim(bits, OwnedKind::CheckedHook);
        }
        let Supplement::Function { code, environment } =
            layout.supplements.get(&index).ok_or(INVALID_IMPORT)?
        else {
            return Err(INVALID_IMPORT);
        };
        let record = self.record(bits)?;
        if record.fields.len() != 3 {
            return Err(INVALID_IMPORT);
        }
        let code_field = record.fields[0].ok_or(INVALID_IMPORT)?;
        let env = record.fields[1].ok_or(INVALID_IMPORT)?;
        let label = record.fields[2].ok_or(INVALID_IMPORT)?;
        if code_field.owned
            || code_field.pending_depth != 0
            || code_field.bits != *code
            || !label.owned
            || label.pending_depth != 0
        {
            return Err(INVALID_IMPORT);
        }
        self.values
            .function_debug_label(bits)
            .map_err(|_| INVALID_IMPORT)?;
        match environment {
            Some(child) => self.push(*child, env)?,
            None if env.bits == 0 && !env.owned && env.pending_depth == 0 => {}
            None => return Err(INVALID_IMPORT),
        }
        self.string(label.bits)
    }

    fn interface(&mut self, index: usize, bits: u64) -> LeafResult<()> {
        let layout = self.layout;
        let Supplement::Interface { concrete, child } =
            layout.supplements.get(&index).ok_or(INVALID_IMPORT)?
        else {
            return Err(INVALID_IMPORT);
        };
        let record = self.record(bits)?;
        if record.fields.len() != 3 {
            return Err(INVALID_IMPORT);
        }
        let type_id = record.fields[0].ok_or(INVALID_IMPORT)?;
        let value = record.fields[1].ok_or(INVALID_IMPORT)?;
        let schema = record.fields[2].ok_or(INVALID_IMPORT)?;
        if type_id.owned
            || type_id.pending_depth != 0
            || type_id.bits != *concrete
            || !schema.owned
            || schema.pending_depth != 0
        {
            return Err(INVALID_IMPORT);
        }
        NativeDebugLayout::parse(self.values.bytes.get(&schema.bits).ok_or(INVALID_IMPORT)?)
            .map_err(|_| INVALID_IMPORT)?;
        self.claim(schema.bits, OwnedKind::Bytes)?;
        self.push(*child, value)
    }

    fn builder(&mut self, index: usize, bits: u64) -> LeafResult<()> {
        let layout = self.layout;
        let values = self.values;
        let Supplement::Builder {
            owner,
            offset,
            fields,
        } = layout.supplements.get(&index).ok_or(INVALID_IMPORT)?
        else {
            return Err(INVALID_IMPORT);
        };
        let info = values.builders.get(&bits).ok_or(INVALID_IMPORT)?;
        if info.owner != *owner
            || info.field_offset != *offset
            || info.field_names.len() != fields.len()
            || info.field_debug_layouts.len() != fields.len()
            || info.unsupported_kind.is_some()
        {
            return Err(INVALID_IMPORT);
        }
        let record = self.record(bits)?;
        if record.fields.len() != fields.len().checked_add(*offset).ok_or(INVALID_IMPORT)? {
            return Err(INVALID_IMPORT);
        }
        for prefix in &record.fields[..*offset] {
            let prefix = prefix.ok_or(INVALID_IMPORT)?;
            if prefix.owned || prefix.pending_depth != 0 {
                return Err(INVALID_IMPORT);
            }
        }
        let mut occupied = HashSet::new();
        occupied
            .try_reserve(info.put_order.len())
            .map_err(|_| EXHAUSTED)?;
        for index in &info.put_order {
            if *index >= fields.len() || !occupied.insert(*index) {
                return Err(INVALID_IMPORT);
            }
        }
        let provided = record.fields[*offset..].to_vec();
        for (index, (child, field)) in fields.iter().zip(provided).enumerate() {
            if field.is_some() != occupied.contains(&index) {
                return Err(INVALID_IMPORT);
            }
            NativeDebugLayout::parse(&info.field_debug_layouts[index])
                .map_err(|_| INVALID_IMPORT)?;
            if let Some(field) = field {
                self.push(*child, field)?;
            }
        }
        Ok(())
    }
}

impl NativeValues {
    pub(super) fn begin_actor_graph(&mut self) -> LeafResult<u64> {
        // One sequential evaluator activation owns one unfinished context import.
        if !self.actor_graph_imports.is_empty() {
            return Err(INVALID_IMPORT);
        }
        #[cfg(test)]
        self.allocation_checkpoint()?;
        self.actor_graph_imports
            .try_reserve(1)
            .map_err(|_| EXHAUSTED)?;
        let ticket = next_identity()?;
        self.actor_graph_imports.insert(
            ticket,
            Import {
                bases: Vec::new(),
                records: HashMap::new(),
                pending: HashMap::new(),
                ordinary: NativeValues::default(),
                roots: Vec::new(),
                output: None,
            },
        );
        Ok(ticket)
    }

    pub(super) fn reserve_actor_graph_base(&mut self, ticket: u64, count: u64) -> LeafResult<u64> {
        #[cfg(test)]
        self.allocation_checkpoint()?;
        let count = usize::try_from(count).map_err(|_| EXHAUSTED)?;
        let import = self
            .actor_graph_imports
            .get_mut(&ticket)
            .ok_or(INVALID_IMPORT)?;
        import.records.try_reserve(1).map_err(|_| EXHAUSTED)?;
        import.bases.try_reserve(1).map_err(|_| EXHAUSTED)?;
        let mut fields = Vec::new();
        fields.try_reserve_exact(count).map_err(|_| EXHAUSTED)?;
        fields.resize(count, None);
        let created = self.structs_created.checked_add(1).ok_or(EXHAUSTED)?;
        let id = next_identity()?;
        import.records.insert(
            id,
            NativeStruct {
                fields,
                pending_depth: 0,
            },
        );
        import.bases.push(id);
        self.structs_created = created;
        Ok(id)
    }

    pub(super) fn reserve_actor_graph_pending(
        &mut self,
        ticket: u64,
        base: u64,
        depth: u64,
    ) -> LeafResult<u64> {
        #[cfg(test)]
        self.allocation_checkpoint()?;
        let import = self
            .actor_graph_imports
            .get_mut(&ticket)
            .ok_or(INVALID_IMPORT)?;
        if depth == 0 || !import.records.contains_key(&base) {
            return Err(INVALID_IMPORT);
        }
        import.pending.try_reserve(1).map_err(|_| EXHAUSTED)?;
        let created = self.structs_created.checked_add(1).ok_or(EXHAUSTED)?;
        let id = next_identity()?;
        import.pending.insert(id, (base, depth));
        self.structs_created = created;
        Ok(id)
    }

    pub(super) fn fill_actor_graph(
        &mut self,
        ticket: u64,
        base: u64,
        index: u64,
        field: NativeField,
        bytes: &[u8],
    ) -> LeafResult<u32> {
        let index = usize::try_from(index).map_err(|_| INVALID_IMPORT)?;
        let import = self
            .actor_graph_imports
            .get(&ticket)
            .ok_or(INVALID_IMPORT)?;
        let slot = import
            .records
            .get(&base)
            .and_then(|record| record.fields.get(index))
            .ok_or(INVALID_IMPORT)?;
        if slot.is_some() {
            return Err(INVALID_IMPORT);
        }
        let owned = self.prepare_actor_graph_tree(ticket, field, bytes)?;
        // All validation and allocations precede the first transfer.
        let mut import = self
            .actor_graph_imports
            .remove(&ticket)
            .ok_or(INVALID_IMPORT)?;
        let slot = match import
            .records
            .get_mut(&base)
            .and_then(|record| record.fields.get_mut(index))
        {
            Some(slot) => slot,
            None => {
                self.actor_graph_imports.insert(ticket, import);
                return Err(INVALID_IMPORT);
            }
        };
        transfer_tree(self, &mut import.ordinary, &mut import.roots, field, owned);
        *slot = Some(field);
        self.actor_graph_imports.insert(ticket, import);
        Ok(0)
    }

    pub(super) fn fill_actor_graph_root(
        &mut self,
        ticket: u64,
        field: NativeField,
        bytes: &[u8],
    ) -> LeafResult<u32> {
        if self
            .actor_graph_imports
            .get(&ticket)
            .ok_or(INVALID_IMPORT)?
            .output
            .is_some()
        {
            return Err(INVALID_IMPORT);
        }
        let owned = self.prepare_actor_graph_tree(ticket, field, bytes)?;
        let mut import = self
            .actor_graph_imports
            .remove(&ticket)
            .ok_or(INVALID_IMPORT)?;
        transfer_tree(self, &mut import.ordinary, &mut import.roots, field, owned);
        import.output = Some(field);
        self.actor_graph_imports.insert(ticket, import);
        Ok(0)
    }

    fn prepare_actor_graph_tree(
        &mut self,
        ticket: u64,
        field: NativeField,
        bytes: &[u8],
    ) -> LeafResult<Vec<(u64, OwnedKind)>> {
        let layout = ImportLayout::parse(bytes)?;
        let import = self
            .actor_graph_imports
            .get(&ticket)
            .ok_or(INVALID_IMPORT)?;
        let owned = Validator {
            values: self,
            import,
            ticket,
            layout: &layout,
            work: Vec::new(),
            owned: Vec::new(),
            seen: HashSet::new(),
            string_edges: HashMap::new(),
        }
        .validate(field)?;
        #[cfg(test)]
        self.allocation_checkpoint()?;
        let import = self
            .actor_graph_imports
            .get_mut(&ticket)
            .ok_or(INVALID_IMPORT)?;
        reserve_transferred_capacity(&mut import.ordinary, &owned)?;
        if field.owned {
            import.roots.try_reserve(1).map_err(|_| EXHAUSTED)?;
        }
        Ok(owned)
    }

    pub(super) fn commit_actor_graph(&mut self, ticket: u64) -> LeafResult<u32> {
        let import = self
            .actor_graph_imports
            .get(&ticket)
            .ok_or(INVALID_IMPORT)?;
        if (import.bases.is_empty() && import.output.is_none())
            || import
                .records
                .values()
                .any(|record| record.fields.iter().any(Option::is_none))
        {
            return Err(INVALID_IMPORT);
        }
        let count = u64::try_from(import.bases.len()).map_err(|_| EXHAUSTED)?;
        let next_ordinal = self
            .next_actor_ordinal
            .checked_add(count)
            .ok_or(EXHAUSTED)?;
        let count = import
            .records
            .len()
            .checked_add(import.pending.len())
            .ok_or(EXHAUSTED)?;
        let records = count
            .checked_add(import.ordinary.structs.len())
            .ok_or(EXHAUSTED)?;
        let mut aliases = Vec::new();
        aliases
            .try_reserve_exact(import.pending.len())
            .map_err(|_| EXHAUSTED)?;
        for (&id, &(base, depth)) in &import.pending {
            let offset = import
                .bases
                .iter()
                .position(|candidate| *candidate == base)
                .ok_or(INVALID_IMPORT)?;
            if depth == 0 {
                return Err(INVALID_IMPORT);
            }
            aliases.push((
                id,
                NativeActorIdentity {
                    instance: base,
                    ordinal: self.next_actor_ordinal + offset as u64,
                },
                depth,
            ));
        }
        #[cfg(test)]
        self.allocation_checkpoint()?;
        self.structs.try_reserve(records).map_err(|_| EXHAUSTED)?;
        self.actors.try_reserve(count).map_err(|_| EXHAUSTED)?;
        reserve_publication_capacity(self, ticket)?;
        // The leaf holds the context state mutex: no fallible work after removal.
        let import = self
            .actor_graph_imports
            .remove(&ticket)
            .ok_or(INVALID_IMPORT)?;
        let first_ordinal = self.next_actor_ordinal;
        for (offset, id) in import.bases.iter().copied().enumerate() {
            self.actors.insert(
                id,
                NativeActorIdentity {
                    instance: id,
                    ordinal: first_ordinal + offset as u64,
                },
            );
        }
        for (id, identity, depth) in aliases {
            self.actors.insert(id, identity);
            self.structs.insert(
                id,
                NativeStruct {
                    fields: Vec::new(),
                    pending_depth: depth,
                },
            );
        }
        self.structs.extend(import.records);
        publish_ordinary(self, import.ordinary);
        self.next_actor_ordinal = next_ordinal;
        Ok(0)
    }

    pub(super) fn abort_actor_graph(&mut self, ticket: u64) -> LeafResult<u32> {
        let mut import = self
            .actor_graph_imports
            .remove(&ticket)
            .ok_or(INVALID_IMPORT)?;
        let mut first_failure = None;
        for root in import.roots {
            if let Err(error) = import.ordinary.drop_value(root) {
                first_failure.get_or_insert(error);
            }
        }
        self.structs_destroyed += import.records.len() as u64 + import.pending.len() as u64;
        self.structs_destroyed += import.ordinary.structs_destroyed;
        self.lists_destroyed += import.ordinary.lists_destroyed;
        self.sets_destroyed += import.ordinary.sets_destroyed;
        self.maps_destroyed += import.ordinary.maps_destroyed;
        self.sums_destroyed += import.ordinary.sums_destroyed;
        self.bytes_destroyed += import.ordinary.bytes_destroyed;
        self.checked_graph_hooks_destroyed += import.ordinary.checked_graph_hooks_destroyed;
        if !import.ordinary.registries_empty() {
            first_failure.get_or_insert(INVALID_IMPORT);
        }
        first_failure.map_or(Ok(0), Err)
    }

    pub(super) fn abort_unfinished_actor_graphs(&mut self) -> LeafResult<()> {
        let mut first_failure = None;
        while let Some(&ticket) = self.actor_graph_imports.keys().next() {
            if let Err(error) = self.abort_actor_graph(ticket) {
                first_failure.get_or_insert(error);
            }
        }
        first_failure.map_or(Ok(()), Err)
    }

    fn registries_empty(&self) -> bool {
        self.strings.is_empty()
            && self.bytes.is_empty()
            && self.bytes_pending.is_empty()
            && self.sums.is_empty()
            && self.lists.is_empty()
            && self.sets.is_empty()
            && self.maps.is_empty()
            && self.structs.is_empty()
            && self.builders.is_empty()
            && self.actors.is_empty()
            && self.equalities.is_empty()
            && self.actor_graph_imports.is_empty()
            && self.checked_graph_hooks.is_empty()
    }
}

fn reserve_transferred_capacity(
    values: &mut NativeValues,
    owned: &[(u64, OwnedKind)],
) -> LeafResult<()> {
    let count = |predicate: fn(OwnedKind) -> bool| {
        owned.iter().filter(|(_, kind)| predicate(*kind)).count()
    };
    values
        .checked_graph_hooks
        .try_reserve(count(|k| matches!(k, OwnedKind::CheckedHook)))
        .map_err(|_| EXHAUSTED)?;
    values
        .strings
        .try_reserve(count(|k| matches!(k, OwnedKind::String)))
        .map_err(|_| EXHAUSTED)?;
    let bytes = count(|k| matches!(k, OwnedKind::Bytes));
    values.bytes.try_reserve(bytes).map_err(|_| EXHAUSTED)?;
    values
        .bytes_pending
        .try_reserve(bytes)
        .map_err(|_| EXHAUSTED)?;
    let records = count(|k| matches!(k, OwnedKind::Struct));
    values.structs.try_reserve(records).map_err(|_| EXHAUSTED)?;
    values
        .builders
        .try_reserve(records)
        .map_err(|_| EXHAUSTED)?;
    values
        .lists
        .try_reserve(count(|k| matches!(k, OwnedKind::List)))
        .map_err(|_| EXHAUSTED)?;
    values
        .sets
        .try_reserve(count(|k| matches!(k, OwnedKind::Set)))
        .map_err(|_| EXHAUSTED)?;
    values
        .maps
        .try_reserve(count(|k| matches!(k, OwnedKind::Map)))
        .map_err(|_| EXHAUSTED)?;
    values
        .sums
        .try_reserve(count(|k| matches!(k, OwnedKind::Sum)))
        .map_err(|_| EXHAUSTED)?;
    Ok(())
}

fn transfer_tree(
    source: &mut NativeValues,
    destination: &mut NativeValues,
    roots: &mut Vec<u64>,
    field: NativeField,
    owned: Vec<(u64, OwnedKind)>,
) {
    for (id, kind) in owned {
        transfer_value(source, destination, id, kind);
    }
    if field.owned {
        roots.push(field.bits);
    }
}

fn transfer_value(
    source: &mut NativeValues,
    destination: &mut NativeValues,
    id: u64,
    kind: OwnedKind,
) {
    match kind {
        OwnedKind::CheckedHook => {
            if let Some(value) = source.checked_graph_hooks.remove(&id) {
                destination.checked_graph_hooks.insert(id, value);
            }
        }
        OwnedKind::String => {
            if let Some(value) = source.strings.remove(&id) {
                destination.strings.insert(id, value);
            }
        }
        OwnedKind::Bytes => {
            if let Some(value) = source.bytes.remove(&id) {
                destination.bytes.insert(id, value);
            }
            if let Some(depth) = source.bytes_pending.remove(&id) {
                destination.bytes_pending.insert(id, depth);
            }
        }
        OwnedKind::Struct => {
            if let Some(value) = source.structs.remove(&id) {
                destination.structs.insert(id, value);
            }
            if let Some(info) = source.builders.remove(&id) {
                destination.builders.insert(id, info);
            }
        }
        OwnedKind::List => {
            if let Some(value) = source.lists.remove(&id) {
                destination.lists.insert(id, value);
            }
        }
        OwnedKind::Set => {
            if let Some(value) = source.sets.remove(&id) {
                destination.sets.insert(id, value);
            }
        }
        OwnedKind::Map => {
            if let Some(value) = source.maps.remove(&id) {
                destination.maps.insert(id, value);
            }
        }
        OwnedKind::Sum => {
            if let Some(value) = source.sums.remove(&id) {
                destination.sums.insert(id, value);
            }
        }
    }
}

fn reserve_publication_capacity(values: &mut NativeValues, ticket: u64) -> LeafResult<()> {
    let import = values
        .actor_graph_imports
        .get(&ticket)
        .ok_or(INVALID_IMPORT)?;
    let staged = &import.ordinary;
    values
        .checked_graph_hooks
        .try_reserve(staged.checked_graph_hooks.len())
        .map_err(|_| EXHAUSTED)?;
    values
        .strings
        .try_reserve(staged.strings.len())
        .map_err(|_| EXHAUSTED)?;
    values
        .bytes
        .try_reserve(staged.bytes.len())
        .map_err(|_| EXHAUSTED)?;
    values
        .bytes_pending
        .try_reserve(staged.bytes_pending.len())
        .map_err(|_| EXHAUSTED)?;
    values
        .builders
        .try_reserve(staged.builders.len())
        .map_err(|_| EXHAUSTED)?;
    values
        .lists
        .try_reserve(staged.lists.len())
        .map_err(|_| EXHAUSTED)?;
    values
        .sets
        .try_reserve(staged.sets.len())
        .map_err(|_| EXHAUSTED)?;
    values
        .maps
        .try_reserve(staged.maps.len())
        .map_err(|_| EXHAUSTED)?;
    values
        .sums
        .try_reserve(staged.sums.len())
        .map_err(|_| EXHAUSTED)?;
    Ok(())
}

fn publish_ordinary(values: &mut NativeValues, mut staged: NativeValues) {
    for value in staged.checked_graph_hooks.values_mut() {
        value.publish();
    }
    values
        .checked_graph_hooks
        .extend(staged.checked_graph_hooks);
    values.strings.extend(staged.strings);
    values.bytes.extend(staged.bytes);
    values.bytes_pending.extend(staged.bytes_pending);
    values.builders.extend(staged.builders);
    values.structs.extend(staged.structs);
    values.lists.extend(staged.lists);
    values.sets.extend(staged.sets);
    values.maps.extend(staged.maps);
    values.sums.extend(staged.sums);
}

#[cfg(test)]
mod tests;
