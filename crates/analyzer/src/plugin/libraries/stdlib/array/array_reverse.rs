//! `array_reverse()` return type provider.

use std::sync::Arc;

use mago_codex::ttype::atomic::TAtomic;
use mago_codex::ttype::atomic::array::TArray;
use mago_codex::ttype::atomic::array::list::TList;
use mago_codex::ttype::get_array_parameters;
use mago_codex::ttype::union::TUnion;

use crate::plugin::context::InvocationInfo;
use crate::plugin::context::ProviderContext;
use crate::plugin::provider::Provider;
use crate::plugin::provider::ProviderMeta;
use crate::plugin::provider::function::FunctionReturnTypeProvider;
use crate::plugin::provider::function::FunctionTarget;

static META: ProviderMeta = ProviderMeta::new(
    "php::array::array_reverse",
    "array_reverse",
    "Preserves array non-emptiness and list element types",
);

#[derive(Default)]
pub struct ArrayReverseProvider;

impl Provider for ArrayReverseProvider {
    fn meta() -> &'static ProviderMeta {
        &META
    }
}

impl FunctionReturnTypeProvider for ArrayReverseProvider {
    fn targets() -> FunctionTarget {
        FunctionTarget::Exact(b"array_reverse")
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

        let preserve_keys = invocation
            .get_argument(1, &[b"preserve_keys"])
            .and_then(|argument| context.get_expression_type(argument))
            .is_some_and(|argument_type| !argument_type.is_false());

        match array {
            TArray::Keyed(keyed) if keyed.parameters.is_some() => Some(array_type.clone()),
            TArray::List(list) if list.known_elements.is_none() && !preserve_keys => Some(array_type.clone()),
            TArray::List(list) if !preserve_keys => {
                let (_, element_type) = get_array_parameters(array, context.codebase());
                let mut result = TList::new(Arc::new(element_type));
                result.non_empty = list.non_empty;
                Some(TUnion::from_atomic(TAtomic::Array(TArray::List(result))))
            }
            _ => None,
        }
    }
}
