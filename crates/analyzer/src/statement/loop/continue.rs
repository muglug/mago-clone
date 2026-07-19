use mago_allocator::Arena;
use std::rc::Rc;

use mago_word::WordSet;

use mago_codex::ttype::TType;
use mago_codex::ttype::add_optional_union_type_rc;
use mago_codex::ttype::combine_union_types;
use mago_codex::ttype::combiner::CombinerOptions;
use mago_reporting::Annotation;
use mago_reporting::Issue;
use mago_span::HasSpan;
use mago_syntax::cst::Continue;
use mago_syntax::cst::Expression;
use mago_syntax::cst::Literal;
use mago_syntax::cst::LiteralInteger;

use crate::analyzable::Analyzable;
use crate::artifacts::AnalysisArtifacts;
use crate::code::IssueCode;
use crate::context::Context;
use crate::context::block::BlockContext;
use crate::context::block::BreakContext;
use crate::context::scope::control_action::ControlAction;
use crate::error::AnalysisError;

impl<'ast, 'arena> Analyzable<'ast, 'arena> for Continue<'arena> {
    fn analyze<'ctx, A>(
        &'ast self,
        context: &mut Context<'ctx, 'arena, A>,
        block_context: &mut BlockContext<'ctx>,
        artifacts: &mut AnalysisArtifacts,
    ) -> Result<(), AnalysisError>
    where
        A: Arena,
    {
        let levels = match self.level.as_ref() {
            Some(expression) => {
                if let Expression::Literal(Literal::Integer(LiteralInteger { value: Some(literal_integer), .. })) =
                    expression
                {
                    *literal_integer
                } else {
                    expression.analyze(context, block_context, artifacts)?;

                    context.collector.report_with_code(
                        IssueCode::InvalidContinue,
                        Issue::error("Continue level must be an integer literal.").with_annotation(
                            Annotation::primary(expression.span()).with_message(format!(
                                "Expected an integer literal here, found an expression of type `{}`.",
                                artifacts
                                    .get_expression_type(expression)
                                    .map_or_else(|| "unknown".to_string(), |union| union.get_id().to_string())
                            )),
                        ),
                    );

                    1
                }
            }
            None => 1,
        };

        if levels < 1 {
            context.collector.report_with_code(
                IssueCode::InvalidContinue,
                Issue::error("Continue level must be greater than zero.").with_annotation(
                    Annotation::primary(self.level.as_ref().map_or_else(|| self.span(), HasSpan::span))
                        .with_message("This level does not identify an enclosing loop or switch."),
                ),
            );

            block_context.flags.set_has_returned(true);
            return Ok(());
        }

        let requested_levels = levels as usize;
        let actual_levels_available = block_context.break_types.len();
        if requested_levels > actual_levels_available {
            let issue = Issue::error(format!(
                "Cannot continue {levels} levels - only {actual_levels_available} enclosing loop{} or switch{} available.",
                if actual_levels_available == 1 { "" } else { "s" },
                if actual_levels_available == 1 { " is" } else { "es are" },
            ))
            .with_annotation(
                Annotation::primary(self.level.as_ref().map_or_else(|| self.span(), HasSpan::span)).with_message(
                    format!("Continue level must be less than or equal to {actual_levels_available}."),
                ),
            );

            context.collector.report_with_code(IssueCode::InvalidContinue, issue);
            block_context.flags.set_has_returned(true);
            return Ok(());
        }

        let target_break_context = &block_context.break_types[actual_levels_available - requested_levels];
        let target_is_switch = matches!(target_break_context, BreakContext::Switch);
        let target_loop_depth = block_context
            .break_types
            .iter()
            .rev()
            .take(requested_levels)
            .filter(|break_context| matches!(break_context, BreakContext::Loop))
            .count();

        // Switches count toward PHP's numeric continue level, but do not own a
        // LoopScope. A continue targeting a switch uses the nearest loop scope
        // only to communicate the case's LeaveSwitch action.
        let loop_scope_depth = target_loop_depth.max(1);
        let mut loop_scope_ref = artifacts.loop_scope.as_mut();
        for _ in 1..loop_scope_depth {
            loop_scope_ref = loop_scope_ref.and_then(|loop_scope| loop_scope.parent_loop.as_deref_mut());
        }

        let Some(loop_scope) = loop_scope_ref else {
            context.collector.report_with_code(
                IssueCode::InvalidContinue,
                Issue::error("Continue statement used outside of loop.").with_annotation(
                    Annotation::primary(self.span())
                        .with_message("Continue statement must be inside a loop.".to_string()),
                ),
            );

            block_context.flags.set_has_returned(true);

            return Ok(());
        };

        if target_is_switch {
            loop_scope.final_actions.insert(ControlAction::LeaveSwitch);
        } else {
            loop_scope.final_actions.insert(ControlAction::Continue);
        }

        let mut removed_var_ids = WordSet::default();
        let redefined_vars =
            block_context.get_redefined_locals(&loop_scope.parent_context_variables, false, &mut removed_var_ids);

        loop_scope.redefined_loop_variables.retain(|redefined_var, current_redefined_type| {
            match redefined_vars.get(redefined_var) {
                Some(outer_redefined_type) => {
                    *current_redefined_type = Rc::new(combine_union_types(
                        outer_redefined_type,
                        current_redefined_type,
                        context.codebase,
                        CombinerOptions::default(),
                    ));

                    true
                }
                None => false,
            }
        });

        for var_id in loop_scope.parent_context_variables.keys() {
            if !redefined_vars.contains_key(var_id)
                && let Some(current_type) = block_context.locals.get(var_id)
            {
                let combined = add_optional_union_type_rc(
                    current_type,
                    loop_scope.possibly_redefined_loop_variables.get(var_id).map(std::convert::AsRef::as_ref),
                    context.codebase,
                );

                loop_scope.possibly_redefined_loop_variables.insert(*var_id, combined);
            }
        }

        for (var_id, var_type) in redefined_vars {
            loop_scope.possibly_redefined_loop_variables.insert(
                var_id,
                match loop_scope.possibly_redefined_loop_variables.get(&var_id) {
                    Some(existing_type) => Rc::new(combine_union_types(
                        existing_type,
                        &var_type,
                        context.codebase,
                        CombinerOptions::default(),
                    )),
                    None => Rc::clone(&var_type),
                },
            );
        }

        if let Some(finally_scope) = block_context.finally_scope.clone() {
            let mut finally_scope = (*finally_scope).borrow_mut();
            for (var_id, var_type) in &block_context.locals {
                if let Some(finally_type) = finally_scope.locals.get_mut(var_id) {
                    *finally_type = Rc::new(combine_union_types(
                        finally_type,
                        var_type,
                        context.codebase,
                        CombinerOptions::default(),
                    ));
                } else {
                    finally_scope.locals.insert(*var_id, Rc::clone(var_type));
                }
            }
        }

        block_context.flags.set_has_returned(true);

        Ok(())
    }
}
