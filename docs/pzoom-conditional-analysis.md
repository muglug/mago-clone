# Pzoom conditional-analysis port assessment

Compared revisions:

- Mago clone: `2923585c`, plus the Pzoom-corpus checkpoint ending at `74881fac`
- Pzoom: `c8162978`

## Conclusion

Pzoom's conditional-analysis behavior can be ported to Mago, and the shared
Psalm/Hakana lineage makes this materially easier than a rewrite. The port
should be semantic and staged, however. Replacing Mago's conditional or
algebra files wholesale would discard Mago-specific correctness and
performance work.

The highest-value missing primitive is assignment provenance on CNF clauses.
Pzoom records which variable a condition redefined, while Mago currently
reduces an assignment expression to the assigned variable's ordinary access
path. After that reduction Mago cannot distinguish:

```text
$value is string       // fact about the value before an assignment
$value = new O         // replaces the value
$value is not null     // fact about the value after the assignment
```

Consequently, pre-assignment truths can be conjoined with post-assignment
truths or merged into the wrong short-circuit path. Several local repairs in
Mago's logical-expression analyzer compensate for individual shapes, but the
algebra still lacks the information needed for the general case.

## Behavioral evidence

The imported checkpoint contains 4,021 Pzoom should-pass cases. With Mago's
normal error-level failure threshold it currently reports:

- 3,359 passing
- 662 failing
- 74 failures in `TypeReconciliation`
- 53 failures in explicitly conditional/algebra/isset/redundancy/scope suites

The two most focused suites show the gap directly:

| Suite | Passing | Failing | Total |
| --- | ---: | ---: | ---: |
| `TypeReconciliation/AssignmentInConditional` | 30 | 4 | 34 |
| `TypeReconciliation/TypeAlgebra` | 76 | 5 | 81 |

The four assignment failures are:

- `assertHardConditionalWithString`
- `assertVarRedefinedInOpWithAnd`
- `assertVarRedefinedInOpWithOr`
- `maintainTruthinessInsideAssignment`

They exercise assignments nested inside `&&`, `||`, negation, and a null
comparison. Pzoom passes them using clause redefinition provenance and ordered
short-circuit replay. The five remaining TypeAlgebra failures also include
branch-join defects, but some require separate assertion-provider work (for
example, `get_class()` comparison narrowing), so 53 is an impact signal rather
than an expected one-change reduction.

## Architecture comparison

| Layer | Pzoom | Mago | Port decision |
| --- | --- | --- | --- |
| Clause representation | Carries `redefined_vars` in addition to assertions | Carries assertions and source spans, but no assignment provenance | Port the provenance concept |
| Truth extraction | A redefinition replaces earlier truths for that variable | Every unit truth is appended conjunctively | Port replacement semantics |
| CNF disjunction | Drops left/pre-assignment facts when the right clause redefines the variable | Unconditionally merges both possibility maps | Port the directional merge rule |
| `&&` / `||` analysis | Uses cloned operand contexts and ordered assignment replay | Has mature cloned contexts plus several shape-specific merges | Integrate provenance and replay into Mago's analyzer |
| `if` / `elseif` joins | Tracks removed, redefined, negatable, and conditionally assigned variables explicitly | Tracks richer Mago state, but some facts are reconstructed late | Port selected join invariants, not the file |
| Formula engine | Psalm-oriented and less configurable | Has thresholds, nullsafe clauses, disjunctive equality support, and optimized saturation | Keep Mago's engine |
| Analyzer context | Pzoom-specific types and string-based variable IDs | Arena-aware Mago types, interned `Word` IDs, plugins, data flow, references, initialization, and symbol-existence state | Keep Mago's context |

The core data structures already correspond closely: both analyzers have a
`Clause`, `BlockContext`, `IfConditionalScope`, `IfScope`, formula generation,
truth extraction, a reconciler, and separate logical/statement analysis. The
main adaptation work is converting Pzoom's `VarName`/`TUnion` APIs to Mago's
`Word`/Codex APIs and preserving Mago's additional context fields.

## Recommended implementation sequence

### 1. Add clause assignment provenance

Extend `mago_algebra::Clause` with an allocation-light set of redefined
variables. Mark assignment-derived clauses in Mago's formula generation rather
than encoding assignment state in a variable-name prefix.

Update:

- `crates/algebra/src/clause.rs`
- `crates/algebra/src/lib.rs`
- `crates/analyzer/src/formula.rs`
- the assignment-aware assertion/formula bridge

Required invariants:

- truth extraction replaces earlier facts when a clause redefines a variable;
- ordered disjunction omits stale facts from before the redefinition;
- clause removal/addition preserves provenance;
- equality, hashing, and deduplication do not collapse clauses whose provenance
  changes their meaning;
- negation has an explicit, tested provenance policy.

The hash/deduplication point is important: Pzoom does not include
`redefined_vars` in its clause hash. Mago should not copy that detail blindly,
because provenance changes algebra behavior.

### 2. Consolidate short-circuit state transitions

Refactor `crates/analyzer/src/expression/binary/logical.rs` around explicit
left-truthy, left-falsy/right, and post-expression states. Use the clause
provenance from step 1 and an ordered replay helper backed by Mago's existing
expression-type artifacts.

The acceptance gate for this step is all 34
`AssignmentInConditional` cases plus Mago's native logical-expression tests.

### 3. Port selected branch-join invariants

Adapt Pzoom's useful `if`/`elseif` behaviors into Mago's existing
`statement/if.rs` and scope structures:

- separate condition assignments from body assignments by presence, not count
  deltas;
- carry removed/redefined variables through every continuing branch;
- preserve the correct negated formula across `elseif` chains;
- replay conditionally assigned variables only on paths where the expression
  executes;
- keep active/reconciled assertion tracking precise enough to avoid duplicate
  paradox diagnostics.

Do not replace Mago's property-initialization, reference, symbol-existence,
branch-discriminator, or data-flow merges.

### 4. Extend the same state model to loops and switches

Only after `if`/`elseif` and logical expressions are stable, apply the shared
state-transition helpers to loop fixpoints, `continue`/`break`, and switch
joins. Run the native analyzer suite and the complete Pzoom corpus after each
stage to keep regressions attributable.

## Expected scope and risk

This is a cross-cutting refactor, but it does not require replacing Mago's type
system or reconciler. Expect changes across the algebra crate, formula and
assertion generation, logical-expression analysis, conditional scope, and
statement-level branch merging. The safest delivery is several reviewable PRs,
with clause provenance first; a single wholesale port would be difficult to
review and would carry a high regression risk.

The recommendation is therefore **yes: port it**, beginning with clause-level
assignment provenance and using the Pzoom cases as acceptance tests, while
retaining Mago's formula engine and richer analysis context.
