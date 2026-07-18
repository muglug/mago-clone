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

## Phase 3: assertion-contract propagation

Planned slices:

1. Normalize declared and inferred `assert`, `assert-if-true`, and
   `assert-if-false` contracts into access-path assertions.
2. Substitute argument, `$this`, `self`, `static`, and inherited template
   subjects at the call boundary.
3. Propagate assertions through nested properties/methods, references, magic
   properties, and immutable arguments with precise invalidation.
4. Apply the same assertion pipeline to `array_key_exists`, chained `isset`,
   `empty`, `ctype_*` ranges, and union/intersection refinements.

Primary baselines: 39 `AssertAnnotation` and 63 `TypeReconciliation` failures.

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
