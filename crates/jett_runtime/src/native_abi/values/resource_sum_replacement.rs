//! Validate the two ordinary failure obligations before a Resource sum retires.
use super::*;
use crate::resource_custody::{NativeShape, RegisteredNativeLayout};

#[derive(Default)]
struct CompanionClaims {
    sums: Vec<u64>,
    string: Option<u64>,
}

impl NativeValues {
    pub(in crate::native_abi) fn preflight_resource_sum_companions(
        &self,
        layout: &RegisteredNativeLayout,
        old: Option<(u32, u64)>,
        replacement: Option<(u32, u64)>,
    ) -> LeafResult<()> {
        let old = self.resource_sum_companion_claims(layout, old)?;
        let replacement = self.resource_sum_companion_claims(layout, replacement)?;
        // Ordinary sum cloning allocates distinct shells. Equal shells cannot
        // represent two owning failure obligations, including nested children.
        if old.sums.iter().any(|id| replacement.sums.contains(id)) {
            return Err(INVALID_HANDLE);
        }
        if let (Some(old), Some(replacement)) = (old.string, replacement.string) {
            if old == replacement && self.strings.get(&old).ok_or(INVALID_HANDLE)?.references < 2 {
                return Err(INVALID_HANDLE);
            }
        }
        Ok(())
    }

    fn resource_sum_companion_claims(
        &self,
        layout: &RegisteredNativeLayout,
        companion: Option<(u32, u64)>,
    ) -> LeafResult<CompanionClaims> {
        let Some((mut shape, mut value)) = companion else {
            return Ok(CompanionClaims::default());
        };
        self.validate_resource_ordinary(value, layout, shape)?;
        let mut claims = CompanionClaims::default();
        loop {
            match layout.shapes().get(shape as usize).ok_or(INVALID_HANDLE)? {
                NativeShape::String => {
                    claims.string = Some(value);
                    return Ok(claims);
                }
                NativeShape::Optional { child } => {
                    self.resource_sum_claim(&mut claims, value)?;
                    let sum = self.sums.get(&value).ok_or(INVALID_HANDLE)?;
                    if sum.tag == SUM_FAILURE {
                        return Ok(claims);
                    }
                    shape = *child;
                    value = sum.bits;
                }
                NativeShape::Result { ok, fail } => {
                    self.resource_sum_claim(&mut claims, value)?;
                    let sum = self.sums.get(&value).ok_or(INVALID_HANDLE)?;
                    shape = if sum.tag == SUM_SUCCESS { *ok } else { *fail };
                    value = sum.bits;
                }
                NativeShape::Integer { .. }
                | NativeShape::Float { .. }
                | NativeShape::Bool
                | NativeShape::Nothing
                | NativeShape::Network => return Ok(claims),
                _ => return Err(INVALID_HANDLE),
            }
        }
    }

    fn resource_sum_claim(&self, claims: &mut CompanionClaims, value: u64) -> LeafResult<()> {
        // A malformed cyclic/repeated selected shell must fail before drop_value
        // can remove an outer shell and discover its child has gone stale.
        if claims.sums.contains(&value) {
            return Err(INVALID_HANDLE);
        }
        claims.sums.try_reserve(1).map_err(|_| EXHAUSTED)?;
        claims.sums.push(value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceRegistry;
    use crate::resource_custody::NativeLayoutInstallation;

    fn layout() -> Arc<RegisteredNativeLayout> {
        fn words(bytes: &mut Vec<u8>, values: &[u32]) {
            for value in values {
                bytes.extend(value.to_le_bytes());
            }
        }
        // Independent ordinary shapes, admitted by the real layout issuer.
        let mut bytes = b"JTRSC001".to_vec();
        words(&mut bytes, &[1, 0]);
        bytes.extend(0u64.to_le_bytes());
        words(&mut bytes, &[0, 0, 1, 6, 1, 0, 0, 0]);
        for row in [
            vec![0, 0, 2],
            vec![0, 1, 64, 1],
            vec![1, 4],
            vec![2, 5],
            vec![3, 8, 1],
            vec![4, 8, 3],
            vec![5, 9, 0, 1],
            vec![0, 1, 0, 0, 0, 0, 0, 1, 0, 0],
        ] {
            words(&mut bytes, &row);
        }
        let length = u64::try_from(bytes.len()).unwrap();
        bytes[16..24].copy_from_slice(&length.to_le_bytes());
        let registry = ResourceRegistry::new();
        NativeLayoutInstallation::new(&registry)
            .install(&registry, &bytes)
            .unwrap()
    }

    #[test]
    fn companion_replacement_requires_real_string_clone_and_preserves_exact_references() {
        let layout = layout();
        let mut values = NativeValues::default();
        let text = values.insert("shared".into()).unwrap();
        let pair = Some((1, text));
        assert!(
            values
                .preflight_resource_sum_companions(&layout, pair, pair)
                .is_err()
        );
        assert_eq!(values.strings[&text].references, 1);
        assert_eq!(values.clone_value(text).unwrap(), text);
        values
            .preflight_resource_sum_companions(&layout, pair, pair)
            .unwrap();
        assert_eq!(values.strings[&text].references, 2);
        values
            .drop_resource_typed_companion(&layout, 1, text)
            .unwrap();
        assert_eq!(values.strings[&text].references, 1);
        values.validate_resource_ordinary(text, &layout, 1).unwrap();
        values
            .drop_resource_typed_companion(&layout, 1, text)
            .unwrap();
        assert!(values.strings.is_empty());
    }

    #[test]
    fn companion_replacement_nested_clones_have_distinct_shells_and_one_shared_string() {
        let layout = layout();
        let mut values = NativeValues::default();
        let text = values.insert("nested".into()).unwrap();
        let inner = values.sum(SUM_SUCCESS, text, true).unwrap();
        let old = values.sum(SUM_SUCCESS, inner, true).unwrap();
        let replacement = values.clone_value(old).unwrap();
        assert_ne!(old, replacement);
        assert_ne!(values.sums[&old].bits, values.sums[&replacement].bits);
        assert_eq!(values.strings[&text].references, 2);
        values
            .preflight_resource_sum_companions(&layout, Some((4, old)), Some((4, replacement)))
            .unwrap();
        values
            .drop_resource_typed_companion(&layout, 4, old)
            .unwrap();
        assert!(!values.sums.contains_key(&old));
        assert!(!values.sums.contains_key(&inner));
        assert_eq!(values.strings[&text].references, 1);
        values
            .validate_resource_ordinary(replacement, &layout, 4)
            .unwrap();
        values
            .drop_resource_typed_companion(&layout, 4, replacement)
            .unwrap();
        assert!(values.sums.is_empty());
        assert!(values.strings.is_empty());
        assert_eq!(values.sums_created, values.sums_destroyed);
    }

    #[test]
    fn companion_replacement_rejects_shared_nested_shell_before_any_drop() {
        let layout = layout();
        let mut values = NativeValues::default();
        let text = values.insert("one child".into()).unwrap();
        let inner = values.sum(SUM_SUCCESS, text, true).unwrap();
        let old = values.sum(SUM_SUCCESS, inner, true).unwrap();
        let invalid = values.sum(SUM_SUCCESS, inner, true).unwrap();
        assert!(
            values
                .preflight_resource_sum_companions(&layout, Some((4, old)), Some((4, invalid)))
                .is_err()
        );
        assert_eq!(values.sums.len(), 3);
        assert_eq!(values.strings[&text].references, 1);
        // Undo only the deliberately malformed second ownership claim.
        let duplicate = values.sums.remove(&invalid).unwrap();
        assert_eq!(duplicate.bits, inner);
        values.sums_destroyed += 1;
        values
            .drop_resource_typed_companion(&layout, 4, old)
            .unwrap();
        assert!(values.sums.is_empty());
        assert!(values.strings.is_empty());
    }

    #[test]
    fn companion_replacement_rejects_repeated_shell_and_stale_child_before_any_drop() {
        let layout = layout();
        let mut values = NativeValues::default();
        let cyclic = values.sum(SUM_SUCCESS, 0, true).unwrap();
        values.sums.get_mut(&cyclic).unwrap().bits = cyclic;
        assert!(
            values
                .preflight_resource_sum_companions(&layout, Some((4, cyclic)), None)
                .is_err()
        );
        assert_eq!(values.sums.len(), 1);
        let stale = values.sum(SUM_SUCCESS, u64::MAX, true).unwrap();
        assert!(
            values
                .preflight_resource_sum_companions(&layout, Some((3, stale)), None)
                .is_err()
        );
        assert_eq!(values.sums.len(), 2);
        // These malformed shells own no actual ordinary payload.
        for shell in [cyclic, stale] {
            values.sums.remove(&shell).unwrap();
            values.sums_destroyed += 1;
        }
        assert!(values.sums.is_empty());
        assert!(values.strings.is_empty());
    }

    #[test]
    fn companion_replacement_primitive_and_none_branches_need_no_ordinary_storage() {
        let layout = layout();
        let mut values = NativeValues::default();
        values
            .preflight_resource_sum_companions(&layout, Some((0, 17)), Some((0, 29)))
            .unwrap();
        let old = values.sum(SUM_FAILURE, 0, false).unwrap();
        let replacement = values.sum(SUM_FAILURE, 0, false).unwrap();
        values
            .preflight_resource_sum_companions(&layout, Some((4, old)), Some((4, replacement)))
            .unwrap();
        values
            .drop_resource_typed_companion(&layout, 4, old)
            .unwrap();
        values
            .drop_resource_typed_companion(&layout, 4, replacement)
            .unwrap();
        assert!(values.sums.is_empty());
        assert!(values.strings.is_empty());
    }
}
