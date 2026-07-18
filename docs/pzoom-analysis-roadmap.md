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

## Phase 4: callable inference and method-result memoization

Planned slices:

1. Preserve generic callable signatures through first-class callables,
   callable arrays, invokable objects, named arguments, and closure creation.
2. Infer callable template bounds contravariantly from parameters and
   covariantly from returns, including partially applied pipelines.
3. Infer mutation-free method bodies and memoize method-call results only when
   dispatch is stable (private, final, or otherwise non-overridable).
4. Keep this separate from Pzoom's property-assignment retention policy.

Primary baseline: 18 `Callable`, 10 `Closure`, and the non-property subset of
33 `MethodCall` failures.

## Phase 5: class/member resolution and remaining control flow

Planned slices:

1. Unify member lookup across inherited private methods, trait aliases, magic
   members, intersections, class strings, and enum/interface relationships.
2. Complete property variance, inherited constant, mixin, and implementation
   requirement handling.
3. Close the remaining foreach/loop fixed-point gaps for non-emptiness,
   by-reference mutation, termination, and switch/continue interaction.
4. Finish match and try/catch joins using the explicit branch invariants from
   the conditional-analysis phase.

This phase is intentionally last because many current member/control-flow
failures are downstream symptoms of missing template substitution, provider
types, or assertion propagation.
