use mago_allocator::Arena;
use mago_reporting::Annotation;
use mago_reporting::Issue;
use mago_span::HasSpan;
use mago_syntax::cst::Access;
use mago_syntax::cst::BinaryOperator;
use mago_syntax::cst::Call;
use mago_syntax::cst::Construct;
use mago_syntax::cst::Expression;
use mago_syntax::cst::FunctionCall;
use mago_syntax::cst::Literal;
use mago_word::Word;
use mago_word::ascii_lowercase_word;

use crate::artifacts::AnalysisArtifacts;
use crate::context::Context;
use crate::context::assertion::AssertionContext;
use crate::context::block::BlockContext;
use crate::context::block::GuardedExpression;
use crate::context::block::GuardedExpressionKind;
use crate::utils::expression::get_expression_id;
use crate::utils::expression::get_stable_method_call_advisory_id;
use crate::utils::misc::unwrap_expression;

/// Records property fetches and stable-result method calls that a true branch
/// has checked. The records only improve later diagnostics; they never become
/// formula subjects or entries in `BlockContext::locals`.
pub fn record_true_branch_guards<A>(
    condition: &Expression<'_>,
    artifacts: &AnalysisArtifacts,
    assertion_context: AssertionContext<'_, '_, A>,
    block_context: &mut BlockContext<'_>,
) where
    A: Arena,
{
    collect_guards(condition, true, artifacts, assertion_context, block_context);
}

fn collect_guards<A>(
    expression: &Expression<'_>,
    branch_is_truthy: bool,
    artifacts: &AnalysisArtifacts,
    assertion_context: AssertionContext<'_, '_, A>,
    block_context: &mut BlockContext<'_>,
) where
    A: Arena,
{
    let expression = unwrap_expression(expression);

    match expression {
        Expression::UnaryPrefix(unary) if unary.operator.is_not() => {
            collect_guards(unary.operand, !branch_is_truthy, artifacts, assertion_context, block_context);
        }
        Expression::Binary(binary) => match binary.operator {
            BinaryOperator::And(_) | BinaryOperator::LowAnd(_) if branch_is_truthy => {
                collect_guards(binary.lhs, true, artifacts, assertion_context, block_context);
                collect_guards(binary.rhs, true, artifacts, assertion_context, block_context);
            }
            BinaryOperator::Or(_) | BinaryOperator::LowOr(_) if !branch_is_truthy => {
                collect_guards(binary.lhs, false, artifacts, assertion_context, block_context);
                collect_guards(binary.rhs, false, artifacts, assertion_context, block_context);
            }
            BinaryOperator::Identical(_) | BinaryOperator::Equal(_) => {
                if branch_is_truthy {
                    record_if_non_null_literal_comparison(
                        binary.lhs,
                        binary.rhs,
                        artifacts,
                        assertion_context,
                        block_context,
                    );
                }
            }
            BinaryOperator::NotIdentical(_) | BinaryOperator::NotEqual(_) => {
                if branch_is_truthy {
                    record_if_nullish_literal_comparison(
                        binary.lhs,
                        binary.rhs,
                        artifacts,
                        assertion_context,
                        block_context,
                    );
                }
            }
            BinaryOperator::Instanceof(_) if branch_is_truthy => {
                record_guarded_expression(binary.lhs, artifacts, assertion_context, block_context);
            }
            _ => {}
        },
        Expression::Call(Call::Function(function_call)) => {
            collect_function_call_guard(function_call, branch_is_truthy, artifacts, assertion_context, block_context);
        }
        Expression::Construct(Construct::Isset(isset)) if branch_is_truthy => {
            for value in isset.values.iter() {
                record_guarded_expression(value, artifacts, assertion_context, block_context);
            }
        }
        Expression::Construct(Construct::Empty(empty)) if !branch_is_truthy => {
            record_guarded_expression(empty.value, artifacts, assertion_context, block_context);
        }
        _ if branch_is_truthy => {
            record_guarded_expression(expression, artifacts, assertion_context, block_context);
        }
        _ => {}
    }
}

fn collect_function_call_guard<A>(
    function_call: &FunctionCall<'_>,
    branch_is_truthy: bool,
    artifacts: &AnalysisArtifacts,
    assertion_context: AssertionContext<'_, '_, A>,
    block_context: &mut BlockContext<'_>,
) where
    A: Arena,
{
    let Expression::Identifier(identifier) = function_call.function else {
        return;
    };

    let unresolved = ascii_lowercase_word(identifier.value());
    let resolved = ascii_lowercase_word(assertion_context.resolved_names.get(identifier));
    let name = if identifier.is_local() && is_builtin_guard(unresolved.as_bytes()) { unresolved } else { resolved };

    let positive_guard = branch_is_truthy && is_positive_type_guard(name.as_bytes());
    let negative_null_guard = !branch_is_truthy && name.as_bytes() == b"is_null";
    if !positive_guard && !negative_null_guard {
        return;
    }

    if let Some(argument) = function_call.argument_list.arguments.first() {
        record_guarded_expression(argument.value(), artifacts, assertion_context, block_context);
    }
}

fn is_builtin_guard(name: &[u8]) -> bool {
    is_positive_type_guard(name) || name == b"is_null"
}

fn is_positive_type_guard(name: &[u8]) -> bool {
    matches!(
        name,
        b"is_array"
            | b"is_bool"
            | b"is_callable"
            | b"is_countable"
            | b"is_float"
            | b"is_double"
            | b"is_real"
            | b"is_int"
            | b"is_integer"
            | b"is_long"
            | b"is_iterable"
            | b"is_numeric"
            | b"is_object"
            | b"is_resource"
            | b"is_scalar"
            | b"is_string"
            | b"ctype_alnum"
            | b"ctype_alpha"
            | b"ctype_cntrl"
            | b"ctype_digit"
            | b"ctype_graph"
            | b"ctype_lower"
            | b"ctype_print"
            | b"ctype_punct"
            | b"ctype_space"
            | b"ctype_upper"
            | b"ctype_xdigit"
    )
}

fn record_if_nullish_literal_comparison<A>(
    left: &Expression<'_>,
    right: &Expression<'_>,
    artifacts: &AnalysisArtifacts,
    assertion_context: AssertionContext<'_, '_, A>,
    block_context: &mut BlockContext<'_>,
) where
    A: Arena,
{
    if is_nullish_literal(left) {
        record_guarded_expression(right, artifacts, assertion_context, block_context);
    } else if is_nullish_literal(right) {
        record_guarded_expression(left, artifacts, assertion_context, block_context);
    }
}

fn record_if_non_null_literal_comparison<A>(
    left: &Expression<'_>,
    right: &Expression<'_>,
    artifacts: &AnalysisArtifacts,
    assertion_context: AssertionContext<'_, '_, A>,
    block_context: &mut BlockContext<'_>,
) where
    A: Arena,
{
    if is_definitely_non_null_literal(left) {
        record_guarded_expression(right, artifacts, assertion_context, block_context);
    } else if is_definitely_non_null_literal(right) {
        record_guarded_expression(left, artifacts, assertion_context, block_context);
    }
}

fn is_nullish_literal(expression: &Expression<'_>) -> bool {
    matches!(unwrap_expression(expression), Expression::Literal(Literal::Null(_) | Literal::False(_)))
}

fn is_definitely_non_null_literal(expression: &Expression<'_>) -> bool {
    matches!(
        unwrap_expression(expression),
        Expression::Literal(Literal::String(_) | Literal::Integer(_) | Literal::Float(_) | Literal::True(_))
    )
}

fn record_guarded_expression<A>(
    expression: &Expression<'_>,
    artifacts: &AnalysisArtifacts,
    assertion_context: AssertionContext<'_, '_, A>,
    block_context: &mut BlockContext<'_>,
) where
    A: Arena,
{
    let Some((id, kind)) = get_guarded_expression_id(expression, artifacts, assertion_context) else {
        return;
    };

    block_context.guarded_expressions.insert(id, GuardedExpression { condition_span: expression.span(), kind });
}

fn get_guarded_expression_id<A>(
    expression: &Expression<'_>,
    artifacts: &AnalysisArtifacts,
    assertion_context: AssertionContext<'_, '_, A>,
) -> Option<(Word, GuardedExpressionKind)>
where
    A: Arena,
{
    let expression = unwrap_expression(expression);

    if matches!(expression, Expression::Call(Call::Method(_) | Call::NullSafeMethod(_))) {
        let id = get_stable_method_call_advisory_id(
            expression,
            assertion_context.this_class_name,
            assertion_context.resolved_names,
            Some(assertion_context.codebase),
            &artifacts.stable_method_call_offsets,
        )?;

        return Some((id, GuardedExpressionKind::MethodCall));
    }

    if matches!(expression, Expression::Access(Access::Property(_) | Access::NullSafeProperty(_))) {
        let id = get_expression_id(
            expression,
            assertion_context.this_class_name,
            assertion_context.resolved_names,
            Some(assertion_context.codebase),
        )?;

        return Some((id, GuardedExpressionKind::PropertyFetch));
    }

    None
}

/// Adds a local-value recommendation when an existing diagnostic was caused
/// by re-evaluating an expression that an earlier guard had checked.
pub fn add_guarded_expression_advice<'ctx, 'arena, A>(
    issue: Issue,
    expression: &Expression<'arena>,
    context: &Context<'ctx, 'arena, A>,
    block_context: &BlockContext<'ctx>,
    artifacts: &AnalysisArtifacts,
) -> Issue
where
    A: Arena,
{
    if block_context.guarded_expressions.is_empty() {
        return issue;
    }

    let assertion_context = context.get_assertion_context_from_block(block_context);
    let Some((id, _)) = get_guarded_expression_id(expression, artifacts, assertion_context) else {
        return issue;
    };
    let Some(guard) = block_context.guarded_expressions.get(&id) else {
        return issue;
    };

    let expression_id = String::from_utf8_lossy(id.as_bytes());
    let (evaluation, stability) = match guard.kind {
        GuardedExpressionKind::MethodCall => (
            "call",
            "Mago analyzes every method invocation as a new evaluation, even when the method is pure or cannot be overridden.",
        ),
        GuardedExpressionKind::PropertyFetch => (
            "property fetch",
            "A property value may have changed or its earlier refinement may have been invalidated before this fetch.",
        ),
    };

    issue
        .with_annotation(
            Annotation::secondary(guard.condition_span)
                .with_message(format!("An earlier condition checked `{expression_id}` here")),
        )
        .with_note(format!("The earlier check narrowed a different {evaluation} of `{expression_id}`. {stability}"))
        .with_help(format!(
            "Evaluate `{expression_id}` once, store the result in a local variable, and check and use that local value."
        ))
}
