//! `Closure::fromCallable()` return type provider.

use mago_codex::ttype::atomic::TAtomic;
use mago_codex::ttype::atomic::callable::TCallable;
use mago_codex::ttype::cast::cast_atomic_to_callable;
use mago_codex::ttype::expander::get_signature_of_function_like_identifier;
use mago_codex::ttype::template::TemplateResult;
use mago_codex::ttype::template::inferred_type_replacer;
use mago_codex::ttype::union::TUnion;

use crate::plugin::context::InvocationInfo;
use crate::plugin::context::ProviderContext;
use crate::plugin::provider::Provider;
use crate::plugin::provider::ProviderMeta;
use crate::plugin::provider::method::MethodReturnTypeProvider;
use crate::plugin::provider::method::MethodTarget;

static META: ProviderMeta = ProviderMeta::new(
    "php::closure::from-callable",
    "Closure::fromCallable",
    "Preserves the source callable signature as a closure",
);

static TARGETS: [MethodTarget; 1] = [MethodTarget::exact(b"closure", b"fromcallable")];

#[derive(Default)]
pub struct ClosureFromCallableProvider;

impl Provider for ClosureFromCallableProvider {
    fn meta() -> &'static ProviderMeta {
        &META
    }
}

impl MethodReturnTypeProvider for ClosureFromCallableProvider {
    fn targets() -> &'static [MethodTarget] {
        &TARGETS
    }

    fn get_return_type(
        &self,
        context: &ProviderContext<'_, '_, '_>,
        _class_name: &[u8],
        _method_name: &[u8],
        invocation: &InvocationInfo<'_, '_, '_>,
    ) -> Option<TUnion> {
        let argument = invocation.get_argument(0, &[b"callback"])?;
        let argument_type = context.get_expression_type(argument)?;
        let codebase = context.codebase();
        let mut result = Vec::new();

        for atomic in argument_type.types.as_ref() {
            let mut template_result = TemplateResult::default();
            let callable = cast_atomic_to_callable(atomic, codebase, Some(&mut template_result))?;
            let signature = match callable.as_ref() {
                TCallable::Signature(signature) => signature.clone_as_closure(),
                TCallable::Alias(identifier) => {
                    get_signature_of_function_like_identifier(identifier, codebase)?.clone_as_closure()
                }
            };

            let closure = TUnion::from_atomic(TAtomic::Callable(TCallable::Signature(signature)));
            let closure = if template_result.has_template_types() || !template_result.lower_bounds.is_empty() {
                inferred_type_replacer::replace(&closure, &template_result, codebase)
            } else {
                closure
            };
            result.extend(closure.types.into_owned());
        }

        (!result.is_empty()).then(|| TUnion::from_vec(result))
    }
}
