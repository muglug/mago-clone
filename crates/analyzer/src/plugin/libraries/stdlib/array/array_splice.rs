//! `array_splice()` return type provider.

use std::sync::Arc;

use mago_codex::ttype::atomic::TAtomic;
use mago_codex::ttype::atomic::array::TArray;
use mago_codex::ttype::atomic::array::keyed::TKeyedArray;
use mago_codex::ttype::atomic::array::list::TList;
use mago_codex::ttype::get_array_parameters;
use mago_codex::ttype::union::TUnion;

use crate::plugin::context::InvocationInfo;
use crate::plugin::context::ProviderContext;
use crate::plugin::provider::Provider;
use crate::plugin::provider::ProviderMeta;
use crate::plugin::provider::function::FunctionReturnTypeProvider;
use crate::plugin::provider::function::FunctionTarget;

static META: ProviderMeta =
    ProviderMeta::new("php::array::array_splice", "array_splice", "Returns the extracted array's key and value types");

#[derive(Default)]
pub struct ArraySpliceProvider;

impl Provider for ArraySpliceProvider {
    fn meta() -> &'static ProviderMeta {
        &META
    }
}

impl FunctionReturnTypeProvider for ArraySpliceProvider {
    fn targets() -> FunctionTarget {
        FunctionTarget::Exact(b"array_splice")
    }

    fn get_return_type(
        &self,
        context: &ProviderContext<'_, '_, '_>,
        invocation: &InvocationInfo<'_, '_, '_>,
    ) -> Option<TUnion> {
        let array_argument = invocation.get_argument(0, &[b"array"])?;
        let array_type = context.get_expression_type(array_argument)?;
        let TAtomic::Array(array) = array_type.get_single() else {
            return None;
        };
        let (key_type, value_type) = get_array_parameters(array, context.codebase());

        let non_empty = match array {
            TArray::Keyed(keyed) => {
                keyed.non_empty
                    || keyed.known_items.as_ref().is_some_and(|items| items.values().any(|(optional, _)| !optional))
            }
            TArray::List(list) => list.non_empty,
        };

        if key_type.has_string() {
            let mut result = TKeyedArray::new_with_parameters(Arc::new(key_type), Arc::new(value_type));
            result.non_empty = non_empty;
            return Some(TUnion::from_atomic(TAtomic::Array(TArray::Keyed(result))));
        }

        let mut result = TList::new(Arc::new(value_type));
        result.non_empty = non_empty;
        Some(TUnion::from_atomic(TAtomic::Array(TArray::List(result))))
    }
}
