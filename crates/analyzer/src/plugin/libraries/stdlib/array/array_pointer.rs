//! Array pointer and key return type providers.

use mago_codex::ttype::atomic::TAtomic;
use mago_codex::ttype::atomic::array::TArray;
use mago_codex::ttype::atomic::scalar::TScalar;
use mago_codex::ttype::get_array_parameters;
use mago_codex::ttype::union::TUnion;

use crate::plugin::context::InvocationInfo;
use crate::plugin::context::ProviderContext;
use crate::plugin::provider::Provider;
use crate::plugin::provider::ProviderMeta;
use crate::plugin::provider::function::FunctionReturnTypeProvider;
use crate::plugin::provider::function::FunctionTarget;

static POINTER_META: ProviderMeta = ProviderMeta::new(
    "php::array::pointer",
    "current/reset/end",
    "Returns the array value type while preserving pointer failure semantics",
);
static KEY_META: ProviderMeta =
    ProviderMeta::new("php::array::key", "key/array_key_first/array_key_last", "Returns the input array's key type");

static POINTER_TARGETS: [&[u8]; 3] = [b"current", b"reset", b"end"];
static KEY_TARGETS: [&[u8]; 3] = [b"key", b"array_key_first", b"array_key_last"];

#[derive(Default)]
pub struct ArrayPointerProvider;

impl Provider for ArrayPointerProvider {
    fn meta() -> &'static ProviderMeta {
        &POINTER_META
    }
}

impl FunctionReturnTypeProvider for ArrayPointerProvider {
    fn targets() -> FunctionTarget {
        FunctionTarget::ExactMultiple(&POINTER_TARGETS)
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
        let (_, value_type) = get_array_parameters(array, context.codebase());

        if value_type.is_never() {
            return Some(TUnion::from_atomic(TAtomic::Scalar(TScalar::r#false())));
        }

        let is_non_empty = match array {
            TArray::Keyed(keyed) => keyed.non_empty,
            TArray::List(list) => list.non_empty,
        };
        let can_exploit_non_empty = !invocation.function_name().eq_ignore_ascii_case("key")
            && !invocation.function_name().eq_ignore_ascii_case("current");

        if is_non_empty && can_exploit_non_empty {
            return Some(value_type);
        }

        let mut result = value_type;
        result.types.to_mut().push(TAtomic::Scalar(TScalar::r#false()));
        result.types.to_mut().sort();
        result.types.to_mut().dedup();
        result.set_ignore_falsable_issues(true);
        Some(result)
    }
}

#[derive(Default)]
pub struct ArrayKeyProvider;

impl Provider for ArrayKeyProvider {
    fn meta() -> &'static ProviderMeta {
        &KEY_META
    }
}

impl FunctionReturnTypeProvider for ArrayKeyProvider {
    fn targets() -> FunctionTarget {
        FunctionTarget::ExactMultiple(&KEY_TARGETS)
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

        if value_type.is_never() {
            return Some(TUnion::from_atomic(TAtomic::Null));
        }

        let is_non_empty = match array {
            TArray::Keyed(keyed) => keyed.non_empty,
            TArray::List(list) => list.non_empty,
        };
        let is_pointer_key = invocation.function_name().eq_ignore_ascii_case("key");

        if is_non_empty && !is_pointer_key {
            return Some(key_type);
        }

        let mut result = key_type;
        result.types.to_mut().push(TAtomic::Null);
        result.types.to_mut().sort();
        result.types.to_mut().dedup();
        if is_pointer_key {
            result.set_ignore_nullable_issues(true);
        }

        Some(result)
    }
}
