//! Iterative equality for the existing supported aggregate enum payloads.
use super::*;
use std::collections::HashSet;

type Visit = (usize, u64, u64);

enum Frame {
    Value(usize, u64, u64),
    Fields(usize, Option<NativeField>, Option<NativeField>),
    Elements {
        node: usize,
        left: Option<u64>,
        right: Option<u64>,
        left_depth: u64,
        right_depth: u64,
        failure: Failure,
    },
    Entry(usize, usize, Option<NativeMapEntry>, Option<NativeMapEntry>),
    Leave(Visit),
}

struct Comparison<'a> {
    layout: &'a NativeDebugLayout,
    values: &'a NativeValues,
    active: &'a mut HashSet<Visit>,
    work: &'a mut Vec<Frame>,
}

impl NativeDebugLayout {
    pub(super) fn equal_value(
        &self,
        values: &NativeValues,
        left: u64,
        right: u64,
        index: usize,
    ) -> LeafResult<bool> {
        let mut active = HashSet::new();
        let mut work = vec![Frame::Value(index, left, right)];
        let mut comparison = Comparison {
            layout: self,
            values,
            active: &mut active,
            work: &mut work,
        };
        while let Some(frame) = comparison.work.pop() {
            if !comparison.step(frame)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Runtime traversal state only. Generated code invokes source methods after
/// the leaf has returned and released the runtime context lock.
pub(super) struct EqualityCursor {
    layout: NativeDebugLayout,
    custom: HashMap<usize, u32>,
    active: HashSet<Visit>,
    work: Vec<Frame>,
    request: Option<EqualityRequest>,
    complete: Option<bool>,
}

struct EqualityRequest {
    arguments: [u64; 2],
    owned: bool,
}

impl EqualityCursor {
    fn parse(bytes: &[u8], left: u64, right: u64) -> LeafResult<Self> {
        let mut input = bytes.strip_prefix(b"JQ\x01").ok_or(INVALID_STRUCT)?;
        fn number(input: &mut &[u8]) -> LeafResult<u32> {
            let (head, tail) = input.split_at_checked(4).ok_or(INVALID_STRUCT)?;
            *input = tail;
            Ok(u32::from_le_bytes(
                head.try_into().map_err(|_| INVALID_STRUCT)?,
            ))
        }
        let count = number(&mut input)?;
        if count as usize > input.len() / 4 {
            return Err(INVALID_STRUCT);
        }
        let mut custom = HashMap::new();
        custom.try_reserve(count as usize).map_err(|_| EXHAUSTED)?;
        for ordinal in 0..count {
            let node = number(&mut input)? as usize;
            if custom.insert(node, ordinal).is_some() {
                return Err(INVALID_STRUCT);
            }
        }
        let layout = NativeDebugLayout::parse(input)?;
        for node in custom.keys() {
            if !matches!(layout.nodes.get(*node), Some(NativeDebugNode::Record(_, fields)) if fields.is_empty())
            {
                return Err(INVALID_STRUCT);
            }
        }
        let root = layout.root;
        Ok(Self {
            layout,
            custom,
            active: HashSet::new(),
            work: vec![Frame::Value(root, left, right)],
            request: None,
            complete: None,
        })
    }

    fn advance(&mut self, values: &mut NativeValues) -> LeafResult<u32> {
        if let Some(equal) = self.complete {
            return Ok(u32::from(equal));
        }
        if self.request.is_some() {
            return Err(INVALID_STRUCT);
        }
        while let Some(frame) = self.work.pop() {
            if let Frame::Value(node, left, right) = frame {
                if let Some(&ordinal) = self.custom.get(&node) {
                    let left_depth = values
                        .structs
                        .get(&left)
                        .ok_or(INVALID_STRUCT)?
                        .pending_depth;
                    let right_depth = values
                        .structs
                        .get(&right)
                        .ok_or(INVALID_STRUCT)?
                        .pending_depth;
                    if left_depth != right_depth {
                        self.complete = Some(false);
                        return Ok(0);
                    }
                    let arguments = if left_depth == 0 {
                        [left, right]
                    } else {
                        // A view shell unwraps the pending record without copying
                        // or inspecting fields that the method may ignore.
                        let left = values.equality_view(left)?;
                        match values.equality_view(right) {
                            Ok(right) => [left, right],
                            Err(error) => {
                                values.drop_value(left)?;
                                return Err(error);
                            }
                        }
                    };
                    self.request = Some(EqualityRequest {
                        arguments,
                        owned: left_depth != 0,
                    });
                    return ordinal.checked_add(2).ok_or(EXHAUSTED);
                }
            }
            let equal = Comparison {
                layout: &self.layout,
                values,
                active: &mut self.active,
                work: &mut self.work,
            }
            .step(frame)?;
            if !equal {
                self.complete = Some(false);
                return Ok(0);
            }
        }
        self.complete = Some(true);
        Ok(1)
    }

    fn retire_request(&mut self, values: &mut NativeValues) -> LeafResult<()> {
        if let Some(request) = self.request.take() {
            if request.owned {
                for argument in request.arguments {
                    values.drop_value(argument)?;
                }
            }
        }
        Ok(())
    }
}

impl NativeValues {
    fn equality_view(&mut self, id: u64) -> LeafResult<u64> {
        let fields = self.structs.get(&id).ok_or(INVALID_STRUCT)?.fields.clone();
        let view = self.new_struct(fields.len() as u64)?;
        self.structs.get_mut(&view).ok_or(INVALID_STRUCT)?.fields = fields
            .into_iter()
            .map(|field| {
                field.map(|field| NativeField {
                    owned: false,
                    ..field
                })
            })
            .collect();
        Ok(view)
    }

    pub(super) fn equality_start(
        &mut self,
        left: u64,
        right: u64,
        bytes: &[u8],
    ) -> LeafResult<u64> {
        #[cfg(test)]
        self.allocation_checkpoint()?;
        let cursor = EqualityCursor::parse(bytes, left, right)?;
        let id = next_identity()?;
        self.equalities.try_reserve(1).map_err(|_| EXHAUSTED)?;
        self.equalities.insert(id, cursor);
        Ok(id)
    }

    pub(super) fn equality_next(&mut self, id: u64) -> LeafResult<u32> {
        let mut cursor = self.equalities.remove(&id).ok_or(INVALID_STRUCT)?;
        let result = cursor.advance(self);
        self.equalities.insert(id, cursor);
        result
    }

    pub(super) fn equality_argument(&self, id: u64, argument: u32) -> LeafResult<u64> {
        let request = self
            .equalities
            .get(&id)
            .and_then(|cursor| cursor.request.as_ref())
            .ok_or(INVALID_STRUCT)?;
        request
            .arguments
            .get(argument as usize)
            .copied()
            .ok_or(INVALID_STRUCT)
    }

    pub(super) fn equality_answer(&mut self, id: u64, equal: u32, depth: u64) -> LeafResult<u32> {
        if depth != 0 {
            return Err((
                JettRuntimeStatusV1::INVALID_ARGUMENT,
                b"Equatable.equals must return bool",
            ));
        }
        if equal > 1 {
            return Err(INVALID_STRUCT);
        }
        let mut cursor = self.equalities.remove(&id).ok_or(INVALID_STRUCT)?;
        let result = if cursor.request.is_some() {
            cursor.retire_request(self).map(|()| {
                if equal == 0 {
                    cursor.complete = Some(false);
                }
                0
            })
        } else {
            Err(INVALID_STRUCT)
        };
        self.equalities.insert(id, cursor);
        result
    }

    pub(super) fn equality_drop(&mut self, mut cursor: EqualityCursor) -> LeafResult<u32> {
        cursor.retire_request(self)?;
        Ok(0)
    }
}

impl Comparison<'_> {
    fn step(&mut self, frame: Frame) -> LeafResult<bool> {
        match frame {
            Frame::Value(index, left, right) => self.value(index, left, right),
            Frame::Leave(visit) => {
                self.active.remove(&visit);
                Ok(true)
            }
            Frame::Fields(index, left, right) => {
                let left = left.ok_or(INVALID_STRUCT)?;
                let right = right.ok_or(INVALID_STRUCT)?;
                if left.pending_depth != right.pending_depth {
                    return Ok(false);
                }
                self.work.push(Frame::Value(index, left.bits, right.bits));
                Ok(true)
            }
            Frame::Elements {
                node,
                left,
                right,
                left_depth,
                right_depth,
                failure,
            } => {
                if left_depth != right_depth {
                    return Ok(false);
                }
                self.work.push(Frame::Value(
                    node,
                    left.ok_or(failure)?,
                    right.ok_or(failure)?,
                ));
                Ok(true)
            }
            Frame::Entry(key, value, left, right) => {
                let left = left.ok_or(INVALID_MAP)?;
                let right = right.ok_or(INVALID_MAP)?;
                if left.key_taken || right.key_taken {
                    return Err(INVALID_MAP);
                }
                if left.key_pending_depth != right.key_pending_depth
                    || left.value_pending_depth != right.value_pending_depth
                {
                    return Ok(false);
                }
                self.work.push(Frame::Value(value, left.value, right.value));
                self.work.push(Frame::Value(key, left.key, right.key));
                Ok(true)
            }
        }
    }

    fn value(&mut self, index: usize, left_bits: u64, right_bits: u64) -> LeafResult<bool> {
        let visit = (index, left_bits, right_bits);
        if !self.active.insert(visit) {
            return Err(INVALID_STRUCT);
        }
        self.work.push(Frame::Leave(visit));
        let values = self.values;
        match self.layout.nodes.get(index).ok_or(INVALID_STRUCT)? {
            NativeDebugNode::Primitive(raw) => Ok(
                match NativeSortKind::from_raw(*raw).map_err(|_| INVALID_STRUCT)? {
                    NativeSortKind::Float32 => {
                        f32::from_bits(left_bits as u32) == f32::from_bits(right_bits as u32)
                    }
                    NativeSortKind::Float64 => {
                        f64::from_bits(left_bits) == f64::from_bits(right_bits)
                    }
                    NativeSortKind::String => values.same_string_value(left_bits, right_bits)?,
                    _ => left_bits == right_bits,
                },
            ),
            NativeDebugNode::Nothing => Ok(left_bits == right_bits),
            NativeDebugNode::Bytes => values.same_bytes_value(left_bits, right_bits),
            NativeDebugNode::Alias(base) => {
                self.work.push(Frame::Value(*base, left_bits, right_bits));
                Ok(true)
            }
            node @ (NativeDebugNode::List(element) | NativeDebugNode::Set(element)) => {
                let (left, right, left_depths, right_depths, left_depth, right_depth, failure) =
                    match node {
                        NativeDebugNode::List(_) => {
                            let left = values.lists.get(&left_bits).ok_or(INVALID_LIST)?;
                            let right = values.lists.get(&right_bits).ok_or(INVALID_LIST)?;
                            (
                                &left.elements,
                                &right.elements,
                                &left.element_pending_depths,
                                &right.element_pending_depths,
                                left.pending_depth,
                                right.pending_depth,
                                INVALID_LIST,
                            )
                        }
                        _ => {
                            let left = values.sets.get(&left_bits).ok_or(INVALID_SET)?;
                            let right = values.sets.get(&right_bits).ok_or(INVALID_SET)?;
                            (
                                &left.elements,
                                &right.elements,
                                &left.element_pending_depths,
                                &right.element_pending_depths,
                                left.pending_depth,
                                right.pending_depth,
                                INVALID_SET,
                            )
                        }
                    };
                if left_depth != right_depth || left.len() != right.len() {
                    return Ok(false);
                }
                for (position, (left, right)) in left.iter().zip(right).enumerate().rev() {
                    self.work.push(Frame::Elements {
                        node: *element,
                        left: *left,
                        right: *right,
                        left_depth: left_depths.get(&position).copied().unwrap_or(0),
                        right_depth: right_depths.get(&position).copied().unwrap_or(0),
                        failure,
                    });
                }
                Ok(true)
            }
            NativeDebugNode::Map(key, value) => {
                let left = values.maps.get(&left_bits).ok_or(INVALID_MAP)?;
                let right = values.maps.get(&right_bits).ok_or(INVALID_MAP)?;
                if left.pending_depth != right.pending_depth
                    || left.entries.len() != right.entries.len()
                {
                    return Ok(false);
                }
                for (left, right) in left.entries.iter().zip(&right.entries).rev() {
                    self.work.push(Frame::Entry(*key, *value, *left, *right));
                }
                Ok(true)
            }
            node @ (NativeDebugNode::Optional(element) | NativeDebugNode::Result(element, _)) => {
                let left = values.sums.get(&left_bits).ok_or(INVALID_SUM)?;
                let right = values.sums.get(&right_bits).ok_or(INVALID_SUM)?;
                if left.pending_depth != right.pending_depth
                    || left.payload_pending_depth != right.payload_pending_depth
                    || left.tag != right.tag
                {
                    return Ok(false);
                }
                let payload = match (node, left.tag) {
                    (NativeDebugNode::Optional(_), SUM_FAILURE) => return Ok(true),
                    (_, SUM_SUCCESS) => *element,
                    (NativeDebugNode::Result(_, error), SUM_FAILURE) => *error,
                    _ => return Err(INVALID_SUM),
                };
                self.work.push(Frame::Value(payload, left.bits, right.bits));
                Ok(true)
            }
            NativeDebugNode::Record(_, fields) => {
                let left = values.structs.get(&left_bits).ok_or(INVALID_STRUCT)?;
                let right = values.structs.get(&right_bits).ok_or(INVALID_STRUCT)?;
                if left.pending_depth != right.pending_depth {
                    return Ok(false);
                }
                if left.fields.len() != fields.len() || right.fields.len() != fields.len() {
                    return Err(INVALID_STRUCT);
                }
                for (position, (_, field_type)) in fields.iter().enumerate().rev() {
                    self.work.push(Frame::Fields(
                        *field_type,
                        left.fields[position],
                        right.fields[position],
                    ));
                }
                Ok(true)
            }
            NativeDebugNode::Enum(_, variants) | NativeDebugNode::Machine(_, variants) => {
                let left = values.structs.get(&left_bits).ok_or(INVALID_STRUCT)?;
                let right = values.structs.get(&right_bits).ok_or(INVALID_STRUCT)?;
                if left.pending_depth != right.pending_depth {
                    return Ok(false);
                }
                let left_tag = usize::try_from(
                    left.fields
                        .first()
                        .copied()
                        .flatten()
                        .ok_or(INVALID_STRUCT)?
                        .bits,
                )
                .map_err(|_| INVALID_STRUCT)?;
                let right_tag = usize::try_from(
                    right
                        .fields
                        .first()
                        .copied()
                        .flatten()
                        .ok_or(INVALID_STRUCT)?
                        .bits,
                )
                .map_err(|_| INVALID_STRUCT)?;
                if left_tag != right_tag {
                    return Ok(false);
                }
                let (_, fields) = variants.get(left_tag).ok_or(INVALID_STRUCT)?;
                if left.fields.len() != fields.len() + 1 || right.fields.len() != fields.len() + 1 {
                    return Err(INVALID_STRUCT);
                }
                for (position, field_type) in fields.iter().enumerate().rev() {
                    self.work.push(Frame::Fields(
                        *field_type,
                        left.fields[position + 1],
                        right.fields[position + 1],
                    ));
                }
                Ok(true)
            }
            NativeDebugNode::Capability(_)
            | NativeDebugNode::Uninhabited
            | NativeDebugNode::Redacted
            | NativeDebugNode::Interface
            | NativeDebugNode::Function
            | NativeDebugNode::Actor
            | NativeDebugNode::TypeConstruction => Err(INVALID_STRUCT),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(bits: u64) -> Option<NativeField> {
        Some(NativeField {
            bits,
            owned: false,
            pending_depth: 0,
        })
    }

    fn custom_cursor(values: &mut NativeValues, left: u64, right: u64) -> u64 {
        let id = next_identity().unwrap();
        values.equalities.insert(
            id,
            EqualityCursor {
                layout: NativeDebugLayout {
                    root: 0,
                    nodes: vec![NativeDebugNode::Record("Opaque".into(), vec![])],
                },
                custom: HashMap::from([(0, 0)]),
                active: HashSet::new(),
                work: vec![Frame::Value(0, left, right)],
                request: None,
                complete: None,
            },
        );
        id
    }

    #[test]
    fn custom_cursors_are_independent_and_ignore_struct_fields() {
        let mut values = NativeValues::default();
        let left = values.new_struct(1).unwrap();
        let right = values.new_struct(1).unwrap();
        for id in [left, right] {
            values.structs.get_mut(&id).unwrap().pending_depth = 1;
        }
        let first = custom_cursor(&mut values, left, right);
        assert_eq!(values.equality_next(first), Ok(2));
        let argument = values.equality_argument(first, 0).unwrap();
        assert_ne!(argument, left);
        assert_eq!(values.structs[&argument].pending_depth, 0);
        assert!(values.structs[&argument].fields[0].is_none());
        let nested = custom_cursor(&mut values, left, right);
        assert_eq!(values.equality_next(nested), Ok(2));
        values.equality_answer(nested, 0, 0).unwrap();
        assert_eq!(values.equality_next(nested), Ok(0));
        values.drop_value(nested).unwrap();
        assert_eq!(values.equality_argument(first, 0), Ok(argument));
        values.equality_answer(first, 1, 0).unwrap();
        assert!(!values.structs.contains_key(&argument));
        assert_eq!(values.equality_next(first), Ok(1));
        for id in [first, left, right] {
            values.drop_value(id).unwrap();
        }
        assert!(values.is_empty());
    }

    #[test]
    fn custom_cursor_cleans_partial_views_and_pending_method_results() {
        let mut values = NativeValues::default();
        let left = values.new_struct(1).unwrap();
        let right = values.new_struct(1).unwrap();
        for id in [left, right] {
            values.structs.get_mut(&id).unwrap().pending_depth = 1;
        }
        let partial = custom_cursor(&mut values, left, right);
        values.allocation_budget = Some(1);
        assert_eq!(values.equality_next(partial), Err(EXHAUSTED));
        values.allocation_budget = None;
        values.drop_value(partial).unwrap();
        assert_eq!(values.structs.len(), 2);
        let cursor = custom_cursor(&mut values, left, right);
        assert_eq!(values.equality_next(cursor), Ok(2));
        assert!(values.equality_answer(cursor, 1, 1).is_err());
        assert_eq!(values.structs.len(), 4);
        for id in [cursor, left, right] {
            values.drop_value(id).unwrap();
        }
        assert!(values.is_empty());
    }

    #[test]
    fn custom_cursor_preserves_deep_iterative_traversal() {
        let mut values = NativeValues::default();
        let leaf = values.new_struct(1).unwrap();
        let mut root = leaf;
        let mut handles = vec![leaf];
        for _ in 0..4096 {
            let next = values.new_struct(2).unwrap();
            values.structs.get_mut(&next).unwrap().fields = vec![field(1), field(root)];
            handles.push(next);
            root = next;
        }
        let base = values.new_struct(2).unwrap();
        values.structs.get_mut(&base).unwrap().fields = vec![field(0), field(leaf)];
        values.structs.get_mut(&handles[1]).unwrap().fields[1] = field(base);
        handles.push(base);
        let cursor = custom_cursor(&mut values, root, root);
        let state = values.equalities.get_mut(&cursor).unwrap();
        state.layout.nodes = vec![
            NativeDebugNode::Enum(
                "Chain".into(),
                vec![("leaf".into(), vec![1]), ("next".into(), vec![0])],
            ),
            NativeDebugNode::Record("Opaque".into(), vec![]),
        ];
        state.custom = HashMap::from([(1, 0)]);
        assert_eq!(values.equality_next(cursor), Ok(2));
        assert_eq!(values.equality_argument(cursor, 0), Ok(leaf));
        values.equality_answer(cursor, 1, 0).unwrap();
        assert_eq!(values.equality_next(cursor), Ok(1));
        values.drop_value(cursor).unwrap();
        for id in handles {
            values.drop_value(id).unwrap();
        }
        assert!(values.is_empty());
    }

    fn chain(values: &mut NativeValues, leaf: f64) -> (u64, Vec<u64>) {
        let base = values.new_struct(2).unwrap();
        values.structs.get_mut(&base).unwrap().fields = vec![field(0), field(leaf.to_bits())];
        let mut handles = vec![base];
        let mut current = base;
        for _ in 0..4096 {
            let next = values.new_struct(2).unwrap();
            values.structs.get_mut(&next).unwrap().fields = vec![field(1), field(current)];
            handles.push(next);
            current = next;
        }
        (current, handles)
    }

    #[test]
    fn deep_equality_preserves_nan_signed_zero_and_shared_children() {
        let layout = NativeDebugLayout {
            root: 0,
            nodes: vec![
                NativeDebugNode::Enum(
                    "Chain".into(),
                    vec![
                        ("leaf".into(), vec![1]),
                        ("next".into(), vec![0]),
                        ("pair".into(), vec![0, 0]),
                    ],
                ),
                NativeDebugNode::Primitive(NativeSortKind::Float64 as u32),
            ],
        };
        let mut values = NativeValues::default();
        let (left, mut handles) = chain(&mut values, 0.0);
        let (right, other) = chain(&mut values, -0.0);
        assert_eq!(layout.equal_value(&values, left, right, 0), Ok(true));
        values.structs.get_mut(&other[0]).unwrap().fields[1] = field(1.0f64.to_bits());
        assert_eq!(layout.equal_value(&values, left, right, 0), Ok(false));
        values.structs.get_mut(&other[0]).unwrap().fields[1] = field(f64::NAN.to_bits());
        assert_eq!(layout.equal_value(&values, right, right, 0), Ok(false));
        let pair = values.new_struct(3).unwrap();
        values.structs.get_mut(&pair).unwrap().fields = vec![field(2), field(left), field(left)];
        assert_eq!(layout.equal_value(&values, pair, pair, 0), Ok(true));
        handles.extend(other);
        handles.push(pair);
        for handle in handles {
            values.drop_value(handle).unwrap();
        }
        assert!(values.is_empty());
    }

    #[test]
    fn equality_rejects_cycles_and_checks_later_fields_only_after_equal_prefixes() {
        let mut values = NativeValues::default();
        let left = values.new_struct(2).unwrap();
        let right = values.new_struct(2).unwrap();
        values.structs.get_mut(&left).unwrap().fields = vec![field(1), None];
        values.structs.get_mut(&right).unwrap().fields = vec![field(2), None];
        let layout = NativeDebugLayout {
            root: 0,
            nodes: vec![
                NativeDebugNode::Record(
                    "Pair".into(),
                    vec![("first".into(), 1), ("second".into(), 1)],
                ),
                NativeDebugNode::Primitive(NativeSortKind::Int64 as u32),
            ],
        };
        assert_eq!(layout.equal_value(&values, left, right, 0), Ok(false));
        values.structs.get_mut(&right).unwrap().fields[0] = field(1);
        assert_eq!(
            layout.equal_value(&values, left, right, 0),
            Err(INVALID_STRUCT)
        );
        let aliases = NativeDebugLayout {
            root: 0,
            nodes: vec![NativeDebugNode::Alias(0)],
        };
        assert_eq!(aliases.equal_value(&values, 0, 0, 0), Err(INVALID_STRUCT));
        let recursive = NativeDebugLayout {
            root: 0,
            nodes: vec![NativeDebugNode::Record(
                "Loop".into(),
                vec![("first".into(), 0), ("second".into(), 0)],
            )],
        };
        values.structs.get_mut(&left).unwrap().fields = vec![field(left), field(left)];
        assert_eq!(
            recursive.equal_value(&values, left, left, 0),
            Err(INVALID_STRUCT)
        );
        for handle in [left, right] {
            values.drop_value(handle).unwrap();
        }
        assert!(values.is_empty());
    }
}
