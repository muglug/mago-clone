use std::collections::BTreeMap;

use mago_names::ResolvedNames;
use mago_syntax::cst::BinaryOperator;
use mago_syntax::cst::Block;
use mago_syntax::cst::Call;
use mago_syntax::cst::Expression;
use mago_syntax::cst::FunctionCall;
use mago_syntax::cst::IfBody;
use mago_syntax::cst::Literal;
use mago_syntax::cst::Statement;
use mago_syntax::cst::UnaryPrefixOperator;
use mago_syntax::cst::Variable;
use mago_word::Word;
use mago_word::WordSet;
use mago_word::word;

use crate::assertion::Assertion;
use crate::metadata::function_like::FunctionLikeMetadata;
use crate::ttype::atomic::TAtomic;
use crate::ttype::atomic::object::TObject;
use crate::ttype::atomic::object::named::TNamedObject;
use crate::ttype::atomic::scalar::TScalar;

type AssertionMap = BTreeMap<Word, Vec<Vec<Assertion>>>;

/// Infers assertions for a function-like whose body is a single expression
/// (arrow function).
pub(super) fn infer_assertions_from_expression_body<'arena>(
    expression: &'arena Expression<'arena>,
    metadata: &mut FunctionLikeMetadata,
    resolved_names: &ResolvedNames<'arena>,
) {
    if has_explicit_assertions(metadata) {
        return;
    }

    let parameter_names = collect_parameter_names(metadata);
    if parameter_names.is_empty() {
        return;
    }

    let (if_true, if_false) = infer_from_expression(expression, &parameter_names, resolved_names, false);

    apply_assertions(metadata, if_true, if_false);
}

/// Infers assertions for a function-like whose body is a block. Only single
/// `return expr;` bodies are recognized to keep inference sound: we cannot
/// reason about side effects or multi-path returns at scan time.
pub(super) fn infer_assertions_from_block_body<'arena>(
    body: &'arena Block<'arena>,
    metadata: &mut FunctionLikeMetadata,
    resolved_names: &ResolvedNames<'arena>,
) {
    if has_explicit_assertions(metadata) {
        return;
    }

    let parameter_names = collect_parameter_names(metadata);
    if parameter_names.is_empty() {
        return;
    }

    if let Some(return_expression) = single_return_expression(body) {
        let (if_true, if_false) = infer_from_expression(return_expression, &parameter_names, resolved_names, false);

        apply_assertions(metadata, if_true, if_false);
        return;
    }

    let assertions = infer_unconditional_from_terminating_guards(body, &parameter_names, resolved_names);
    apply_unconditional_assertions(metadata, assertions);
}

fn has_explicit_assertions(metadata: &FunctionLikeMetadata) -> bool {
    !metadata.assertions.is_empty()
        || !metadata.if_true_assertions.is_empty()
        || !metadata.if_false_assertions.is_empty()
}

fn collect_parameter_names(metadata: &FunctionLikeMetadata) -> WordSet {
    metadata.parameters.iter().map(|p| p.get_name().0).collect()
}

fn single_return_expression<'arena>(body: &'arena Block<'arena>) -> Option<&'arena Expression<'arena>> {
    if body.statements.len() != 1 {
        return None;
    }

    let Statement::Return(ret) = &body.statements.as_slice()[0] else {
        return None;
    };

    ret.value
}

fn apply_assertions(metadata: &mut FunctionLikeMetadata, if_true: AssertionMap, if_false: AssertionMap) {
    if if_true.is_empty() && if_false.is_empty() {
        return;
    }

    for (var, assertions) in if_true {
        metadata.if_true_assertions.entry(var).or_default().extend(assertions);
    }
    for (var, assertions) in if_false {
        metadata.if_false_assertions.entry(var).or_default().extend(assertions);
    }

    metadata.assertions_inferred = true;
}

fn apply_unconditional_assertions(metadata: &mut FunctionLikeMetadata, assertions: AssertionMap) {
    if assertions.is_empty() {
        return;
    }

    for (var, assertions) in assertions {
        metadata.assertions.entry(var).or_default().extend(assertions);
    }

    metadata.assertions_inferred = true;
}

fn infer_unconditional_from_terminating_guards<'arena>(
    body: &'arena Block<'arena>,
    parameter_names: &WordSet,
    resolved_names: &ResolvedNames<'arena>,
) -> AssertionMap {
    let mut assertions = AssertionMap::new();

    for statement in &body.statements {
        let Statement::If(guard) = statement else {
            continue;
        };

        let IfBody::Statement(guard_body) = &guard.body else {
            continue;
        };

        if !guard_body.else_if_clauses.is_empty()
            || guard_body.else_clause.is_some()
            || !statement_always_terminates(guard_body.statement)
        {
            continue;
        }

        let (_, if_false) = infer_from_expression(guard.condition, parameter_names, resolved_names, false);
        merge_assertion_maps(&mut assertions, if_false);
    }

    assertions
}

fn statement_always_terminates(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::Return(_) => true,
        Statement::Expression(statement) => matches!(unwrap_parens(statement.expression), Expression::Throw(_)),
        Statement::Block(block) => block.statements.as_slice().last().is_some_and(statement_always_terminates),
        _ => false,
    }
}

fn merge_assertion_maps(target: &mut AssertionMap, incoming: AssertionMap) {
    for (variable, clauses) in incoming {
        target.entry(variable).or_default().extend(clauses);
    }
}

fn infer_from_expression<'arena>(
    expression: &'arena Expression<'arena>,
    parameter_names: &WordSet,
    resolved_names: &ResolvedNames<'arena>,
    negated: bool,
) -> (AssertionMap, AssertionMap) {
    let expression = unwrap_parens(expression);

    match expression {
        Expression::UnaryPrefix(unary) if matches!(unary.operator, UnaryPrefixOperator::Not(_)) => {
            infer_from_expression(unary.operand, parameter_names, resolved_names, !negated)
        }
        Expression::Binary(binary) => match &binary.operator {
            BinaryOperator::And(_) | BinaryOperator::LowAnd(_) => {
                let (left_true, _) = infer_from_expression(binary.lhs, parameter_names, resolved_names, false);
                let (right_true, _) = infer_from_expression(binary.rhs, parameter_names, resolved_names, false);
                let mut if_true = left_true;
                merge_assertion_maps(&mut if_true, right_true);
                if negated { (AssertionMap::new(), if_true) } else { (if_true, AssertionMap::new()) }
            }
            BinaryOperator::Or(_) | BinaryOperator::LowOr(_) => {
                let (_, left_false) = infer_from_expression(binary.lhs, parameter_names, resolved_names, false);
                let (_, right_false) = infer_from_expression(binary.rhs, parameter_names, resolved_names, false);
                let mut if_false = left_false;
                merge_assertion_maps(&mut if_false, right_false);
                if negated { (if_false, AssertionMap::new()) } else { (AssertionMap::new(), if_false) }
            }
            BinaryOperator::Instanceof(_) => parse_instanceof(binary.lhs, binary.rhs, parameter_names, resolved_names)
                .map(|(var, atomic)| build_assertions(var, atomic, negated))
                .unwrap_or_default(),
            BinaryOperator::Identical(_) | BinaryOperator::Equal(_) => {
                parse_null_compare(binary.lhs, binary.rhs, parameter_names)
                    .map(|var| build_assertions(var, TAtomic::Null, negated))
                    .unwrap_or_default()
            }
            BinaryOperator::NotIdentical(_) | BinaryOperator::NotEqual(_) | BinaryOperator::AngledNotEqual(_) => {
                parse_null_compare(binary.lhs, binary.rhs, parameter_names)
                    .map(|var| build_assertions(var, TAtomic::Null, !negated))
                    .unwrap_or_default()
            }
            _ => Default::default(),
        },
        Expression::Call(Call::Function(call)) => parse_type_check_function(call, parameter_names, resolved_names)
            .map(|(var, atomic)| build_assertions(var, atomic, negated))
            .unwrap_or_default(),
        _ => Default::default(),
    }
}

fn build_assertions(var: Word, atomic: TAtomic, negated: bool) -> (AssertionMap, AssertionMap) {
    let mut if_true = AssertionMap::new();
    let mut if_false = AssertionMap::new();

    if negated {
        if_true.insert(var, vec![vec![Assertion::IsNotType(atomic.clone())]]);
        if_false.insert(var, vec![vec![Assertion::IsType(atomic)]]);
    } else {
        if_true.insert(var, vec![vec![Assertion::IsType(atomic.clone())]]);
        if_false.insert(var, vec![vec![Assertion::IsNotType(atomic)]]);
    }

    (if_true, if_false)
}

fn unwrap_parens<'expr, 'arena>(mut expression: &'expr Expression<'arena>) -> &'expr Expression<'arena> {
    while let Expression::Parenthesized(p) = expression {
        expression = p.expression;
    }
    expression
}

fn parameter_var(expression: &Expression<'_>, parameter_names: &WordSet) -> Option<Word> {
    let Expression::Variable(Variable::Direct(direct)) = unwrap_parens(expression) else {
        return None;
    };

    let candidate = word(direct.name);
    if parameter_names.contains(&candidate) { Some(candidate) } else { None }
}

fn parse_instanceof<'arena>(
    lhs: &Expression<'arena>,
    rhs: &Expression<'arena>,
    parameter_names: &WordSet,
    resolved_names: &ResolvedNames<'arena>,
) -> Option<(Word, TAtomic)> {
    let var = parameter_var(lhs, parameter_names)?;

    let Expression::Identifier(identifier) = unwrap_parens(rhs) else {
        return None;
    };

    let class_name = word(resolved_names.get(identifier));

    Some((var, TAtomic::Object(TObject::Named(TNamedObject::new(class_name)))))
}

fn parse_null_compare<'arena>(
    lhs: &Expression<'arena>,
    rhs: &Expression<'arena>,
    parameter_names: &WordSet,
) -> Option<Word> {
    if let Some(var) = parameter_var(lhs, parameter_names)
        && is_null_literal(rhs)
    {
        return Some(var);
    }

    if let Some(var) = parameter_var(rhs, parameter_names)
        && is_null_literal(lhs)
    {
        return Some(var);
    }

    None
}

fn is_null_literal(expression: &Expression<'_>) -> bool {
    matches!(unwrap_parens(expression), Expression::Literal(Literal::Null(_)))
}

fn parse_type_check_function<'arena>(
    call: &FunctionCall<'arena>,
    parameter_names: &WordSet,
    resolved_names: &ResolvedNames<'arena>,
) -> Option<(Word, TAtomic)> {
    let Expression::Identifier(function_id) = call.function else {
        return None;
    };

    let resolved = resolved_names.get(function_id);
    let atomic = type_for_check_function(resolved)?;

    if call.argument_list.arguments.len() != 1 {
        return None;
    }

    let arg = call.argument_list.arguments.iter().next()?;

    parameter_var(arg.value(), parameter_names).map(|var| (var, atomic))
}

fn type_for_check_function(name: &[u8]) -> Option<TAtomic> {
    match name.to_ascii_lowercase().as_slice() {
        b"is_int" | b"is_integer" | b"is_long" => Some(TAtomic::Scalar(TScalar::int())),
        b"is_string" => Some(TAtomic::Scalar(TScalar::string())),
        b"is_float" | b"is_double" | b"is_real" => Some(TAtomic::Scalar(TScalar::float())),
        b"is_bool" => Some(TAtomic::Scalar(TScalar::bool())),
        b"is_null" => Some(TAtomic::Null),
        b"is_object" => Some(TAtomic::Object(TObject::Any)),
        b"is_numeric" => Some(TAtomic::Scalar(TScalar::numeric())),
        _ => None,
    }
}
