//! `array_shift()` and `array_pop()` return type provider.

use mago_codex::ttype::add_union_type;
use mago_codex::ttype::atomic::TAtomic;
use mago_codex::ttype::atomic::array::TArray;
use mago_codex::ttype::combiner::CombinerOptions;
use mago_codex::ttype::union::TUnion;

use crate::plugin::context::InvocationInfo;
use crate::plugin::context::ProviderContext;
use crate::plugin::provider::Provider;
use crate::plugin::provider::ProviderMeta;
use crate::plugin::provider::function::FunctionReturnTypeProvider;
use crate::plugin::provider::function::FunctionTarget;

static META: ProviderMeta =
    ProviderMeta::new("php::array::shift-pop", "array_shift/array_pop", "Returns the removed array element type");

static TARGETS: [&[u8]; 2] = [b"array_shift", b"array_pop"];

#[derive(Default)]
pub struct ArrayShiftPopProvider;

impl Provider for ArrayShiftPopProvider {
    fn meta() -> &'static ProviderMeta {
        &META
    }
}

impl FunctionReturnTypeProvider for ArrayShiftPopProvider {
    fn targets() -> FunctionTarget {
        FunctionTarget::ExactMultiple(&TARGETS)
    }

    fn get_return_type(
        &self,
        context: &ProviderContext<'_, '_, '_>,
        invocation: &InvocationInfo<'_, '_, '_>,
    ) -> Option<TUnion> {
        let array_argument = invocation.get_argument(0, &[b"array"])?;
        let array_type = context.get_expression_type(array_argument)?;
        let is_shift = invocation.function_name().eq_ignore_ascii_case("array_shift");
        let mut result = None;

        for atomic in array_type.types.as_ref() {
            let TAtomic::Array(array) = atomic else {
                return None;
            };
            let (mut removed, may_be_missing) = removed_element(array, is_shift)?;
            if may_be_missing && !removed.is_mixed() {
                removed.types.to_mut().push(TAtomic::Null);
                removed.types.to_mut().sort();
                removed.types.to_mut().dedup();
            }

            result = Some(match result {
                Some(existing) => add_union_type(existing, &removed, context.codebase(), CombinerOptions::default()),
                None => removed,
            });
        }

        result
    }
}

fn removed_element(array: &TArray, is_shift: bool) -> Option<(TUnion, bool)> {
    match array {
        TArray::List(list) => {
            if let Some(elements) = &list.known_elements
                && !elements.is_empty()
            {
                let entry = if is_shift { elements.first_key_value() } else { elements.last_key_value() }?;
                return Some((entry.1.1.clone(), entry.1.0 || !list.non_empty));
            }

            Some(((*list.element_type).clone(), !list.non_empty))
        }
        TArray::Keyed(keyed) => {
            if let Some(items) = &keyed.known_items
                && !items.is_empty()
            {
                let entry = if is_shift { items.first_key_value() } else { items.last_key_value() }?;
                return Some((entry.1.1.clone(), entry.1.0 || !keyed.non_empty));
            }

            let value_type = keyed.parameters.as_ref().map(|(_, value)| (**value).clone())?;
            Some((value_type, !keyed.non_empty))
        }
    }
}
