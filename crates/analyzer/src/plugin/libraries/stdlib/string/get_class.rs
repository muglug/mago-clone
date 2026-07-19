//! `get_class()` return type provider.

use mago_codex::ttype::atomic::TAtomic;
use mago_codex::ttype::atomic::scalar::TScalar;
use mago_codex::ttype::atomic::scalar::class_like_string::TClassLikeString;
use mago_codex::ttype::atomic::scalar::class_like_string::TClassLikeStringKind;
use mago_codex::ttype::union::TUnion;

use crate::plugin::context::InvocationInfo;
use crate::plugin::context::ProviderContext;
use crate::plugin::provider::Provider;
use crate::plugin::provider::ProviderMeta;
use crate::plugin::provider::function::FunctionReturnTypeProvider;
use crate::plugin::provider::function::FunctionTarget;

static META: ProviderMeta = ProviderMeta::new(
    "php::string::get_class",
    "get_class",
    "Preserves the object's class template in the returned class-string",
);

#[derive(Default)]
pub struct GetClassProvider;

impl Provider for GetClassProvider {
    fn meta() -> &'static ProviderMeta {
        &META
    }
}

impl FunctionReturnTypeProvider for GetClassProvider {
    fn targets() -> FunctionTarget {
        FunctionTarget::Exact(b"get_class")
    }

    fn get_return_type(
        &self,
        context: &ProviderContext<'_, '_, '_>,
        invocation: &InvocationInfo<'_, '_, '_>,
    ) -> Option<TUnion> {
        let object_argument = invocation.get_argument(0, &[b"object"])?;
        let object_type = context.get_expression_type(object_argument)?;
        let mut class_strings = Vec::new();

        for atomic in object_type.types.iter() {
            match atomic {
                TAtomic::GenericParameter(parameter) => {
                    class_strings.extend(parameter.constraint.types.iter().cloned().map(|constraint| {
                        TAtomic::Scalar(TScalar::ClassLikeString(TClassLikeString::generic(
                            TClassLikeStringKind::Class,
                            parameter.parameter_name,
                            parameter.defining_entity,
                            constraint,
                        )))
                    }));
                }
                TAtomic::Object(_) => {
                    class_strings.push(TAtomic::Scalar(TScalar::ClassLikeString(
                        TClassLikeString::class_string_of_type(atomic.clone()),
                    )));
                }
                _ => {}
            }
        }

        (!class_strings.is_empty()).then(|| TUnion::from_vec(class_strings))
    }
}
