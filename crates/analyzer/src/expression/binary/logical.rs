use mago_allocator::Arena;
use std::rc::Rc;

use foldhash::HashSet;
use indexmap::IndexMap;

use mago_algebra::find_satisfying_assignments;
use mago_algebra::saturate_clauses;
use mago_bytes::BytesDisplay;
use mago_codex::ttype::combine_union_types;
use mago_codex::ttype::combine_union_types_rc;
use mago_codex::ttype::get_bool;
use mago_codex::ttype::get_false;
use mago_codex::ttype::get_mixed;
use mago_codex::ttype::get_true;
use mago_codex::ttype::union::TUnion;
use mago_reporting::Annotation;
use mago_reporting::Issue;
use mago_span::HasSpan;
use mago_syntax::cst::Binary;
use mago_syntax::cst::BinaryOperator;
use mago_syntax::cst::Expression;
use mago_text_edit::Safety;
use mago_text_edit::TextEdit;
use mago_word::WordSet;

use crate::analyzable::Analyzable;
use crate::artifacts::AnalysisArtifacts;
use crate::artifacts::get_expression_range;
use crate::code::IssueCode;
use crate::context::Context;
use crate::context::block::BlockContext;
use crate::context::scope::if_scope::IfScope;
use crate::error::AnalysisError;
use crate::formula::get_formula;
use crate::formula::negate_or_synthesize;
use crate::reconciler;
use crate::utils::conditional;
use crate::utils::expression::expression_has_observable_side_effect;
use crate::utils::expression::get_expression_id;
use crate::utils::symbol_existence::extract_function_constant_existence;

#[inline]
pub fn analyze_logical_and_operation<'ctx, 'arena, A>(
    binary: &Binary<'arena>,
    context: &mut Context<'ctx, 'arena, A>,
    block_context: &mut BlockContext<'ctx>,
    artifacts: &mut AnalysisArtifacts,
) -> Result<(), AnalysisError>
where
    A: Arena,
{
    let left_is_nested_and = is_logical_operator_expression(binary.lhs, true);
    let pre_left_replay_context = left_is_nested_and.then(|| block_context.clone());
    let mut left_block_context = block_context.clone();
    let pre_referenced_var_ids = left_block_context.conditionally_referenced_variable_ids.clone();
    let pre_assigned_var_ids = left_block_context.assigned_variable_ids.clone();
    let pre_conflicting_clause_vars = left_block_context.parent_conflicting_clause_variables.clone();
    left_block_context.conditionally_referenced_variable_ids.clear();
    left_block_context.assigned_variable_ids.clear();
    left_block_context.parent_conflicting_clause_variables.clear();
    left_block_context.reconciled_expression_clauses = Vec::new();

    let left_was_inside_general_use = left_block_context.flags.inside_general_use();
    left_block_context.flags.set_inside_general_use(true);
    binary.lhs.analyze(context, &mut left_block_context, artifacts)?;
    left_block_context.flags.set_inside_general_use(left_was_inside_general_use);

    extract_function_constant_existence(binary.lhs, artifacts, &mut left_block_context, false);

    let lhs_type = match artifacts.get_rc_expression_type(&binary.lhs).cloned() {
        Some(lhs_type) => {
            check_logical_operand(context, binary.lhs, &lhs_type, "Left", "&&");

            lhs_type
        }
        None => Rc::new(get_mixed()),
    };

    let left_clauses = get_formula(
        binary.lhs.span(),
        binary.lhs.span(),
        binary.lhs,
        context.get_assertion_context_from_block(block_context),
        artifacts,
        &context.settings.algebra_thresholds(),
        context.settings.formula_size_threshold,
    )
    .unwrap_or_default();

    // Statement-level `A && B` needs the left formula again after `left_clauses`
    // is consumed, to reconstruct the `!A` world during the final merge.
    let saved_left_clauses = if block_context.if_body_context.is_none() { Some(left_clauses.clone()) } else { None };

    for (var_id, var_type) in &left_block_context.locals {
        if left_block_context.assigned_variable_ids.contains_key(var_id) {
            block_context.locals.insert(*var_id, Rc::clone(var_type));
        }
    }

    let mut left_referenced_var_ids = left_block_context.conditionally_referenced_variable_ids.clone();
    let mut context_clauses = left_block_context.clauses.iter().map(|v| &**v).collect::<Vec<_>>();
    block_context.conditionally_referenced_variable_ids.extend(pre_referenced_var_ids);
    block_context.assigned_variable_ids.extend(pre_assigned_var_ids);
    context_clauses.extend(left_clauses.iter());
    if !left_block_context.reconciled_expression_clauses.is_empty() {
        let left_reconciled_clauses_hashed =
            left_block_context.reconciled_expression_clauses.iter().map(|v| &**v).collect::<HashSet<_>>();

        context_clauses.retain(|c| !left_reconciled_clauses_hashed.contains(c));
        if context_clauses.len() == 1 {
            let first = &context_clauses[0];
            if first.wedge && first.possibilities.is_empty() {
                context_clauses = Vec::new();
            }
        }
    }

    let simplified_clauses = saturate_clauses(context_clauses, &context.settings.algebra_thresholds());
    let (mut left_assertions, active_left_assertions) = find_satisfying_assignments(
        simplified_clauses.as_slice(),
        Some(binary.lhs.span()),
        &mut left_referenced_var_ids,
    );

    if !left_block_context.parent_conflicting_clause_variables.is_empty() {
        left_assertions.retain(|var_id, _| !left_block_context.parent_conflicting_clause_variables.contains(var_id));
    }

    let mut changed_var_ids = WordSet::default();
    let mut right_block_context;
    if left_assertions.is_empty() {
        right_block_context = left_block_context.clone();
    } else {
        right_block_context = block_context.clone();

        right_block_context.known_functions.extend(left_block_context.known_functions.iter().copied());
        right_block_context.known_constants.extend(left_block_context.known_constants.iter().copied());

        let empty_referenced_vars = WordSet::default();

        reconciler::reconcile_keyed_types(
            context,
            &left_assertions,
            active_left_assertions,
            &mut right_block_context,
            &mut changed_var_ids,
            &empty_referenced_vars,
            &binary.rhs.span(),
            !binary.operator.span().is_zero(),
            !block_context.flags.inside_negation(),
        );
    }

    if let Some(pre_left_replay_context) = pre_left_replay_context
        && !left_block_context.assigned_variable_ids.is_empty()
    {
        replay_short_circuit_assignments(
            context,
            binary.lhs,
            pre_left_replay_context,
            &mut right_block_context,
            &left_block_context.assigned_variable_ids.keys().copied().collect(),
            artifacts,
            true,
        );
    }

    let partitioned_clauses = BlockContext::remove_reconciled_clause_refs(
        &{
            let mut c = left_block_context.clauses.clone();
            c.extend(left_clauses.into_iter().map(Rc::new));
            c
        },
        &changed_var_ids,
    );
    right_block_context.clauses = partitioned_clauses.0;

    let result_type: TUnion;
    if lhs_type.is_always_falsy() {
        if !block_context.flags.inside_loop_expressions() {
            report_redundant_logical_operation(context, binary, "always falsy", "not evaluated", "`false`", None);
        }

        result_type = get_false();
        let mut dead_rhs_context = right_block_context.clone();
        dead_rhs_context.flags.set_has_returned(true);
        binary.rhs.analyze(context, &mut dead_rhs_context, artifacts)?;
    } else {
        binary.rhs.analyze(context, &mut right_block_context, artifacts)?;
        let rhs_type = match artifacts.get_rc_expression_type(&binary.rhs).cloned() {
            Some(rhs_type) => {
                check_logical_operand(context, binary.rhs, &rhs_type, "Right", "&&");

                rhs_type
            }
            None => Rc::new(get_mixed()),
        };

        let left_is_truthy = lhs_type.is_always_truthy();
        if left_is_truthy && !block_context.flags.inside_loop_expressions() {
            // true && x → x (remove left, keep right)
            report_redundant_logical_operation(
                context,
                binary,
                "always truthy",
                "evaluated",
                "the boolean value of the right-hand side",
                Some(false), // remove left
            );
        }

        if rhs_type.is_always_falsy() {
            // x && false → false (no fix)
            if !block_context.flags.inside_loop_expressions() {
                report_redundant_logical_operation(context, binary, "evaluated", "always falsy", "`false`", None);
            }

            result_type = get_false();
        } else if rhs_type.is_always_truthy() {
            // x && true → x (remove right, keep left)
            if !block_context.flags.inside_loop_expressions() {
                report_redundant_logical_operation(
                    context,
                    binary,
                    "evaluated",
                    "always truthy",
                    "the boolean value of the left-hand side",
                    Some(true), // remove right
                );
            }

            if left_is_truthy {
                result_type = get_true();
            } else {
                result_type = get_bool();
            }
        } else {
            result_type = get_bool();
        }
    }

    artifacts.set_expression_type(binary, result_type);

    block_context.conditionally_referenced_variable_ids =
        left_block_context.conditionally_referenced_variable_ids.clone();
    block_context
        .conditionally_referenced_variable_ids
        .extend(right_block_context.conditionally_referenced_variable_ids.clone());

    let left_assigned_var_ids = left_block_context.assigned_variable_ids.clone();
    let right_assigned_var_ids = right_block_context.assigned_variable_ids.clone();
    block_context.assigned_variable_ids.clone_from(&left_assigned_var_ids);
    block_context.assigned_variable_ids.extend(right_assigned_var_ids.clone());
    block_context
        .possibly_assigned_variable_ids
        .extend(left_block_context.possibly_assigned_variable_ids.iter().copied());
    block_context
        .possibly_assigned_variable_ids
        .extend(right_block_context.possibly_assigned_variable_ids.iter().copied());
    block_context.variables_possibly_in_scope.extend(left_block_context.variables_possibly_in_scope.iter().copied());
    block_context.variables_possibly_in_scope.extend(right_block_context.variables_possibly_in_scope.iter().copied());

    // Propagate clause invalidations from both sides, plus restore pre-existing
    // ones, so parent expressions know which variables had their narrowing voided.
    block_context.parent_conflicting_clause_variables.extend(pre_conflicting_clause_vars);
    block_context
        .parent_conflicting_clause_variables
        .extend(left_block_context.parent_conflicting_clause_variables.iter().copied());
    block_context
        .parent_conflicting_clause_variables
        .extend(right_block_context.parent_conflicting_clause_variables.iter().copied());

    if let Some(if_body_context) = &block_context.if_body_context {
        let mut if_body_context_inner = if_body_context.borrow_mut();

        if block_context.flags.inside_negation() {
            block_context.locals = left_block_context.locals;
        } else {
            block_context.locals = right_block_context.locals;

            if_body_context_inner.locals.extend(block_context.locals.iter().map(|(k, v)| (*k, Rc::clone(v))));
            if_body_context_inner
                .conditionally_referenced_variable_ids
                .extend(block_context.conditionally_referenced_variable_ids.iter().copied());
            if_body_context_inner
                .assigned_variable_ids
                .extend(block_context.assigned_variable_ids.iter().map(|(k, v)| (*k, *v)));
            if_body_context_inner.reconciled_expression_clauses.extend(partitioned_clauses.1);
        }
    } else {
        // Statement-level `A && B;`: a variable assigned inside B ends up with
        // either its B-assigned type (A was truthy) or its left-context type
        // narrowed by `!A` (B never ran). Merge both worlds for variables the
        // RHS assigned; everything else keeps the left state.
        let rhs_merged_vars: Vec<_> = right_assigned_var_ids
            .keys()
            .filter(|var_id| {
                !left_assigned_var_ids.contains_key(*var_id) && left_block_context.locals.contains_key(*var_id)
            })
            .copied()
            .collect();

        let negated_left_locals = if rhs_merged_vars.is_empty() {
            None
        } else {
            saved_left_clauses.map(|left_clauses_for_negation| {
                let negated_clauses = negate_or_synthesize(
                    left_clauses_for_negation,
                    binary.lhs,
                    context.get_assertion_context_from_block(block_context),
                    artifacts,
                    &context.settings.algebra_thresholds(),
                    context.settings.formula_size_threshold,
                );

                let all_negated_clauses = saturate_clauses(
                    block_context.clauses.iter().map(|v| &**v).chain(negated_clauses.iter()),
                    &context.settings.algebra_thresholds(),
                );

                let (negated_assertions, _) =
                    find_satisfying_assignments(all_negated_clauses.as_slice(), None, &mut WordSet::default());

                let mut else_context = left_block_context.clone();
                if !negated_assertions.is_empty() {
                    reconciler::reconcile_keyed_types(
                        context,
                        &negated_assertions,
                        IndexMap::new(),
                        &mut else_context,
                        &mut WordSet::default(),
                        &WordSet::default(),
                        &binary.lhs.span(),
                        false,
                        false,
                    );
                }

                else_context.locals
            })
        };

        let left_locals = left_block_context.locals;
        for (var_id, var_type) in &right_block_context.locals {
            if right_assigned_var_ids.contains_key(var_id)
                && !left_assigned_var_ids.contains_key(var_id)
                && !left_locals.contains_key(var_id)
            {
                block_context.locals.insert(*var_id, Rc::clone(var_type));
            }
        }

        block_context.locals.extend(left_locals);

        if let Some(negated_left_locals) = negated_left_locals {
            for var_id in rhs_merged_vars {
                let Some(right_type) = right_block_context.locals.get(&var_id) else {
                    continue;
                };

                let merged_type = match negated_left_locals.get(&var_id) {
                    Some(else_type) => combine_union_types_rc(
                        right_type,
                        else_type,
                        context.codebase,
                        context.settings.combiner_options(),
                    ),
                    None => Rc::clone(right_type),
                };

                block_context.locals.insert(var_id, merged_type);
            }
        }
    }

    Ok(())
}

#[inline]
pub fn analyze_logical_or_operation<'ctx, 'arena, A>(
    binary: &Binary<'arena>,
    context: &mut Context<'ctx, 'arena, A>,
    block_context: &mut BlockContext<'ctx>,
    artifacts: &mut AnalysisArtifacts,
) -> Result<(), AnalysisError>
where
    A: Arena,
{
    let left_is_nested_or = is_logical_or_operation(binary.lhs, 3);
    let pre_left_replay_context = is_logical_operator_expression(binary.lhs, false).then(|| block_context.clone());
    let mut left_block_context;
    let mut left_referenced_var_ids;
    let left_assigned_var_ids;

    if left_is_nested_or {
        let pre_referenced_var_ids = block_context.conditionally_referenced_variable_ids.clone();
        block_context.conditionally_referenced_variable_ids.clear();

        let pre_assigned_var_ids = block_context.assigned_variable_ids.clone();

        left_block_context = block_context.clone();
        left_block_context.assigned_variable_ids.clear();

        let tmp_if_body_block_context = left_block_context.if_body_context;
        left_block_context.if_body_context = None;

        binary.lhs.analyze(context, &mut left_block_context, artifacts)?;

        left_block_context.if_body_context = tmp_if_body_block_context;

        for var_id in &left_block_context.parent_conflicting_clause_variables {
            block_context.remove_variable_from_conflicting_clauses(context, *var_id, None);
        }

        let cloned_vars = block_context.locals.clone();
        for (var_id, left_type) in &left_block_context.locals {
            if let Some(context_type) = cloned_vars.get(var_id) {
                block_context.locals.insert(
                    *var_id,
                    Rc::new(combine_union_types(
                        context_type,
                        left_type,
                        context.codebase,
                        context.settings.combiner_options(),
                    )),
                );
            } else if left_block_context.assigned_variable_ids.contains_key(var_id) {
                block_context.locals.insert(*var_id, Rc::clone(left_type));
            } else {
                // variable wasn't assigned in the left branch and isn't in the parent; drop it
            }
        }

        left_referenced_var_ids = left_block_context.conditionally_referenced_variable_ids.clone();
        left_block_context.conditionally_referenced_variable_ids.extend(pre_referenced_var_ids);

        left_assigned_var_ids = left_block_context.assigned_variable_ids.clone();
        left_block_context.assigned_variable_ids.extend(pre_assigned_var_ids);

        left_referenced_var_ids.retain(|id| !left_assigned_var_ids.contains_key(id));
    } else {
        let mut if_scope = IfScope::default();

        let (if_conditional_scope, applied_block_context) =
            conditional::analyze(context, block_context.clone(), artifacts, &mut if_scope, binary.lhs, false)?;
        *block_context = applied_block_context;

        left_assigned_var_ids = if_conditional_scope.assigned_in_conditional_variable_ids.clone();
        left_block_context = if_conditional_scope.if_body_context;
        left_referenced_var_ids = if_conditional_scope.conditionally_referenced_variable_ids;
    }

    let lhs_type = match artifacts.get_rc_expression_type(&binary.lhs).cloned() {
        Some(lhs_type) => {
            check_logical_operand(context, binary.lhs, &lhs_type, "Left", "||");

            lhs_type
        }
        None => Rc::new(get_mixed()),
    };

    let left_clauses = get_formula(
        binary.lhs.span(),
        binary.lhs.span(),
        binary.lhs,
        context.get_assertion_context_from_block(block_context),
        artifacts,
        &context.settings.algebra_thresholds(),
        context.settings.formula_size_threshold,
    )
    .unwrap_or_default();

    // Statement-level `A || B` needs the positive left formula again during the
    // final merge, to reconstruct the `A`-was-truthy world.
    let saved_left_clauses = if block_context.if_body_context.is_none() { Some(left_clauses.clone()) } else { None };

    let mut negated_left_clauses = negate_or_synthesize(
        left_clauses,
        binary.lhs,
        context.get_assertion_context_from_block(block_context),
        artifacts,
        &context.settings.algebra_thresholds(),
        context.settings.formula_size_threshold,
    );

    if !left_block_context.reconciled_expression_clauses.is_empty() {
        let left_reconciled_clauses_hashed =
            left_block_context.reconciled_expression_clauses.iter().map(|v| &**v).collect::<HashSet<_>>();

        negated_left_clauses.retain(|c| !left_reconciled_clauses_hashed.contains(c));

        if negated_left_clauses.len() == 1 {
            let first = &negated_left_clauses[0];
            if first.wedge && first.possibilities.is_empty() {
                negated_left_clauses = Vec::new();
            }
        }
    }

    let clauses_for_right_analysis = saturate_clauses(
        block_context.clauses.iter().map(|v| &**v).chain(negated_left_clauses.iter()),
        &context.settings.algebra_thresholds(),
    );

    let (negated_type_assertions, active_negated_type_assertions) = find_satisfying_assignments(
        clauses_for_right_analysis.as_slice(),
        Some(binary.lhs.span()),
        &mut left_referenced_var_ids,
    );

    let mut changed_var_ids = WordSet::default();
    let mut right_block_context = block_context.clone();
    let mut right_assigned_var_ids = Default::default();
    let mut right_redefined_var_ids = WordSet::default();

    if let Some(pre_left_replay_context) = pre_left_replay_context
        && !left_assigned_var_ids.is_empty()
    {
        replay_short_circuit_assignments(
            context,
            binary.lhs,
            pre_left_replay_context,
            &mut right_block_context,
            &left_assigned_var_ids.keys().copied().collect(),
            artifacts,
            false,
        );
    }

    let result_type: TUnion;

    if lhs_type.is_always_truthy() {
        // true || x → true (no fix)
        report_redundant_logical_operation(context, binary, "always true", "not evaluated", "`true`", None);
        result_type = get_true();
        right_block_context.flags.set_has_returned(true);
        binary.rhs.analyze(context, &mut right_block_context, artifacts)?;
    } else {
        if !negated_type_assertions.is_empty() {
            reconciler::reconcile_keyed_types(
                context,
                &negated_type_assertions,
                active_negated_type_assertions,
                &mut right_block_context,
                &mut changed_var_ids,
                &left_referenced_var_ids,
                &binary.lhs.span(),
                true,
                !block_context.flags.inside_negation(),
            );
        }

        right_block_context.clauses = clauses_for_right_analysis.iter().map(|v| Rc::new(v.clone())).collect();

        if !changed_var_ids.is_empty() {
            let partiioned_clauses =
                BlockContext::remove_reconciled_clause_refs(&right_block_context.clauses, &changed_var_ids);
            right_block_context.clauses = partiioned_clauses.0;
            right_block_context.reconciled_expression_clauses.extend(partiioned_clauses.1);
        }

        let pre_referenced_var_ids = right_block_context.conditionally_referenced_variable_ids.clone();
        right_block_context.conditionally_referenced_variable_ids.clear();

        let pre_assigned_var_ids = right_block_context.assigned_variable_ids.clone();
        right_block_context.assigned_variable_ids.clear();

        let tmp_if_body_context = right_block_context.if_body_context;
        right_block_context.if_body_context = None;

        binary.rhs.analyze(context, &mut right_block_context, artifacts)?;

        right_block_context.if_body_context = tmp_if_body_context;

        let rhs_type = match artifacts.get_rc_expression_type(&binary.rhs).cloned() {
            Some(rhs_type) => {
                check_logical_operand(context, binary.rhs, &rhs_type, "Right", "||");

                rhs_type
            }
            None => Rc::new(get_mixed()),
        };

        if lhs_type.is_always_falsy() {
            if rhs_type.is_always_falsy() {
                // false || false → false (no fix)
                report_redundant_logical_operation(context, binary, "always falsy", "always falsy", "`false`", None);
                result_type = get_false();
            } else if rhs_type.is_always_truthy() {
                // false || true → true (no fix)
                report_redundant_logical_operation(context, binary, "always falsy", "always truthy", "`true`", None);
                result_type = get_true();
            } else {
                // false || x → x (remove left, keep right)
                report_redundant_logical_operation(
                    context,
                    binary,
                    "always false",
                    "evaluated",
                    "the boolean value of the right-hand side",
                    Some(false), // remove left
                );

                result_type = get_bool();
            }
        } else if rhs_type.is_always_falsy() {
            // x || false → x (remove right, keep left)
            report_redundant_logical_operation(
                context,
                binary,
                "evaluated",
                "always falsy",
                "the boolean value of the left-hand side",
                Some(true), // remove right
            );

            result_type = get_bool();
        } else if rhs_type.is_always_truthy() {
            // x || true → true (no fix)
            report_redundant_logical_operation(context, binary, "evaluated", "always truthy", "`true`", None);

            result_type = get_true();
        } else {
            result_type = get_bool();
        }

        let mut right_referenced_var_ids = right_block_context.conditionally_referenced_variable_ids.clone();
        right_block_context.conditionally_referenced_variable_ids.extend(pre_referenced_var_ids);

        right_assigned_var_ids = right_block_context.assigned_variable_ids.clone();
        right_block_context.assigned_variable_ids.extend(pre_assigned_var_ids);

        let right_clauses = get_formula(
            binary.rhs.span(),
            binary.rhs.span(),
            binary.rhs,
            context.get_assertion_context_from_block(block_context),
            artifacts,
            &context.settings.algebra_thresholds(),
            context.settings.formula_size_threshold,
        )
        .unwrap_or_default();
        right_redefined_var_ids.extend(right_clauses.iter().flat_map(|clause| clause.redefined_vars.iter().copied()));
        collect_asserted_assignment_ids(context, &right_block_context, binary.rhs, &mut right_redefined_var_ids);

        let mut clauses_for_right_analysis = BlockContext::remove_reconciled_clauses(
            &clauses_for_right_analysis,
            &right_assigned_var_ids.keys().copied().collect::<WordSet>(),
        )
        .0;

        clauses_for_right_analysis.extend(right_clauses);

        let combined_right_clauses =
            saturate_clauses(clauses_for_right_analysis.iter(), &context.settings.algebra_thresholds());

        let (right_type_assertions, active_right_type_assertions) = find_satisfying_assignments(
            combined_right_clauses.as_slice(),
            Some(binary.rhs.span()),
            &mut right_referenced_var_ids,
        );

        if !right_type_assertions.is_empty() && !operand_contains_assignment(binary.rhs) {
            let mut right_changed_var_ids = WordSet::default();

            reconciler::reconcile_keyed_types(
                context,
                &right_type_assertions,
                active_right_type_assertions,
                &mut right_block_context.clone(),
                &mut right_changed_var_ids,
                &right_referenced_var_ids,
                &binary.rhs.span(),
                !binary.operator.span().is_zero(),
                block_context.flags.inside_negation(),
            );
        }

        block_context
            .conditionally_referenced_variable_ids
            .extend(right_block_context.conditionally_referenced_variable_ids.clone());
        block_context.assigned_variable_ids.extend(right_block_context.assigned_variable_ids.clone());

        // Statement-level `A || B;`: a variable assigned inside B holds either
        // its B-assigned type (A was falsy) or its left-context type narrowed
        // by `A` being truthy (B never ran). Merge both worlds for variables
        // the RHS assigned.
        if block_context.if_body_context.is_none() {
            let rhs_merged_vars: Vec<_> = right_block_context
                .locals
                .keys()
                .filter(|var_id| {
                    right_assigned_var_ids.contains_key(*var_id) && !left_assigned_var_ids.contains_key(*var_id)
                })
                .copied()
                .collect();

            let truthy_left_locals = if rhs_merged_vars.is_empty() {
                None
            } else {
                saved_left_clauses.map(|left_clauses_for_narrowing| {
                    let all_left_clauses = saturate_clauses(
                        block_context.clauses.iter().map(|v| &**v).chain(left_clauses_for_narrowing.iter()),
                        &context.settings.algebra_thresholds(),
                    );

                    let (truthy_assertions, _) =
                        find_satisfying_assignments(all_left_clauses.as_slice(), None, &mut WordSet::default());

                    let mut truthy_context = left_block_context.clone();
                    if !truthy_assertions.is_empty() {
                        reconciler::reconcile_keyed_types(
                            context,
                            &truthy_assertions,
                            IndexMap::new(),
                            &mut truthy_context,
                            &mut WordSet::default(),
                            &WordSet::default(),
                            &binary.lhs.span(),
                            false,
                            false,
                        );
                    }

                    truthy_context.locals
                })
            };

            for var_id in rhs_merged_vars {
                let Some(right_type) = right_block_context.locals.get(&var_id) else {
                    continue;
                };

                let left_type = truthy_left_locals
                    .as_ref()
                    .and_then(|locals| locals.get(&var_id))
                    .or_else(|| left_block_context.locals.get(&var_id));

                match left_type {
                    Some(left_type) => {
                        block_context.locals.insert(
                            var_id,
                            combine_union_types_rc(
                                right_type,
                                left_type,
                                context.codebase,
                                context.settings.combiner_options(),
                            ),
                        );
                    }
                    None => {
                        // Unknown before the statement: it is only defined when
                        // the RHS actually ran.
                        block_context.variables_possibly_in_scope.insert(var_id);
                    }
                }
            }
        }
    }

    // A variable created only on the right side of `||` is not generally in
    // scope afterwards. The exception needed by a following false/else path
    // is an assignment whose value itself participates in that path's
    // formula: if the whole OR is false, the RHS necessarily ran and the
    // assignment-derived assertion can safely reconcile its value.
    for var_id in right_redefined_var_ids {
        if let Some(assignment_offset) = right_assigned_var_ids.get(&var_id)
            && let Some(right_type) = right_block_context.locals.get(&var_id)
        {
            block_context.locals.entry(var_id).or_insert_with(|| Rc::clone(right_type));
            block_context.possibly_assigned_variable_ids.insert(var_id);
            block_context.assigned_variable_ids.insert(var_id, *assignment_offset);
        }
    }

    if let Some(if_body_context) = &block_context.if_body_context {
        let mut if_body_context_inner = if_body_context.borrow_mut();
        let left_vars = left_block_context.locals.clone();
        let if_vars = if_body_context_inner.locals.clone();
        for (var_id, right_type) in right_block_context.locals.clone() {
            if let Some(if_type) = if_vars.get(&var_id) {
                if_body_context_inner.locals.insert(
                    var_id,
                    combine_union_types_rc(&right_type, if_type, context.codebase, context.settings.combiner_options()),
                );
            } else if let Some(left_type) = left_vars.get(&var_id) {
                if_body_context_inner.locals.insert(
                    var_id,
                    combine_union_types_rc(
                        &right_type,
                        left_type,
                        context.codebase,
                        context.settings.combiner_options(),
                    ),
                );
            } else {
                // variable doesn't appear in the if body or left branch; nothing to merge in
            }
        }

        if_body_context_inner
            .conditionally_referenced_variable_ids
            .extend(block_context.conditionally_referenced_variable_ids.iter().copied());
        if_body_context_inner
            .assigned_variable_ids
            .extend(block_context.assigned_variable_ids.iter().map(|(k, v)| (*k, *v)));
    }

    artifacts.set_expression_type(binary, result_type);

    Ok(())
}

/// Analyzes the logical XOR operator (`xor`).
///
/// The `xor` operator evaluates both operands and returns `true` if exactly one of them is truthy,
/// and `false` otherwise. The result type is always `bool`.
/// This function analyzes both operands, checks for problematic types in a boolean context,
/// determines if the result can be statically known, and sets up data flow.
pub fn analyze_logical_xor_operation<'ctx, 'arena, A>(
    binary: &Binary<'arena>,
    context: &mut Context<'ctx, 'arena, A>,
    block_context: &mut BlockContext<'ctx>,
    artifacts: &mut AnalysisArtifacts,
) -> Result<(), AnalysisError>
where
    A: Arena,
{
    binary.lhs.analyze(context, block_context, artifacts)?;
    binary.rhs.analyze(context, block_context, artifacts)?;

    let fallback_type = Rc::new(get_mixed());
    let lhs_type = artifacts.get_rc_expression_type(&binary.lhs).unwrap_or(&fallback_type);
    let rhs_type = artifacts.get_rc_expression_type(&binary.rhs).unwrap_or(&fallback_type);

    check_logical_operand(context, binary.lhs, lhs_type, "Left", "xor");
    check_logical_operand(context, binary.rhs, rhs_type, "Right", "xor");

    let result_type = if lhs_type.is_always_truthy() && rhs_type.is_always_truthy() {
        if !block_context.flags.inside_loop_expressions() {
            // true xor true → false (no fix)
            report_redundant_logical_operation(context, binary, "always true", "always true", "`false`", None);
        }

        get_false()
    } else if lhs_type.is_always_truthy() && rhs_type.is_always_falsy() {
        if !block_context.flags.inside_loop_expressions() {
            // true xor false → true (no fix)
            report_redundant_logical_operation(context, binary, "always true", "always false", "`true`", None);
        }

        get_true()
    } else if lhs_type.is_always_falsy() && rhs_type.is_always_truthy() {
        if !block_context.flags.inside_loop_expressions() {
            // false xor true → true (no fix)
            report_redundant_logical_operation(context, binary, "always false", "always true", "`true`", None);
        }

        get_true()
    } else if lhs_type.is_always_falsy() && rhs_type.is_always_falsy() {
        if !block_context.flags.inside_loop_expressions() {
            // false xor false → false (no fix)
            report_redundant_logical_operation(context, binary, "always false", "always false", "`false`", None);
        }

        get_false()
    } else {
        get_bool()
    };

    artifacts.expression_types.insert(get_expression_range(binary), Rc::new(result_type));

    Ok(())
}

/// Checks a single operand of a logical operation (like AND, OR, XOR) for problematic types.
/// Reports errors for `mixed` and warnings for types that PHP coerces to boolean
/// (e.g., `null`, `array`, `resource`, `object`).
fn check_logical_operand<'arena, A>(
    context: &mut Context<'_, 'arena, A>,
    operand: &Expression<'arena>,
    operand_type: &TUnion,
    side: &'static str,
    operator_name: &'static str,
) where
    A: Arena,
{
    if operand_type.is_mixed() {
        context.collector.report_with_code(
            IssueCode::MixedOperand,
            Issue::error(format!("{side} operand in `{operator_name}` operation has `mixed` type."))
                .with_annotation(Annotation::primary(operand.span()).with_message("This has type `mixed`"))
                .with_note(format!(
                    "Using `mixed` in a boolean context like `{operator_name}` is unsafe as its truthiness is unknown."
                ))
                .with_help("Ensure this operand has a known type or explicitly cast to `bool`."),
        );
    } else if operand_type.is_null() {
        context.collector.report_with_code(
            IssueCode::NullOperand,
            Issue::warning(format!(
                "{side} operand in `{operator_name}` operation is `null`, which coerces to `false`."
            ))
            .with_annotation(Annotation::primary(operand.span()).with_message("This is `null` (coerces to `false`)"))
            .with_help("Explicitly check for `null` or cast to `bool` if this coercion is not intended."),
        );
    } else if operand_type.is_array() {
        if !context.settings.allow_array_truthy_operand {
            context.collector.report_with_code(
                IssueCode::InvalidOperand,
                Issue::warning(format!("{side} operand in `{operator_name}` operation is an `array`."))
                    .with_annotation(Annotation::primary(operand.span()).with_message("This is an `array`"))
                    .with_note(
                        "Arrays coerce to `false` if empty, `true` if non-empty. This implicit conversion can be unclear.",
                    )
                    .with_help("Consider using `empty()` or `count()` for explicit checks, or cast to `bool`."),
            );
        }
    } else if operand_type.is_objecty() {
        context.collector.report_with_code(
            IssueCode::InvalidOperand,
            Issue::warning(format!("{side} operand in `{operator_name}` operation is an `object`."))
                .with_annotation(Annotation::primary(operand.span()).with_message("This is an `object`"))
                .with_note(
                    "Objects generally coerce to `true` in boolean contexts. Ensure this is the intended behavior.",
                )
                .with_help("If specific truthiness is required, implement a method on the object or cast explicitly."),
        );
    } else if operand_type.is_resource() {
        context.collector.report_with_code(
            IssueCode::InvalidOperand,
            Issue::warning(format!("{side} operand in `{operator_name}` operation is a `resource`."))
                .with_annotation(Annotation::primary(operand.span()).with_message("This is a `resource`"))
                .with_note("Resources generally coerce to `true`. This implicit conversion can be unclear.")
                .with_help("Explicitly check the state of the resource or cast to `bool` if necessary."),
        );
    } else {
        // operand has a clean boolean coercion; no diagnostic needed
    }
}

/// Helper to report redundant logical operation issues.
///
/// * `side_to_remove`: Controls fix generation:
///   - `None`: No fix offered (just report the issue)
///   - `Some(false)`: Remove left operand, keep right operand
///   - `Some(true)`: Remove right operand, keep left operand
fn report_redundant_logical_operation<'arena, A>(
    context: &mut Context<'_, 'arena, A>,
    binary: &Binary<'arena>,
    lhs_description: &str,
    rhs_description: &str,
    result_value_str: &str,
    side_to_remove: Option<bool>,
) where
    A: Arena,
{
    let operator_span = binary.operator.span();
    if operator_span.is_zero() {
        // Do not report issues for synthetic nodes.
        return;
    }

    if let Some(remove_right) = side_to_remove
        && expression_has_observable_side_effect(if remove_right { binary.rhs } else { binary.lhs })
    {
        return;
    }

    // An operand judged constant-truthy/falsy that performs an assignment is
    // there for its side effect: `$cond && $x = 1;` and `$cond || $x = 1;` are
    // idiomatic conditional assignments, not redundant logic.
    let lhs_is_judged_constant = lhs_description.contains("always");
    let rhs_is_judged_constant = rhs_description.contains("always");
    if (lhs_is_judged_constant && operand_contains_assignment(binary.lhs))
        || (rhs_is_judged_constant && operand_contains_assignment(binary.rhs))
    {
        return;
    }

    let issue = Issue::help(format!(
        "Redundant `{}` operation: left operand is {} and right operand is {}.",
        BytesDisplay(binary.operator.as_bytes()),
        lhs_description,
        rhs_description
    ))
    .with_annotation(Annotation::primary(binary.lhs.span()).with_message(format!("Left operand is {lhs_description}")))
    .with_annotation(
        Annotation::secondary(binary.rhs.span()).with_message(format!("Right operand is {rhs_description}")),
    )
    .with_note(format!(
        "The `{}` operator will always return {} in this case.",
        BytesDisplay(binary.operator.as_bytes()),
        result_value_str
    ))
    .with_help(if side_to_remove.is_some() {
        let kept_side = if side_to_remove == Some(true) { "left" } else { "right" };
        format!("Consider simplifying this expression to just the {kept_side} operand.")
    } else {
        format!("Consider simplifying this expression to {result_value_str}.")
    });

    if let Some(remove_right) = side_to_remove {
        let to_remove = if remove_right {
            binary.operator.span().join(binary.rhs.span())
        } else {
            binary.lhs.span().join(binary.operator.span())
        };

        context.collector.propose_with_code(IssueCode::RedundantLogicalOperation, issue, |edits| {
            edits.push(TextEdit::delete(to_remove).with_safety(Safety::PotentiallyUnsafe));
        });
    } else {
        context.collector.report_with_code(IssueCode::RedundantLogicalOperation, issue);
    }
}

/// Whether the operand is an assignment expression (possibly parenthesized),
/// e.g. the `($x = 1)` in `$cond && ($x = 1);`.
fn operand_contains_assignment(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Parenthesized(parenthesized) => operand_contains_assignment(parenthesized.expression),
        Expression::Assignment(_) => true,
        _ => false,
    }
}

/// Reconstructs the state seen by the next disjunct of a nested `||` chain.
/// Every preceding disjunct is known to be false on that path, so assignments
/// and false-branch refinements must be replayed in source order rather than
/// combined as if they described one immutable value.
fn replay_short_circuit_assignments<'ctx, 'arena, A>(
    context: &mut Context<'ctx, 'arena, A>,
    left: &Expression<'arena>,
    mut replay_context: BlockContext<'ctx>,
    right_context: &mut BlockContext<'ctx>,
    assigned_var_ids: &WordSet,
    artifacts: &AnalysisArtifacts,
    branch_is_truthy: bool,
) where
    A: Arena,
{
    let mut operands = Vec::new();
    flatten_logical_expressions(left, branch_is_truthy, &mut operands);
    let mut replayed_assignment_var_ids = WordSet::default();

    for operand in operands {
        apply_recorded_assignments(context, operand, artifacts, &mut replay_context, &mut replayed_assignment_var_ids);

        let Some(formula) = get_formula(
            operand.span(),
            operand.span(),
            operand,
            context.get_assertion_context_from_block(&replay_context),
            artifacts,
            &context.settings.algebra_thresholds(),
            context.settings.formula_size_threshold,
        ) else {
            continue;
        };

        let branch_formula = if branch_is_truthy {
            formula
        } else {
            negate_or_synthesize(
                formula,
                operand,
                context.get_assertion_context_from_block(&replay_context),
                artifacts,
                &context.settings.algebra_thresholds(),
                context.settings.formula_size_threshold,
            )
        };
        let clauses = saturate_clauses(
            replay_context.clauses.iter().map(|clause| &**clause).chain(branch_formula.iter()),
            &context.settings.algebra_thresholds(),
        );
        let (assertions, _) = find_satisfying_assignments(&clauses, None, &mut WordSet::default());
        if assertions.is_empty() {
            continue;
        }

        reconciler::reconcile_keyed_types(
            context,
            &assertions,
            IndexMap::new(),
            &mut replay_context,
            &mut WordSet::default(),
            &WordSet::default(),
            &operand.span(),
            false,
            false,
        );
    }

    for var_id in assigned_var_ids.intersection(&replayed_assignment_var_ids) {
        if let Some(final_type) = replay_context.locals.get(var_id) {
            right_context.locals.insert(*var_id, Rc::clone(final_type));
        }
    }
}

fn flatten_logical_expressions<'ast, 'arena>(
    expression: &'ast Expression<'arena>,
    flatten_and: bool,
    expressions: &mut Vec<&'ast Expression<'arena>>,
) {
    match expression {
        Expression::Parenthesized(parenthesized) => {
            flatten_logical_expressions(parenthesized.expression, flatten_and, expressions);
        }
        Expression::Binary(binary)
            if if flatten_and {
                matches!(binary.operator, BinaryOperator::And(_) | BinaryOperator::LowAnd(_))
            } else {
                matches!(binary.operator, BinaryOperator::Or(_) | BinaryOperator::LowOr(_))
            } =>
        {
            flatten_logical_expressions(binary.lhs, flatten_and, expressions);
            flatten_logical_expressions(binary.rhs, flatten_and, expressions);
        }
        _ => expressions.push(expression),
    }
}

fn apply_recorded_assignments<'ctx, A>(
    context: &Context<'ctx, '_, A>,
    expression: &Expression<'_>,
    artifacts: &AnalysisArtifacts,
    replay_context: &mut BlockContext<'ctx>,
    replayed_assignment_var_ids: &mut WordSet,
) where
    A: Arena,
{
    match expression {
        Expression::Parenthesized(parenthesized) => {
            apply_recorded_assignments(
                context,
                parenthesized.expression,
                artifacts,
                replay_context,
                replayed_assignment_var_ids,
            );
        }
        Expression::Assignment(assignment) => {
            apply_recorded_assignments(context, assignment.rhs, artifacts, replay_context, replayed_assignment_var_ids);
            let Some(var_id) = get_expression_id(
                assignment.lhs,
                replay_context.scope.get_class_like_name(),
                context.resolved_names,
                Some(context.codebase),
            ) else {
                return;
            };
            let Some(assigned_type) = artifacts.get_rc_expression_type(&assignment.rhs) else {
                return;
            };
            replay_context.locals.insert(var_id, Rc::clone(assigned_type));
            replayed_assignment_var_ids.insert(var_id);
        }
        Expression::Binary(binary) => {
            apply_recorded_assignments(context, binary.lhs, artifacts, replay_context, replayed_assignment_var_ids);
            apply_recorded_assignments(context, binary.rhs, artifacts, replay_context, replayed_assignment_var_ids);
        }
        Expression::UnaryPrefix(unary) => {
            apply_recorded_assignments(context, unary.operand, artifacts, replay_context, replayed_assignment_var_ids);
        }
        _ => {}
    }
}

/// Collects assignments whose value is itself tested by the surrounding
/// formula. Assignments nested only as call arguments are intentionally not
/// included: the call result, rather than the assigned value, controls that
/// branch.
fn collect_asserted_assignment_ids<A>(
    context: &Context<'_, '_, A>,
    block_context: &BlockContext<'_>,
    expression: &Expression<'_>,
    assigned_var_ids: &mut WordSet,
) where
    A: Arena,
{
    match expression {
        Expression::Parenthesized(parenthesized) => {
            collect_asserted_assignment_ids(context, block_context, parenthesized.expression, assigned_var_ids);
        }
        Expression::Assignment(assignment) => {
            if let Some(var_id) = get_expression_id(
                assignment.lhs,
                block_context.scope.get_class_like_name(),
                context.resolved_names,
                Some(context.codebase),
            ) {
                assigned_var_ids.insert(var_id);
            }
        }
        Expression::UnaryPrefix(unary) if unary.operator.is_not() => {
            collect_asserted_assignment_ids(context, block_context, unary.operand, assigned_var_ids);
        }
        Expression::Binary(binary) if binary.operator.is_logical() || binary.operator.is_equality() => {
            collect_asserted_assignment_ids(context, block_context, binary.lhs, assigned_var_ids);
            collect_asserted_assignment_ids(context, block_context, binary.rhs, assigned_var_ids);
        }
        _ => {}
    }
}

#[inline]
const fn is_logical_or_operation(expression: &Expression<'_>, max_nesting: usize) -> bool {
    if max_nesting == 0 {
        return true;
    }

    match expression {
        Expression::Parenthesized(p) => is_logical_or_operation(p.expression, max_nesting),
        Expression::Binary(b) => match b.operator {
            BinaryOperator::Or(_) | BinaryOperator::LowOr(_) => is_logical_or_operation(b.lhs, max_nesting - 1),
            _ => false,
        },
        _ => false,
    }
}

#[inline]
const fn is_logical_operator_expression(expression: &Expression<'_>, and: bool) -> bool {
    match expression {
        Expression::Parenthesized(p) => is_logical_operator_expression(p.expression, and),
        Expression::Binary(b) if and => matches!(b.operator, BinaryOperator::And(_) | BinaryOperator::LowAnd(_)),
        Expression::Binary(b) => matches!(b.operator, BinaryOperator::Or(_) | BinaryOperator::LowOr(_)),
        _ => false,
    }
}
