//! `array_column()` return type provider.

use std::borrow::Cow;
use std::sync::Arc;

use mago_codex::metadata::class_like::ClassLikeMetadata;
use mago_codex::ttype::atomic::TAtomic;
use mago_codex::ttype::atomic::array::TArray;
use mago_codex::ttype::atomic::array::key::ArrayKey;
use mago_codex::ttype::atomic::array::keyed::TKeyedArray;
use mago_codex::ttype::atomic::array::list::TList;
use mago_codex::ttype::atomic::object::TObject;
use mago_codex::ttype::atomic::scalar::TScalar;
use mago_codex::ttype::get_array_parameters;
use mago_codex::ttype::get_arraykey;
use mago_codex::ttype::union::TUnion;
use mago_word::concat_word;
use mago_word::word;

use crate::plugin::context::InvocationInfo;
use crate::plugin::context::ProviderContext;
use crate::plugin::provider::Provider;
use crate::plugin::provider::ProviderMeta;
use crate::plugin::provider::function::FunctionReturnTypeProvider;
use crate::plugin::provider::function::FunctionTarget;

static META: ProviderMeta = ProviderMeta::new(
    "php::array::array_column",
    "array_column",
    "Returns list or array based on column_key and index_key arguments",
);

/// Provider for the `array_column()` function.
///
/// Returns typed arrays based on the `column_key` and `index_key` arguments.
#[derive(Default)]
pub struct ArrayColumnProvider;

impl Provider for ArrayColumnProvider {
    fn meta() -> &'static ProviderMeta {
        &META
    }
}

impl FunctionReturnTypeProvider for ArrayColumnProvider {
    fn targets() -> FunctionTarget {
        FunctionTarget::Exact(b"array_column")
    }

    fn get_return_type(
        &self,
        context: &ProviderContext<'_, '_, '_>,
        invocation: &InvocationInfo<'_, '_, '_>,
    ) -> Option<TUnion> {
        let array_argument = invocation.get_argument(0, &[b"array"])?;
        let array_type = context.get_expression_type(array_argument)?;

        let array = array_type.get_single_array()?;
        let codebase = context.codebase();
        let element_type = get_array_parameters(array, codebase).1;

        let column_key_argument = invocation.get_argument(1, &[b"column_key"])?;
        let column_key_type = context.get_expression_type(column_key_argument)?;

        let index_key_argument = invocation.get_argument(2, &[b"index_key"]);
        let index_key_type = index_key_argument.and_then(|arg| context.get_expression_type(arg));
        let input_non_empty = array.is_non_empty();

        if let Some(result) =
            try_resolve_from_named_object(&element_type, column_key_type, index_key_type, input_non_empty, codebase)
        {
            return Some(result);
        }

        if let Some(result) =
            try_resolve_from_object_shape(&element_type, column_key_type, index_key_type, input_non_empty)
        {
            return Some(result);
        }

        if let Some(result) =
            try_resolve_from_keyed_array(&element_type, column_key_type, index_key_type, input_non_empty)
        {
            return Some(result);
        }

        if column_key_type.is_null()
            && element_type.types.iter().all(|atomic| matches!(atomic, TAtomic::Array(_) | TAtomic::Object(_)))
        {
            return Some(build_result(element_type, None, has_index_key(index_key_type), input_non_empty));
        }

        None
    }
}

/// Resolve column and index types from an object element type by looking up
/// class properties.
fn try_resolve_from_named_object(
    element_type: &TUnion,
    column_key_type: &TUnion,
    index_key_type: Option<&TUnion>,
    input_non_empty: bool,
    codebase: &mago_codex::metadata::CodebaseMetadata,
) -> Option<TUnion> {
    let obj = element_type.get_single_named_object()?;
    let class_like = codebase.get_class_like(obj.name.as_bytes())?;

    let column_type = if column_key_type.is_null() {
        TUnion::from_atomic(TAtomic::Object(TObject::Named(obj.clone())))
    } else {
        let prop_name = column_key_type.get_single_literal_string_value()?;
        let prop = class_like.properties.get(&concat_word!(b"$", prop_name))?;
        prop.type_metadata.as_ref()?.type_union.clone()
    };

    let index_type = resolve_index_type_from_property(index_key_type, class_like);

    Some(build_result(column_type, index_type, has_index_key(index_key_type), input_non_empty))
}

fn try_resolve_from_object_shape(
    element_type: &TUnion,
    column_key_type: &TUnion,
    index_key_type: Option<&TUnion>,
    input_non_empty: bool,
) -> Option<TUnion> {
    if !element_type.is_single() {
        return None;
    }

    let TAtomic::Object(TObject::WithProperties(object)) = element_type.get_single() else {
        return None;
    };

    let column_type = if column_key_type.is_null() {
        element_type.clone()
    } else {
        let key = word(column_key_type.get_single_literal_string_value()?);
        let (optional, value_type) = object.known_properties.get(&key)?;
        if *optional {
            return None;
        }
        value_type.clone()
    };

    let index_type = index_key_type.and_then(|index_key_type| {
        let key = word(index_key_type.get_single_literal_string_value()?);
        let (_, value_type) = object.known_properties.get(&key)?;
        extract_scalar_for_key(value_type)
    });

    Some(build_result(column_type, index_type, has_index_key(index_key_type), input_non_empty))
}

/// Resolve column and index types from a keyed-array element type by looking
/// up known items.
fn try_resolve_from_keyed_array(
    element_type: &TUnion,
    column_key_type: &TUnion,
    index_key_type: Option<&TUnion>,
    input_non_empty: bool,
) -> Option<TUnion> {
    if !element_type.is_single() {
        return None;
    }

    let TAtomic::Array(TArray::Keyed(keyed)) = element_type.get_single() else {
        return None;
    };

    let known_items = keyed.get_known_items()?;

    let column_type = if column_key_type.is_null() {
        element_type.clone()
    } else {
        let key_str = column_key_type.get_single_literal_string_value()?;
        let (_, value_type) = known_items.get(&ArrayKey::String(word(key_str)))?;
        value_type.clone()
    };

    let index_type = if let Some(index_key_type) = index_key_type {
        if index_key_type.is_null() {
            None
        } else {
            let key_str = index_key_type.get_single_literal_string_value()?;
            let (_, value_type) = known_items.get(&ArrayKey::String(word(key_str)))?;
            extract_scalar_for_key(value_type)
        }
    } else {
        None
    };

    Some(build_result(column_type, index_type, has_index_key(index_key_type), input_non_empty))
}

/// Try to extract the key scalar type from a value type (for use as array index).
fn extract_scalar_for_key(value_type: &TUnion) -> Option<&TScalar> {
    if !value_type.is_single() {
        return None;
    }

    match value_type.get_single() {
        TAtomic::Scalar(
            scalar @ (TScalar::ArrayKey | TScalar::Integer(_) | TScalar::String(_) | TScalar::ClassLikeString(_)),
        ) => Some(scalar),
        _ => None,
    }
}

fn resolve_index_type_from_property<'meta>(
    index_key_type: Option<&TUnion>,
    class_like: &'meta ClassLikeMetadata,
) -> Option<&'meta TScalar> {
    let index_key_type = index_key_type?;
    if index_key_type.is_null() {
        return None;
    }

    let prop_name = index_key_type.get_single_literal_string_value()?;
    let prop = class_like.properties.get(&concat_word!("$", prop_name))?;
    let prop_type = &prop.type_metadata.as_ref()?.type_union;

    extract_scalar_for_key(prop_type)
}

fn has_index_key(index_key_type: Option<&TUnion>) -> bool {
    index_key_type.is_some_and(|index_key_type| !index_key_type.is_null())
}

fn build_result(column_type: TUnion, index_type: Option<&TScalar>, has_index_key: bool, non_empty: bool) -> TUnion {
    if has_index_key {
        let keyed_array = TKeyedArray::new_with_parameters(
            Arc::new(
                index_type
                    .map(|scalar| TUnion::from_atomic(TAtomic::Scalar(scalar.clone())))
                    .unwrap_or_else(get_arraykey),
            ),
            Arc::new(column_type),
        );
        let mut keyed_array = keyed_array;
        keyed_array.non_empty = non_empty;

        TUnion::from_atomic(TAtomic::Array(TArray::Keyed(keyed_array)))
    } else {
        let mut list = TList::new(Arc::new(column_type));
        list.non_empty = non_empty;

        TUnion::from_single(Cow::Owned(TAtomic::Array(TArray::List(list))))
    }
}
