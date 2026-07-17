# Porting pzoom's conditional analysis to Mago — feasibility study

Status: in progress — the first migration rounds have landed on this branch.
Companion to the pzoom should-pass corpus in `crates/analyzer/tests/pzoom/`,
which is the measurement harness for this work (baseline when this study was
written: **868 of 4021 corpus tests failing**; after the rounds below:
**853 failing / 3168 passing**, zero regressions in the analyzer's own
2410-case suite).

Landed so far:

- Phase 3 flow fidelity: Psalm-faithful
  `getDefinitelyEvaluatedExpression{AfterIf,InsideIf}` (strip `=== true`,
  `&&`-only descent, `!` swap) and branch-intersection of
  `assigned_variable_ids` in `update_if_scope`.
- Phase 1 detectors: `get_class()`/`gettype()`/`get_debug_type()` comparisons
  (including `switch` subjects), cast comparisons, `$a === $b` intersection
  narrowing of both operands, ungated `in_array` via `InArray`/`NotInArray`,
  and `array_key_exists` key-of narrowing with loose int/string key coercion
  (plus Psalm-semantics fixes to InArray/NotInArray reconciliation).
- Phase 0 algebra: `Clause::redefined_vars` end-to-end (marked from
  assignments in leaf conditionals, superseding in
  `find_satisfying_assignments`, pre-assignment-fact dropping in
  `disjoin_clauses`), and Psalm's practical statement-level `&&`/`||` merge
  semantics for conditional assignments.

- Reconciler round: pzoom's per-assertion redundancy-emission gate
  (`should_emit_redundant_issue_for_unchanged_assertion`) on the shared
  `get_acceptable_type` reporting path; ancestor-walking template inference
  when narrowing a parameterless class assertion against an existing generic;
  post-assignment truths exempted from conflicting-clause invalidation in
  `&&` chains and from `find_expression_logic_issues` re-reporting (fixes
  `is_string($v) && (($v = new O) !== null) && $v->foo()`).

Still open from the plan: `IsLooselyEqual` assertion variants, inline-site
redundancy gating (the corpus's redundant/impossible FPs flow through
per-function trigger sites, not the shared path), docblock-provenance issue
splitting, `get_value_for_key` reach (method-call memo keys, ArrayAccess,
class-string-map), trait-`$this` handling, dependent-type atomics for
indirect `get_class`/`gettype` flows, pzoom's or-replay machinery for
conditionally-assigned vars in else branches
(`maintainTruthinessInsideAssignment`), `removed_var_ids`, and the
reference-constraint branch-carry.

## Verdict

**Yes, portable — but as an incremental graft, not a transplant.** The two
analyzers are siblings: both descend from Psalm's design (via Hakana-style
Rust ports), with matching architecture at every layer. pzoom's conditional
analysis is the more faithful Psalm port and demonstrably more precise in
assertion *finding* and several reconciliation paths; Mago's is ahead in a
handful of places of its own. A wholesale file swap is blocked by the type-IR
difference, but a phased function-by-function port of pzoom's behaviors into
Mago's IR is mechanical for most of the surface.

Three facts make this far more tractable than a typical cross-analyzer port:

1. **Shared AST.** pzoom is built directly on `mago-syntax` / `mago-span` /
   `mago-database` (git-pinned to this repo). Its assertion finder, formula
   generator, and every flow analyzer already walk *Mago's own CST*. The
   expression-walking logic — usually the hardest part of a port — needs no
   translation at all.
2. **Same architecture, same names.** Clause/CNF algebra, assertion enums
   (~95% identical variants), `reconcile_keyed_types`, `IfScope` /
   `IfConditionalScope`, `BlockContext` fields, `update_if_scope`,
   negated-clause pipeline, loop fixpoint — every pzoom component has a 1:1
   Mago counterpart with near-identical field lists.
3. **Both key inferred types by span offsets** (`expr_types` /
   `expression_types: HashMap<(u32,u32), Rc<TUnion>>`), so ported detectors
   can look up operand types the same way.

## The wall: type IR and plumbing

What prevents a copy-paste swap:

| Layer | pzoom | Mago |
|---|---|---|
| Atomics | flat `TAtomic::{TLiteralInt, TString, TNamedObject, TArray{is_list, is_sealed}, TTemplateParam, ...}` | nested `TAtomic::Scalar(TScalar::...)`, `TArray::{Keyed(TKeyedArray), List(TList)}`, `TAtomic::Object(TObject::...)` |
| Interning | `StrId` / `VarName` (strings) | `Word` (byte atoms) |
| Combinators | `combine_union_types`, `type_combiner::combine`, `TypeComparisonResult` | `add_union_type`, `combiner::combine`, `ComparisonResult` |
| Issue sink | `FunctionAnalysisData::add_issue`, Psalm issue kinds | `Context.collector.report_with_code`, Mago `IssueCode` |
| Flags | plain bools on `BlockContext` | `BlockContextFlags` bitflags |

Every type-constructing or type-matching site in ported code must be
translated arm-by-arm (large but rote). The *algorithms* carry over
unchanged.

Also note an output-contract difference in assertion finding: pzoom eagerly
produces `if_true` + `if_false` maps *and* clause vectors, with hand-written
asymmetric negations (e.g. positive-only `class_exists` narrowing); Mago's
finder is positive-only and derives the false-branch via `negate_formula`.
Porting detectors into Mago's positive-only contract works for nearly all of
them; the few genuinely asymmetric ones need the leaf contract extended or an
explicit if-false side-channel (Mago already has one:
`artifacts.if_true_assertions` / `if_false_assertions`).

## What pzoom has that Mago lacks (the port targets)

### Algebra layer (small, highest leverage)
- **`Clause::redefined_vars`** — Psalm's assignment-in-conditional machinery.
  Assertion keys prefixed `=` (from `($v = expr) === null` etc.) mark the
  clause as describing the var's *post-assignment* value;
  `get_truths_from_formula` then *replaces* rather than conjoins earlier
  truths, and `combine_ored_clauses` drops the other side's pre-assignment
  possibilities. Mago's `Clause` has no such field; it compensates with
  coarser mechanisms (entry-clause wedging in elseif analysis,
  `synthesize_branch_discriminator_clauses`) which could be retired once
  this exists. This is the root cause of the remaining
  `assertVarRedefinedInOpWith{And,Or}`-style corpus failures.
- pzoom's typed `ClauseKey::{Name, Range}` maps onto Mago's `Word` sentinel
  (`*start-end`) convention — no change needed, just be aware when porting.

### Assertion finder (pzoom: 4555 lines vs Mago: 2225 — the biggest gap)
Missing entirely in Mago (each is a self-contained pure detector):
- `get_class($x)` / `gettype($x)` / `get_debug_type($x)` comparisons and
  switches (requires dependent-type atomics, see below). Verified missing by
  probe: `get_class($x) === A::class` and `switch (gettype($x))` both
  false-positive today.
- Cast comparisons (`(string)$x === $x`).
- Loose `==`/`!=` semantics (`IsLooselyEqual` assertion variants; Mago
  collapses loose onto identical/truthy paths).
- `$a === $b` narrowing *both* operands to their intersection; `=== ""`
  empty-string refinement.
- `in_array` without the `strict === true`-literal gate, over any
  array-bearing union, with `NotInArray` on the false branch.
- `array_key_exists($k, $arr)` narrowing the **key** variable to the
  array's key-of union; negative-branch `ArrayKeyDoesNotExist` paths.
- `class_exists`/`interface_exists`/`trait_exists`/`enum_exists`
  positive-only narrowing to `class-string` (+ literal-name existence keys).
- Memoized no-arg method-call var ids (`$e->getPrevious()` as an assertable
  key) — requires extending `get_expression_id`, which currently returns
  `None` for calls.
- isset dynamic-key root walk-up (`IsEqualIsset` on the first resolvable
  prefix).

Dependent types: pzoom carries Psalm's `TDependentGetClass` /
`TDependentGetType` atomics so `$t = get_class($x)` can narrow `$x` later.
Mago needs an equivalent (a small `TAtomic` addition in `mago-codex`).

### Reconciler (both ~9-11k lines; targeted grafts, not replacement)
- Ancestor-walking template inference for `instanceof Generic` on an
  existing generic object (`infer_class_template_replacements_from_ancestors`).
- `class-string<T>` ∩ `class-string<U>` constraint-object intersection.
- `get_value_for_key` reach: resolving `$base->method()` memo keys to
  declared return types, `ArrayAccess::offsetGet`, `class-string-map`
  offsets (Mago handles only `[...]` and `->property`).
- Reporting semantics: tri-state `EmissionMode { Silent, ImpossibleOnly,
  All }` instead of a bool; `redundant_reconciled_vars` (suppress re-reports
  and drop clauses of a var already flagged redundant); docblock-provenance
  split (Psalm's `RedundantConditionGivenDocblockType` /
  `DocblockTypeContradiction` vs plain variants) — Mago's taxonomy is flat,
  which is part of why redundant/impossible FPs dominate the corpus
  failures (~140 tests across those buckets).
- Deterministic `BTreeMap` key ordering in `reconcile_keyed_types` (Mago
  uses caller insertion order; nested-key propagation is order-sensitive).
- Trait-body `$this` open-narrowing (assertions on `$this` in a trait keep
  it open rather than emptying it).
- `@psalm-inheritors` closed-set negation; DateTime/DateTimeImmutable
  interface-negation special case.

### Flow layer (small fidelity fixes)
- `getDefinitelyEvaluatedExpression{AfterIf,InsideIf}`: pzoom (faithful to
  Psalm) strips `=== true`, descends only `&&`-left for the after-variant,
  and swaps after/inside under `!`. Mago diverges on all three.
- `update_if_scope`: pzoom **intersects** `assigned_var_ids` across branches
  (Psalm-correct: "assigned in all branches"); Mago unions.
- `IfScope::removed_var_ids` tracking + the tail step dropping them from the
  outer context — absent in Mago.
- Reference-constraint branch-carry with `ConflictingReferenceConstraint`
  reporting (pzoom models constraints as `Vec<TUnion>` per var; Mago's
  single-constraint model would need extending).

## What Mago has that pzoom lacks (keep, don't regress)

A wholesale transplant would *lose* these; the graft approach keeps them:
- Statement-level `A && B;` / `A || B;` two-world merge (added on this
  branch; pzoom's statement-level `&&` actually discards RHS type effects).
- `TList::known_count` and finer count-based list sizing.
- Object-property back-propagation (`adjust_object_property_type`: narrowing
  `$obj->prop` filters the object union) — pzoom has no analogue.
- `method_exists`/`property_exists` → `TObject::HasMethod/HasProperty`
  intersection narrowing.
- Integer-range comparison narrowing (`TInteger::From/To/Range`,
  variable-vs-variable) — pzoom only handles single literals.
- Whole nullsafe-chain base clauses (every `?->` link asserts non-null).
- `ctype_digit/lower/upper`, `is_countable`, Psl `Iter\contains` handling.
- Psalm-faithful switch scope model (pzoom's switch deviates from Psalm),
  match exhaustiveness/unreachable-arm diagnostics, property-initialization
  intersection tracking through if/else, `active_method_call_assertions`.

## Recommended plan

Use the corpus harness as the meter for every phase
(`cargo test -p mago-analyzer --test pzoom -- --ignored`), with the existing
2410-case suite as the regression guard.

- **Phase 0 — algebra enablers (~1-2k lines, unlocks the rest).**
  `Clause::redefined_vars` + `mark_redefined` from `=`-keys, supersede logic
  in `find_satisfying_assignments`, drop-other-side in `disjoin_clauses`;
  `IsLooselyEqual/IsNotLooselyEqual` assertion variants; dependent-type
  atomics (`get_class`/`gettype` results) in `mago-codex`. Then retire
  `synthesize_branch_discriminator_clauses` if subsumed.
- **Phase 1 — assertion-finder detectors (~2-3k lines, biggest corpus win).**
  Port the missing detectors listed above one at a time as pure functions
  emitting Mago's positive-only assertion maps. Each lands independently
  with corpus deltas.
- **Phase 2 — reconciler grafts (~3-4k lines).** Reporting semantics first
  (EmissionMode, redundant_reconciled_vars, docblock provenance, BTreeMap
  ordering) since they target the dominant redundant/impossible FP buckets;
  then the precision features (ancestor template inference, class-string
  intersection, get_value_for_key reach, trait-$this).
- **Phase 3 — flow fidelity (~1k lines).** definitely-evaluated-expression
  fidelity, assigned-vars intersection, removed_var_ids, reference-constraint
  carry.

Estimated total: **~8-12k lines of translated/adapted code** across four
independently shippable milestones (pzoom's full conditional stack is ~23k
lines, but roughly half already exists in Mago in equivalent or better form).
Realistic corpus impact: 250-400 of the remaining 868 failures, concentrated
in TypeReconciliation, AssertAnnotation, Loop, and the
impossible/redundant-condition buckets, plus knock-on wins in mixed-* and
invalid-argument cascades that stem from lost narrowing.

The alternative — swapping Mago's analyzer for pzoom's wholesale — is not
recommended: it would require adopting `pzoom-code-info` as the type system
(abandoning `mago-codex`, the expander, comparators, and every non-conditional
analyzer built on them), and would regress the Mago-only strengths listed
above along with Mago's richer diagnostics and fixer integration.
