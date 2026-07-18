//! Mode-sensitive `PDOStatement::fetch()` and `fetchAll()` return types.

use std::sync::Arc;

use mago_codex::ttype::atomic::TAtomic;
use mago_codex::ttype::atomic::array::TArray;
use mago_codex::ttype::atomic::array::keyed::TKeyedArray;
use mago_codex::ttype::atomic::array::list::TList;
use mago_codex::ttype::atomic::object::TObject;
use mago_codex::ttype::atomic::scalar::TScalar;
use mago_codex::ttype::atomic::scalar::class_like_string::TClassLikeString;
use mago_codex::ttype::atomic::scalar::string::TStringLiteral;
use mago_codex::ttype::get_arraykey;
use mago_codex::ttype::get_bool;
use mago_codex::ttype::get_string;
use mago_codex::ttype::union::TUnion;
use mago_word::Word;
use mago_word::word;

use crate::plugin::context::InvocationInfo;
use crate::plugin::context::ProviderContext;
use crate::plugin::provider::Provider;
use crate::plugin::provider::ProviderMeta;
use crate::plugin::provider::method::MethodReturnTypeProvider;
use crate::plugin::provider::method::MethodTarget;

static META: ProviderMeta = ProviderMeta::new(
    "php::pdo::statement-fetch",
    "PDOStatement::fetch/fetchAll",
    "Returns the row shape selected by the PDO fetch mode",
);

static TARGETS: [MethodTarget; 2] =
    [MethodTarget::exact(b"pdostatement", b"fetch"), MethodTarget::exact(b"pdostatement", b"fetchall")];

#[derive(Default)]
pub struct PdoStatementReturnTypeProvider;

impl Provider for PdoStatementReturnTypeProvider {
    fn meta() -> &'static ProviderMeta {
        &META
    }
}

impl MethodReturnTypeProvider for PdoStatementReturnTypeProvider {
    fn targets() -> &'static [MethodTarget] {
        &TARGETS
    }

    fn get_return_type(
        &self,
        context: &ProviderContext<'_, '_, '_>,
        _class_name: &[u8],
        method_name: &[u8],
        invocation: &InvocationInfo<'_, '_, '_>,
    ) -> Option<TUnion> {
        let mode_argument = invocation.get_argument(0, &[b"mode"])?;
        let mode = context.get_expression_type(mode_argument)?.get_single_literal_int_value()?;

        if method_name.eq_ignore_ascii_case(b"fetch") {
            return fetch_return_type(mode);
        }

        if method_name.eq_ignore_ascii_case(b"fetchall") {
            return fetch_all_return_type(context, invocation, mode);
        }

        None
    }
}

fn fetch_return_type(mode: i64) -> Option<TUnion> {
    Some(match mode {
        1 => with_false(TAtomic::Object(TObject::Any)),
        2 => with_false(keyed_array_atomic(get_string(), scalar_or_null())),
        3 => with_false(list_atomic(scalar_or_null())),
        4 => with_false(keyed_array_atomic(get_arraykey(), scalar_or_null())),
        5 => with_false(TAtomic::Object(TObject::new_named(word("stdClass")))),
        6 => get_bool(),
        7 => TUnion::from_vec(vec![
            TAtomic::Scalar(TScalar::Generic),
            TAtomic::Null,
            TAtomic::Scalar(TScalar::r#false()),
        ]),
        8 => with_false(TAtomic::Object(TObject::Any)),
        11 => with_false(keyed_array_atomic(get_string(), named_row_value())),
        12 => TUnion::from_atomic(keyed_array_atomic(get_arraykey(), scalar_or_null())),
        _ => return None,
    })
}

fn fetch_all_return_type(
    context: &ProviderContext<'_, '_, '_>,
    invocation: &InvocationInfo<'_, '_, '_>,
    mode: i64,
) -> Option<TUnion> {
    let result = match mode {
        2 => list_atomic(TUnion::from_atomic(keyed_array_atomic(get_string(), scalar_or_null()))),
        3 => list_atomic(TUnion::from_atomic(list_atomic(scalar_or_null()))),
        4 => list_atomic(TUnion::from_atomic(keyed_array_atomic(get_arraykey(), scalar_or_null()))),
        5 => list_atomic(TUnion::from_atomic(TAtomic::Object(TObject::new_named(word("stdClass"))))),
        6 => list_atomic(get_bool()),
        7 => list_atomic(scalar_or_null()),
        8 => {
            let element = fetch_class_name(context, invocation).map(TObject::new_named).unwrap_or(TObject::Any);

            list_atomic(TUnion::from_atomic(TAtomic::Object(element)))
        }
        11 => list_atomic(TUnion::from_atomic(keyed_array_atomic(get_string(), named_row_value()))),
        12 => keyed_array_atomic(get_arraykey(), scalar_or_null()),
        _ => return None,
    };

    Some(TUnion::from_atomic(result))
}

fn fetch_class_name(context: &ProviderContext<'_, '_, '_>, invocation: &InvocationInfo<'_, '_, '_>) -> Option<Word> {
    let class_argument = invocation.get_argument(1, &[b"class", b"classname"])?;
    let class_type = context.get_expression_type(class_argument)?.get_single();

    match class_type {
        TAtomic::Scalar(TScalar::ClassLikeString(TClassLikeString::Literal { value })) => Some(*value),
        TAtomic::Scalar(TScalar::ClassLikeString(TClassLikeString::OfType { constraint, .. })) => {
            match constraint.as_ref() {
                TAtomic::Object(TObject::Named(named)) => Some(named.name),
                TAtomic::Object(TObject::Enum(r#enum)) => Some(r#enum.name),
                _ => None,
            }
        }
        TAtomic::Scalar(TScalar::String(string)) => match string.literal {
            Some(TStringLiteral::Value(value)) => Some(value),
            _ => None,
        },
        _ => None,
    }
}

fn scalar_or_null() -> TUnion {
    TUnion::from_vec(vec![TAtomic::Scalar(TScalar::Generic), TAtomic::Null])
}

fn named_row_value() -> TUnion {
    TUnion::from_vec(vec![TAtomic::Scalar(TScalar::Generic), TAtomic::Null, list_atomic(scalar_or_null())])
}

fn list_atomic(element: TUnion) -> TAtomic {
    TAtomic::Array(TArray::List(TList::new(Arc::new(element))))
}

fn keyed_array_atomic(key: TUnion, value: TUnion) -> TAtomic {
    TAtomic::Array(TArray::Keyed(TKeyedArray::new_with_parameters(Arc::new(key), Arc::new(value))))
}

fn with_false(atomic: TAtomic) -> TUnion {
    TUnion::from_vec(vec![atomic, TAtomic::Scalar(TScalar::r#false())])
}
