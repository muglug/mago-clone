# Pzoom analysis parity roadmap

This roadmap continues from the conditional-analysis checkpoint on
`agent/pzoom-conditional-analysis`. Work is deliberately split into stacked,
reviewable phases. Each phase must preserve Mago's native contracts rather
than treating every Pzoom behavior as authoritative.

## Common delivery gate

Every coherent slice should include:

1. a focused Pzoom baseline and exact before/after failing-test set;
2. a native regression for the generalized invariant;
3. the relevant crate unit tests and complete native analyzer suite;
4. the full 4,021-case imported Pzoom run before the phase is published;
5. a separate branch/commit when the next phase changes subsystem ownership.

## Phase 1: template substitution and conditional returns

Branch: `agent/pzoom-template-substitution`

Baseline: 359 passing and 126 failing out of 485 `Template/` cases.

| Cluster | Failing |
| --- | ---: |
| Class templates | 26 |
| Class-template inheritance | 24 |
| Conditional return types | 21 |
| Function templates | 15 |
| Class-string maps | 6 |
| Function class-string templates | 6 |
| Function template assertions | 6 |
| `properties-of` templates | 6 |
| Nested templates | 4 |
| Template covariance | 4 |
| Trait templates | 3 |
| `key-of` templates | 2 |
| Other template cases | 3 |

Planned slices:

1. Partition conditional subjects into matching, non-matching, and undecidable
   atomics. Refine the subject template/parameter before substituting each
   selected branch. This fixes unions such as `T = User|null` under
   `T is object` without leaking `null` into the true branch.
2. Move synthetic conditional inputs into one invocation binding layer:
   parameter variables and defaults, argument count, and configured PHP
   version constants. Parsing should produce semantic subjects rather than
   class-like placeholders.
3. Make conditional evaluation recursive inside callable returns, generic
   object parameters, arrays, and other nested unions instead of relying on
   final expansion to blindly union both branches.
4. Consolidate lower-bound selection and replacement across function calls,
   method calls, inheritance specialization, and trait application. Preserve
   defining entity, variance, and bound depth.
5. Resolve derived template types (`key-of`, `value-of`, `properties-of`, and
   indexed access) only after their target templates have concrete bounds.
6. Align conditional types used while checking function bodies and inherited
   signatures, not only types resolved at an invocation site.

Acceptance targets:

- all existing native template/conditional-return tests remain green;
- no regression in the 34 assignment-conditional or 81 type-algebra cases;
- reduce the 126-test template failure set in independently attributable
  slices; do not suppress parser or type errors in the harness.

Checkpoint A complete:

- template subset: 371 passing / 114 failing, from 359 / 126;
- complete corpus: 3,383 passing / 638 failing, from 3,371 / 650 on the
  identical 4,021-test phase base;
- 12 previously failing tests resolved and no newly failing tests;
- conditional evaluation now runs recursively in inferred template
  substitution with arm-local bounds;
- `$param`, `func_num_args()`, `PHP_MAJOR_VERSION`, and `PHP_VERSION_ID` are
  registered as generated function templates and bound once per invocation;
- unresolved declaration conditionals preserve a conservative arm envelope,
  including class-constant targets;
- generic child parameters compare through their constraints only for method
  signature contravariance, without weakening ordinary argument checks;
- native gates: 2,411 analyzer integrations, the complete Codex suite, and the
  complete PHPDoc syntax suite pass.

The remaining 114 template failures are retained as explicit backlog. The
largest groups are class-template inheritance/variance, callable inference,
derived template utility types, and assertion-driven standalone `@var` cases;
they do not block the conditional-substitution layer from serving as the base
for provider work.

## Phase 2: PDO and array/function providers

Start only after Phase 1 has a stable published checkpoint.

Planned slices:

1. Port mode-sensitive `PDOStatement::fetch()` and `fetchAll()` return types,
   including associative, numeric, both, column, class, object, key-pair,
   bound, lazy, and named modes.
2. Centralize failure-sentinel policy so provider results retain `false` or
   `null` for flow analysis while honoring explicit ignore metadata at the
   diagnostic boundary.
3. Port array-shape/list providers in groups: `array_column`; filter/map/reduce;
   merge/replace; splice/shift/unshift; pointer/key helpers; count and
   non-emptiness.
4. Preserve optional keys, listness, known counts, non-emptiness, literal keys,
   and by-reference mutations through every provider.

Primary baselines: 21 PDO cases, 40 `ArrayFunctionCall` cases, 20 array
access/assignment/key cases, and the provider-related portion of 44
`FunctionCall` failures.

Execution checkpoints on `agent/pzoom-providers`:

1. **PDO row modes.** Preserve `PDO::prepare()`'s declared `false` member but
   honor its internal ignored-falsable contract during member resolution; add
   mode-sensitive `fetch()` and `fetchAll()` results. Baseline: 0 / 20 focused
   cases passing. Target: 20 / 20 plus a native test that proves both the
   retained failure type and safe chained access.
2. **Structural array transforms.** Correct `array_merge`, `array_reverse`,
   `array_splice`, and list/key normalization. Record the exact
   delta within the 40 failing `ArrayFunctionCall` cases.
3. **Element transforms.** Complete `array_column`, `array_filter`, `array_map`,
   and `array_reduce`, including callback-derived element types and shape
   degradation rules.
4. **Mutation and pointer helpers.** Preserve by-reference list/shape changes
   for shift/unshift/sort/walk, and return precise `current`, `key`, `count`,
   first/last-key, and non-empty results.
5. **Provider gate.** Run the complete native analyzer suite and the full 4,021
   imported cases, then publish a stacked checkpoint before assertion work.

Phase 2 starting measurements:

- PDO mode subset: 0 passing / 20 failing;
- complete `ArrayFunctionCall` subset: 171 passing / 40 failing;
- complete corpus inherited from Phase 1: 3,383 passing / 638 failing.

Phase 2 checkpoint complete:

- PDO mode subset: 20 passing / 0 failing;
- complete `ArrayFunctionCall` subset: 204 passing / 7 failing;
- complete corpus: 3,441 passing / 580 failing;
- 58 previously failing tests resolved and no newly failing tests;
- mode-sensitive PDO fetch results retain the declared failure sentinel while
  honoring explicit ignored-falsable metadata at member access;
- array providers now preserve reindexing, optional shape entries, exact
  counts, non-emptiness, pointer sentinels, callback filtering, and
  by-reference mutations where PHP's contract permits it;
- `array_column()` accepts heterogeneous rows that PHP skips while retaining
  precise results for shapes, objects, and non-empty inputs;
- native gate: 2,413 analyzer integrations pass.

The seven remaining `ArrayFunctionCall` failures are not standalone provider
return gaps: three require callable-array specialization, two require Psalm
type-alias/list-shape support, one requires loop fixed-point refinement, and
one requires large literal-union key comparison. They remain assigned to the
phases that own those mechanics rather than being hidden by broader provider
types.

## Phase 3: assertion-contract propagation

Branch: `agent/pzoom-assertion-contracts`

Planned slices:

1. Normalize declared and inferred `assert`, `assert-if-true`, and
   `assert-if-false` contracts into access-path assertions.
2. Substitute argument, `$this`, `self`, `static`, and inherited template
   subjects at the call boundary.
3. Propagate assertions through nested properties/methods, references, magic
   properties, and immutable arguments with precise invalidation.
4. Apply the same assertion pipeline to `array_key_exists`, chained `isset`,
   `empty`, `ctype_*` ranges, and union/intersection refinements.

Primary baselines: 39 `AssertAnnotation` and 62 `TypeReconciliation` failures.

Post-provider baseline: 64 passing / 39 failing in `AssertAnnotation`, and
623 passing / 62 failing in `TypeReconciliation`.

Execution checkpoints:

1. **Post-provider baseline.** Recompute the complete `AssertAnnotation` and
   `TypeReconciliation` sets on the Phase 2 commit, classify failures by
   declaration parsing, subject mapping, reconciliation, and invalidation,
   and freeze the exact failure-name lists.
2. **Canonical contract model.** Normalize unconditional, true-branch, and
   false-branch contracts from docblocks and inferred bodies into the same
   access-path assertion representation; retain source and polarity for
   diagnostics.
3. **Call-boundary substitution.** Map assertion subjects through positional
   and named arguments, `$this`, `self`/`static`, inherited methods, and the
   template bounds produced by Phase 1 before reconciliation runs.
4. **Access paths and invalidation.** Propagate nested property and method
   assertions only while their roots remain stable; invalidate paths after
   by-reference calls, writes, escaping mutable arguments, or unstable
   dispatch.
5. **Built-in contracts.** Route `array_key_exists`, chained `isset`/`empty`,
   and `ctype_*` refinements through the same machinery, including union and
   intersection subjects.
6. **Assertion gate.** Add one native regression per generalized invariant,
   run the complete analyzer suite and 4,021-case corpus, and publish a stacked
   checkpoint before callable inference begins.

Checkpoint A (access-path subject mapping):

- match assertion subjects to their root parameter, then append property,
  method, static, or array-access suffixes to the actual argument expression;
- canonicalize mapped method-call segments before they enter the formula and
  reconciliation pipeline;
- `AssertAnnotation`: 74 passing / 29 failing, resolving 10 cases with no new
  failures;
- covered behaviors: nested and magic properties, true/false/unconditional
  contracts, immutable roots, and reference aliases.

Checkpoint B (canonical clause boundaries):

- store function-like contracts as CNF assertion sets instead of flattening
  every tag and union member into one vector;
- preserve union members as one OR clause while treating separate assertion
  tags as independent AND clauses through scanning, inheritance, template
  substitution, plugins, and callback providers;
- `AssertAnnotation`: 75 passing / 28 failing, resolving the multiple-contract
  case without regressing Checkpoint A.

Checkpoint C (references and method-result keys):

- propagate plain-variable reconciliation through the complete PHP reference
  graph in either direction;
- use canonical lowercase method identifiers for active assertion contracts,
  matching mapped `->method()` access paths;
- `AssertAnnotation`: 78 passing / 25 failing, resolving both direct-reference
  cases and nested method-result narrowing. The post-dominator variant after an
  early return remains a Phase 5 control-flow join rather than an assertion-key
  defect.

Checkpoint D (static-property subjects):

- represent `self::$property`, `static::$property`, and named-class static
  properties explicitly in the PHPDoc CST/HIR instead of rejecting them as
  malformed parameter subjects;
- lower and scan the target into the same canonical access-path contract model,
  resolving lexical/late-bound class subjects at the invocation boundary;
- `AssertAnnotation`: 84 passing / 19 failing, resolving all six static and
  inherited-static cases.

Checkpoint E (inferred and structurally reconciled contracts):

- infer unconditional postconditions from `if (condition) { throw/return; }`
  guards without alternate branches, because normal completion proves the
  condition false;
- compose the sound sides of boolean AND/OR predicates so a normal return can
  retain multiple interface requirements as independent clauses;
- cover null exclusion, namespaced classes, class methods, and single/multiple
  interface intersections;
- merge sequential keyed-array shape assertions, preserve list element types
  when asserting non-emptiness, and push iterable value assertions back through
  concrete traversable template mappings;
- `AssertAnnotation`: 92 passing / 11 failing. The remaining failures belong to
  standalone `@var` handling, derived enum/class templates, post-dominator
  control flow, plus one deliberate Mago diagnostic for invoking a contract
  whose unconditional assertion contradicts the argument type.

Phase 3 gate:

- full Pzoom corpus: 3,469 passing / 552 failing, resolving 28 of the Phase 2
  failures with zero newly failing cases;
- native analyzer suite: 2,414 passing / 0 failing;
- Codex, PHPDoc syntax, HIR, formatting, and diff checks pass;
- `TypeReconciliation` remains 623 passing / 62 failing, confirming the phase
  did not change unrelated conditional-flow behavior.

## Phase 4: callable inference

Branch: `agent/pzoom-callable-inference`

Planned slices:

1. Preserve generic callable signatures through first-class callables,
   callable arrays, invokable objects, named arguments, and closure creation.
2. Infer callable template bounds contravariantly from parameters and
   covariantly from returns, including partially applied pipelines.
3. Infer mutation-free method bodies for purity and effect analysis only.
   Repeated method calls remain independent evaluations unless PHP stores the
   result in a local variable.
4. Keep this separate from Pzoom's property-assignment retention policy.

Post-assertion baseline:

- `Callable/` plus `PureCallable/`: 103 passing / 19 failing;
- `Closure/`: 51 passing / 10 failing;
- `Closures/`: 2 passing / 2 failing;
- `MethodCall/`: 53 passing / 13 failing.

Execution checkpoints:

1. **Ownership baseline.** Freeze the four failure-name lists and separate
   callable signature loss from PHPDoc dialect, unavailable-symbol, DOM/magic
   member, and private-lookup failures owned by Phase 5.
2. **Callable target normalization.** Resolve callable arrays, invokable
   objects, `Closure::__invoke()`, class strings, `self`, and inherited
   first-class callables into one signature representation before comparison.
3. **Generic signature substitution.** Substitute receiver/class/function
   templates into callable parameters and returns, preserve named parameters,
   and infer omitted closure parameter types using contravariant inputs and
   covariant outputs.
4. **Higher-order composition.** Carry those specialized signatures through
   `array_map`/filter-like callbacks, returned closures, partially applied
   functions, and pipeline helpers without falling back to `mixed`.
5. **Phase gate.** Add native regressions per invariant, run all callable,
   closure, and method subsets, then the native suite and complete Pzoom corpus
   before publishing the stacked checkpoint.

Checkpoint A (callable target normalization):

- retain callable parameter names through PHPDoc, expansion, partial
  application, and `Closure::fromCallable()` so named invocation remains
  valid after specialization;
- normalize callable objects, direct `Closure::__invoke()`, callable arrays,
  and leading namespace separators without discarding their signatures;
- `Callable/` plus `PureCallable/`: 109 passing / 13 failing;
- `Closure/`: 55 passing / 6 failing, for ten resolved target-normalization
  cases across the two subsets with no newly failing cases.

Checkpoint B (generic and contextual callable inference):

- solve templates owned by first-class callable arguments from the receiving
  callable's parameter positions before binding the outer invocation's
  templates;
- pass contextual callable results into nested invocations before their
  arguments are analyzed, preserving generic filter/map/pipeline composition
  instead of collapsing returned closures to `mixed`;
- contextualize returned closure parameters from declared callable return
  contracts and narrow explicitly typed inline closure parameters when the
  actual callback input is a concrete object subtype;
- preserve named variadic callback types, accept dynamic and relative callable
  arrays, forward first-class callables through `call_user_func`, and infer
  purity for syntactically side-effect-free closure expressions;
- `Callable/` plus `PureCallable/`: 119 passing / 3 failing;
- `Closure/`: 56 passing / 5 failing;
- `Closures/`: 3 passing / 1 failing.

Phase 4 gate:

- full Pzoom corpus: 3,496 passing / 525 failing, resolving 27 of the Phase 3
  failures with zero newly failing non-memoization cases;
- the four ownership subsets moved from 209 passing / 44 failing to 231
  passing / 22 failing;
- analyzer-level memoization of repeated method-call results is deliberately
  excluded: separate calls are separate evaluations unless the PHP code stores
  the result in a local variable. This leaves 12 Pzoom cases unsupported by
  design rather than making purity or dispatch stability imply value identity;
- native analyzer suite: 2,415 passing / 0 failing; Codex: 199 passing / 0
  failing; formatting and diff checks pass;
- the remaining non-memoization focused residuals are assigned to Phase 5:
  PHPDoc aliases/dialect,
  inherited or magic member lookup, static/intersection comparison,
  by-reference closure flow, DOM metadata, and one PHP-version availability
  mismatch. The invalid non-static `C::method(...)` case remains an explicit
  Mago/Pzoom behavioral divergence because PHP rejects that static access.

## Phase 5: class/member resolution and remaining control flow

Branch: `agent/pzoom-member-control-flow`

Starting baseline: 3,496 passing / 525 failing. A deliberately broad ownership
cut contains 157 failures: 62 `TypeReconciliation`, 74 class/member/property/
trait/interface cases, 12 loops, 5 switches, and 4 match/try joins. The exact
list is frozen outside the repository for before/after comparison during each
checkpoint.

Execution checkpoints:

1. **Ownership and invariants.** Classify failures by lookup, visibility,
   specialization, metadata, fixed-point, and branch-join ownership. Keep the
   12 repeated-method-call memoization cases as explicit non-goals: separate
   calls remain separate evaluations unless user code stores the value.
2. **Canonical method lookup.** Return one candidate model containing the
   declaring class, appearing class, lexical lookup scope, visibility, and
   receiver template substitution. Use it consistently for inherited private
   methods, trait aliases, magic fallback, intersections, callable targets,
   and argument-count checks.
3. **Property and class-like contracts.** Apply the same declaring/appearing
   split to real, pseudo, magic, and mixin properties; then complete readonly
   and template variance, inherited constants, class aliases, and
   `self`/`parent`/`static` resolution through traits and intersections.
4. **Loop fixed points.** Represent entry, body, continue/back-edge, break, and
   normal exit contexts explicitly. Preserve non-emptiness inside `foreach`,
   invalidate by-reference writes on every back edge, and distinguish
   terminating infinite loops from loops with reachable exits.
5. **Switch, match, and exception joins.** Route `continue` to the correct
   enclosing switch/loop target, retain exhaustiveness and post-dominator
   facts across match arms, and merge try/catch/finally variables by reachable
   normal-completion paths rather than syntactic branches.
6. **Phase gate.** Add one native regression per generalized invariant, freeze
   every focused before/after set, then run the full native analyzer and Codex
   suites plus all 4,021 imported Pzoom cases before publication.

Initial checkpoint targets:

- inherited-private and magic method lookup must improve without allowing
  inaccessible real methods or weakening unknown-method diagnostics;
- property and class-like work must not rely on method-result identity;
- loop and branch work must preserve the clause/access-path model delivered by
  Phases 1 and 3 instead of adding reconciled-variable payloads to clauses.

Checkpoint A (canonical method candidates):

- resolve a private method from the current lexical class when the receiver is
  that class or a subclass, without adding private members to child inheritance
  maps or allowing the same access outside the declaring scope;
- defer missing-method decisions until every receiver atomic has been checked,
  reporting a warning for an ordinary union call when another runtime target
  is callable while retaining hard errors for partial-callable creation and
  unions with no valid target;
- treat a union target's argument-count mismatch as possible when another
  signature accepts the exact positional arity, retaining a warning for the
  narrower target and hard errors when every signature rejects the call;
- `MethodCall/`: 56 passing / 10 failing, from 53 / 13. The ten residuals are
  eight intentional repeated-call non-goals and two DOM metadata contracts;
- full corpus: 3,501 passing / 520 failing, resolving five cases with zero new
  failures;
- native analyzer: 348 unit and 2,415 integration tests pass.

Checkpoint B (property specialization and extension metadata):

- specialize inherited property contracts through the extending class's
  template map before checking native or PHPDoc invariance, including nested
  lists, generic objects, `class-string<T>`, and grandchild substitutions;
- allow a narrowed PHPDoc override only when the original parent contract uses
  a covariant template parameter or is explicitly `@readonly`; retain native
  `readonly` modifier invariance as a separate, tested PHP rule;
- preserve concrete `DOMNode::appendChild()` inputs in its result and refine
  `DOMElement::$attributes`/`$localName` without weakening the nullable base
  `DOMNode` contracts;
- `MethodCall/`: 58 passing / 8 failing. Every residual is an intentional
  repeated-call identity non-goal after removing analyzer memoization;
- `PropertyTypeInvariance/`: 9 passing / 0 failing, from 4 / 5;
- full corpus: 3,510 passing / 511 failing, resolving nine cases with zero new
  failures. Template-aware property localization additionally fixes
  `PropertyType/genericTypeFromPropertyMap` and `propertyMapHydration`;
- native analyzer: 348 unit and 2,416 integration tests pass; the complete
  Codex suite passes.

Checkpoint C (loop entry, back-edge, and exit contexts):

- recognize omitted and statically truthy `for`/`while` conditions as
  non-terminating unless a reachable break exists, so normal function exit is
  not synthesized after an infinite loop;
- mark an iterated array non-empty only in the `foreach` body, preserving that
  fact for direct variables and property paths without leaking it after a
  possibly-empty loop;
- record synthetic foreach/by-reference assignments in the same assignment
  bookkeeping as source assignments, so each iteration's key/value targets
  replace back-edge values before the body is reanalyzed;
- retain prior-iteration values only for self-referential pre-condition
  assignments such as `$value = next($value ?? $initial)`, rather than broadly
  carrying every condition assignment and destabilizing unrelated loop facts;
- count both switches and loops for PHP's numeric `continue` target while
  mapping the selected target back to the owning loop scope;
- prefer explicit Traversable template parameters for foreach element types,
  falling back to concrete `current()`/`key()` methods only for unparameterized
  iterators; SimpleXML metadata now exposes its non-null yielded element type;
- `Loop/`: 152 passing / 2 failing, from 142 / 12. The two residuals are a
  standalone `@var` trust-policy case and a generic array-offset policy case,
  not fixed-point mechanics;
- full corpus: 3,521 passing / 500 failing, resolving eleven cases with zero
  new failures, including `Php71/iterableArg` outside the focused loop set;
- native analyzer: 348 unit and 2,417 integration tests pass; the complete
  Codex suite passes.

Checkpoint D (match-derived subject reconciliation and branch joins):

- retain Mago's synthetic match subject as the identity of PHP's single
  subject evaluation, while also generating Pzoom-style variable-free
  equality clauses from the original derived subject;
- use the source view to reconcile `count($array) === 0` back to the array and
  `get_class($value) === Foo::class` back to the object, without treating a
  future function or method call as the same evaluation;
- negate the captured-value and source-relation views independently for later
  and default arms, so a throwing empty-count arm proves the default result is
  a non-empty array;
- `Match/`: 10 passing / 0 failing, from 8 / 2. No imported switch or try case
  remains in the Phase 5 failure set; numeric switch/loop continuation was
  completed in Checkpoint C, and existing exception arms already join only
  through reachable normal-completion contexts;
- full corpus: 3,523 passing / 498 failing, resolving exactly
  `Match/MatchWithCount` and `Match/getClassWithMethod` with zero new failures;
- native analyzer: 348 unit and 2,418 integration tests pass; Codex: 199
  passing / 0 failing; formatting and diff checks pass.

Phase 5 gate:

- full Pzoom corpus moved from 3,496 passing / 525 failing to 3,523 passing /
  498 failing: 27 independently identified failures resolved with no newly
  failing cases;
- class/member resolution now carries declaring, appearing, and lexical
  context through private and union candidates, while inherited property
  contracts specialize receiver templates before compatibility checks;
- loop fixed points distinguish entry, back-edge, break, and normal-exit facts,
  and match-derived conditions now use the same access-path formula mechanics
  as ordinary conditionals;
- clauses remain variable-free and analyzer method-result memoization remains
  excluded. The eight residual `MethodCall/` identity cases are intentional
  non-goals;
- native analyzer and Codex gates are green at the Checkpoint D counts above.

This phase is intentionally last because many current member/control-flow
failures are downstream symptoms of missing template substitution, provider
types, or assertion propagation.
