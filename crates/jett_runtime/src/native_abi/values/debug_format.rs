//! Iterative typed observation of finite values, with active-path cycle checks.
use super::*;
use std::collections::HashSet;
use std::rc::Rc;

type Layout = Rc<NativeDebugLayout>;
type Visit = (usize, usize, u64);

enum Frame {
    Value(Layout, usize, u64, u64),
    Text(String),
    Leave(Layout, usize, u64),
}

struct Formatter<'a> {
    values: &'a NativeValues,
    layouts: HashMap<Vec<u8>, Layout>,
    active: HashSet<Visit>,
    work: Vec<Frame>,
    text: String,
}

impl NativeDebugLayout {
    pub(super) fn format_value(
        &self,
        values: &NativeValues,
        bits: u64,
        index: usize,
    ) -> LeafResult<String> {
        let mut formatter = Formatter {
            values,
            layouts: HashMap::new(),
            active: HashSet::new(),
            work: vec![Frame::Value(Rc::new(self.clone()), index, bits, 0)],
            text: String::new(),
        };
        while let Some(frame) = formatter.work.pop() {
            match frame {
                Frame::Text(text) => formatter.append(&text)?,
                Frame::Leave(layout, index, bits) => {
                    formatter.active.remove(&visit(&layout, index, bits));
                }
                Frame::Value(layout, index, bits, pending) => {
                    formatter.value(layout, index, bits, pending)?;
                }
            }
        }
        Ok(formatter.text)
    }
}

fn visit(layout: &Layout, index: usize, bits: u64) -> Visit {
    (Rc::as_ptr(layout) as usize, index, bits)
}

fn resolved_index(layout: &NativeDebugLayout, mut index: usize) -> LeafResult<usize> {
    // Alias cycles are malformed schemas, whereas recursive value nodes
    // are valid when their runtime payload has a finite base case.
    for _ in 0..layout.nodes.len() {
        match layout.nodes.get(index).ok_or(INVALID_TRACE_LABEL)? {
            NativeDebugNode::Alias(base) => index = *base,
            _ => return Ok(index),
        }
    }
    Err(INVALID_TRACE_LABEL)
}

impl Formatter<'_> {
    fn append(&mut self, text: &str) -> LeafResult<()> {
        self.text.try_reserve(text.len()).map_err(|_| EXHAUSTED)?;
        self.text.push_str(text);
        Ok(())
    }

    fn pending(&mut self, depth: u64) -> LeafResult<()> {
        if depth == 0 {
            return Ok(());
        }
        let count = usize::try_from(depth).map_err(|_| EXHAUSTED)?;
        let size = count.checked_mul(8).ok_or(EXHAUSTED)?;
        self.text.try_reserve(size).map_err(|_| EXHAUSTED)?;
        let mut suffix = String::new();
        suffix.try_reserve(count).map_err(|_| EXHAUSTED)?;
        for _ in 0..count {
            self.text.push_str("pending(");
            suffix.push(')');
        }
        self.work.push(Frame::Text(suffix));
        Ok(())
    }

    fn layout(&mut self, bytes: &[u8]) -> LeafResult<Layout> {
        if let Some(layout) = self.layouts.get(bytes) {
            return Ok(Rc::clone(layout));
        }
        let layout = Rc::new(NativeDebugLayout::parse(bytes)?);
        self.layouts.insert(bytes.to_vec(), Rc::clone(&layout));
        Ok(layout)
    }

    fn field(&mut self, layout: &Layout, index: usize, field: NativeField) {
        self.work.push(Frame::Value(
            Rc::clone(layout),
            index,
            field.bits,
            field.pending_depth,
        ));
    }

    fn value(&mut self, layout: Layout, index: usize, bits: u64, pending: u64) -> LeafResult<()> {
        let index = resolved_index(&layout, index)?;
        let node = layout.nodes.get(index).ok_or(INVALID_TRACE_LABEL)?;
        if matches!(
            node,
            NativeDebugNode::Alias(_) | NativeDebugNode::Uninhabited
        ) {
            return Err(INVALID_TRACE_LABEL);
        }
        if matches!(node, NativeDebugNode::Redacted) {
            return self.append("[redacted]");
        }
        if !self.active.insert(visit(&layout, index, bits)) {
            return Err(INVALID_TRACE_LABEL);
        }
        // Retain the layout until leaving the path so its identity stays live.
        self.work
            .push(Frame::Leave(Rc::clone(&layout), index, bits));
        self.pending(pending)?;
        let values = self.values;
        match node {
            NativeDebugNode::Interface => {
                let field = values.struct_field(bits, 1)?;
                let schema = values.struct_field(bits, 2)?.bits;
                let inner = self.layout(values.bytes(schema)?)?;
                let root = resolved_index(&inner, inner.root)?;
                if matches!(inner.nodes[root], NativeDebugNode::Redacted) {
                    return self.append("[redacted]");
                }
                let depth = values
                    .structs
                    .get(&bits)
                    .ok_or(INVALID_STRUCT)?
                    .pending_depth;
                let depth = depth.checked_add(field.pending_depth).ok_or(EXHAUSTED)?;
                self.work.push(Frame::Value(
                    Rc::clone(&inner),
                    inner.root,
                    field.bits,
                    depth,
                ));
            }
            NativeDebugNode::Primitive(kind) => self.append(&values.debug_value(bits, *kind)?)?,
            NativeDebugNode::Nothing => self.append(&format_nothing(bits)?)?,
            NativeDebugNode::Bytes => self.append(&values.debug_value(bits, DEBUG_BYTES_KIND)?)?,
            NativeDebugNode::Capability(name) => {
                self.append(&values.debug_capability(bits, name)?)?
            }
            NativeDebugNode::Function => {
                self.pending(
                    values
                        .structs
                        .get(&bits)
                        .ok_or(INVALID_STRUCT)?
                        .pending_depth,
                )?;
                self.append(values.function_debug_label(bits)?)?;
            }
            NativeDebugNode::Actor => {
                let ordinal = values.actors.get(&bits).ok_or(INVALID_ACTOR)?;
                self.pending(
                    values
                        .structs
                        .get(&bits)
                        .ok_or(INVALID_ACTOR)?
                        .pending_depth,
                )?;
                self.append(&format!("actor#{ordinal}"))?;
            }
            NativeDebugNode::TypeConstruction => {
                let builder = values.builders.get(&bits).ok_or(INVALID_CONSTRUCTION)?;
                let record = values.structs.get(&bits).ok_or(INVALID_CONSTRUCTION)?;
                self.pending(record.pending_depth)?;
                self.append(&format!("TypeConstruction[{}", builder.owner))?;
                if let Some(variant) = &builder.variant {
                    self.append(&format!(".{variant}"))?;
                }
                if let Some(state) = &builder.state {
                    self.append(&format!("@{state}"))?;
                }
                self.append("](")?;
                self.work.push(Frame::Text(")".into()));
                for (position, &field_index) in builder.put_order.iter().enumerate().rev() {
                    let field = record
                        .fields
                        .get(field_index + builder.field_offset)
                        .and_then(Option::as_ref)
                        .ok_or(INVALID_CONSTRUCTION)?;
                    let name = builder
                        .field_names
                        .get(field_index)
                        .ok_or(INVALID_CONSTRUCTION)?;
                    let bytes = builder
                        .field_debug_layouts
                        .get(field_index)
                        .ok_or(INVALID_CONSTRUCTION)?;
                    let inner = self.layout(bytes)?;
                    self.field(&inner, inner.root, *field);
                    let separator = if position > 0 { ", " } else { "" };
                    self.work.push(Frame::Text(format!("{separator}{name}: ")));
                }
            }
            NativeDebugNode::List(element) | NativeDebugNode::Set(element) => {
                let (elements, depths, depth, name, failure) = match node {
                    NativeDebugNode::List(_) => {
                        let list = values.lists.get(&bits).ok_or(INVALID_LIST)?;
                        (
                            &list.elements,
                            &list.element_pending_depths,
                            list.pending_depth,
                            "list",
                            INVALID_LIST,
                        )
                    }
                    _ => {
                        let set = values.sets.get(&bits).ok_or(INVALID_SET)?;
                        (
                            &set.elements,
                            &set.element_pending_depths,
                            set.pending_depth,
                            "set",
                            INVALID_SET,
                        )
                    }
                };
                self.pending(depth)?;
                self.append(&format!("{name}("))?;
                self.work.push(Frame::Text(")".into()));
                for (position, item) in elements.iter().enumerate().rev() {
                    self.work.push(Frame::Value(
                        Rc::clone(&layout),
                        *element,
                        item.ok_or(failure)?,
                        depths.get(&position).copied().unwrap_or(0),
                    ));
                    if position > 0 {
                        self.work.push(Frame::Text(", ".into()));
                    }
                }
            }
            NativeDebugNode::Map(key, value) => {
                let map = values.maps.get(&bits).ok_or(INVALID_MAP)?;
                self.pending(map.pending_depth)?;
                self.append("map(")?;
                self.work.push(Frame::Text(")".into()));
                for (position, entry) in map.entries.iter().enumerate().rev() {
                    let entry = entry.ok_or(INVALID_MAP)?;
                    if entry.key_taken {
                        return Err(INVALID_MAP);
                    }
                    self.work.push(Frame::Value(
                        Rc::clone(&layout),
                        *value,
                        entry.value,
                        entry.value_pending_depth,
                    ));
                    self.work.push(Frame::Text(": ".into()));
                    self.work.push(Frame::Value(
                        Rc::clone(&layout),
                        *key,
                        entry.key,
                        entry.key_pending_depth,
                    ));
                    if position > 0 {
                        self.work.push(Frame::Text(", ".into()));
                    }
                }
            }
            NativeDebugNode::Optional(element) | NativeDebugNode::Result(element, _) => {
                let sum = values.sums.get(&bits).ok_or(INVALID_SUM)?;
                self.pending(sum.pending_depth)?;
                let (prefix, child) = match (node, sum.tag) {
                    (NativeDebugNode::Optional(_), SUM_FAILURE) => {
                        self.append("none")?;
                        return Ok(());
                    }
                    (NativeDebugNode::Optional(_), SUM_SUCCESS) => ("some(", *element),
                    (NativeDebugNode::Result(_, error), SUM_FAILURE) => ("fail(", *error),
                    (NativeDebugNode::Result(_, _), SUM_SUCCESS) => ("ok(", *element),
                    _ => return Err(INVALID_SUM),
                };
                self.append(prefix)?;
                self.work.push(Frame::Text(")".into()));
                self.work.push(Frame::Value(
                    Rc::clone(&layout),
                    child,
                    sum.bits,
                    sum.payload_pending_depth,
                ));
            }
            NativeDebugNode::Record(name, fields) => {
                let record = values.structs.get(&bits).ok_or(INVALID_STRUCT)?;
                if record.fields.len() != fields.len() {
                    return Err(INVALID_STRUCT);
                }
                self.pending(record.pending_depth)?;
                self.append(&format!("{name}("))?;
                self.work.push(Frame::Text(")".into()));
                for (position, (name, child)) in fields.iter().enumerate().rev() {
                    self.field(
                        &layout,
                        *child,
                        record.fields[position].ok_or(INVALID_STRUCT)?,
                    );
                    let separator = if position > 0 { ", " } else { "" };
                    self.work.push(Frame::Text(format!("{separator}{name}: ")));
                }
            }
            NativeDebugNode::Enum(name, variants) | NativeDebugNode::Machine(name, variants) => {
                let record = values.structs.get(&bits).ok_or(INVALID_STRUCT)?;
                let tag = usize::try_from(
                    record
                        .fields
                        .first()
                        .copied()
                        .flatten()
                        .ok_or(INVALID_STRUCT)?
                        .bits,
                )
                .map_err(|_| INVALID_STRUCT)?;
                let (variant, fields) = variants.get(tag).ok_or(INVALID_STRUCT)?;
                if record.fields.len() != fields.len() + 1 {
                    return Err(INVALID_STRUCT);
                }
                self.pending(record.pending_depth)?;
                let separator = if matches!(node, NativeDebugNode::Enum(..)) {
                    "."
                } else {
                    "@"
                };
                self.append(&format!("{name}{separator}{variant}"))?;
                if !fields.is_empty() {
                    self.append("(")?;
                    self.work.push(Frame::Text(")".into()));
                    for (position, child) in fields.iter().enumerate().rev() {
                        self.field(
                            &layout,
                            *child,
                            record.fields[position + 1].ok_or(INVALID_STRUCT)?,
                        );
                        if position > 0 {
                            self.work.push(Frame::Text(", ".into()));
                        }
                    }
                }
            }
            NativeDebugNode::Alias(_)
            | NativeDebugNode::Uninhabited
            | NativeDebugNode::Redacted => unreachable!(),
        }
        Ok(())
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

    #[test]
    fn deep_finite_values_use_a_work_stack_and_shared_children_remain_valid() {
        let layout = NativeDebugLayout {
            root: 0,
            nodes: vec![NativeDebugNode::Enum(
                "Chain".into(),
                vec![
                    ("end".into(), vec![]),
                    ("next".into(), vec![0]),
                    ("pair".into(), vec![0, 0]),
                ],
            )],
        };
        let mut values = NativeValues::default();
        let mut handles = Vec::new();
        let mut current = values.new_struct(1).unwrap();
        values.structs.get_mut(&current).unwrap().fields[0] = field(0);
        handles.push(current);
        for _ in 0..4096 {
            let next = values.new_struct(2).unwrap();
            values.structs.get_mut(&next).unwrap().fields = vec![field(1), field(current)];
            handles.push(next);
            current = next;
        }
        let expected = format!(
            "{}Chain.end{}",
            "Chain.next(".repeat(4096),
            ")".repeat(4096)
        );
        assert_eq!(
            layout.format_value(&values, current, 0),
            Ok(expected.clone())
        );
        let pair = values.new_struct(3).unwrap();
        values.structs.get_mut(&pair).unwrap().fields =
            vec![field(2), field(current), field(current)];
        assert_eq!(
            layout.format_value(&values, pair, 0),
            Ok(format!("Chain.pair({expected}, {expected})"))
        );
        handles.push(pair);
        for handle in handles {
            values.drop_value(handle).unwrap();
        }
        assert!(values.is_empty());
    }

    #[test]
    fn alias_and_payload_cycles_fail_without_recursing() {
        let mut values = NativeValues::default();
        let alias = NativeDebugLayout {
            root: 0,
            nodes: vec![NativeDebugNode::Alias(0)],
        };
        assert_eq!(alias.format_value(&values, 0, 0), Err(INVALID_TRACE_LABEL));
        let record = values.new_struct(1).unwrap();
        values.structs.get_mut(&record).unwrap().fields[0] = field(record);
        let recursive = NativeDebugLayout {
            root: 0,
            nodes: vec![NativeDebugNode::Record(
                "Loop".into(),
                vec![("value".into(), 0)],
            )],
        };
        assert_eq!(
            recursive.format_value(&values, record, 0),
            Err(INVALID_TRACE_LABEL)
        );
        values.drop_value(record).unwrap();
        assert!(values.is_empty());
    }

    fn one_node(tag: NativeDebugTag) -> Vec<u8> {
        let mut bytes = b"JD\x01".to_vec();
        for word in [1u32, 0, 1] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.push(tag as u8);
        bytes
    }

    #[test]
    fn dynamic_layout_cycles_fail_atomically_and_redaction_skips_invalid_payloads() {
        let mut values = NativeValues::default();
        let bytes = one_node(NativeDebugTag::Interface);
        let schema = values.insert_bytes(bytes.clone()).unwrap();
        let interface = values.new_struct(3).unwrap();
        values.structs.get_mut(&interface).unwrap().fields =
            vec![field(0), field(interface), field(schema)];
        let text = values.insert("before".into()).unwrap();
        assert_eq!(
            values.debug_append_aggregate(text, "", interface, &bytes),
            Err(INVALID_TRACE_LABEL)
        );
        assert_eq!(values.text(text), Ok("before"));
        let redacted = values
            .insert_bytes(one_node(NativeDebugTag::Redacted))
            .unwrap();
        let record = values.structs.get_mut(&interface).unwrap();
        record.fields[1] = Some(NativeField {
            bits: u64::MAX,
            pending_depth: u64::MAX,
            owned: false,
        });
        record.fields[2] = field(redacted);
        record.pending_depth = 2;
        assert_eq!(
            values.debug_append_aggregate(text, "", interface, &bytes),
            Ok(0)
        );
        assert_eq!(values.text(text), Ok("before[redacted]"));
        for handle in [interface, schema, redacted, text] {
            values.drop_value(handle).unwrap();
        }
        assert!(values.is_empty());
    }
}
