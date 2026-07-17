# Pzoom conditional-analysis migration

Compared revisions:

- Mago clone: conditional-analysis branch based on `ddf48480`
- Pzoom: `c8162978`

## Conclusion

Pzoom's assignment-aware conditional analysis has been migrated into Mago's
existing formula engine. The implementation is semantic rather than a file
replacement: Mago retains its thresholds, optimized saturation, richer
context, and Mago-specific branch handling.

The highest-value missing primitive was assignment provenance on CNF clauses.
Pzoom records which variable a condition redefined, while Mago previously
reduced an assignment expression to the assigned variable's ordinary access
path. After that reduction Mago could not distinguish:

```text
$value is string       // fact about the value before an assignment
$value = new O         // replaces the value
$value is not null     // fact about the value after the assignment
```

The migration adds that missing temporal information and consumes it at the
same source-clause boundary as Pzoom. Synthesized and negated clauses do not
inherit assignment provenance, because doing so would incorrectly treat a
short-circuited assignment as executed.

## Implemented result

The migration includes:

- assignment provenance and assignment-result types on source CNF clauses;
- replacement of stale pre-assignment truths during truth extraction;
- directional disjunction that drops facts invalidated by a later assignment;
- ordered `&&` and `||` assignment replay, limited to syntactic assignments so
  by-reference mutations retain Mago's existing invalidation behavior;
- precise propagation of assignments whose value controls an `||` branch;
- a bounded fixed-point saturation pass;
- native regressions for nested logical assignments, saved boolean conditions,
  by-reference mutation, and assignments defined on only one path.

## Behavioral evidence

The imported checkpoint contains 4,021 Pzoom should-pass cases. With Mago's
normal error-level failure threshold, the result changed as follows:

| Corpus | Passing | Failing |
| --- | ---: | ---: |
| Before migration | 3,359 | 662 |
| After migration | 3,363 | 658 |

The exact failure-set difference contains the four intended fixes and no new
failures. Mago's complete native analyzer suite also passes: 2,411 passed and
0 failed.

The two most focused suites show the gap directly:

| Suite | Before | After | Total |
| --- | ---: | ---: | ---: |
| `TypeReconciliation/AssignmentInConditional` | 30 pass / 4 fail | 34 pass / 0 fail | 34 |
| `TypeReconciliation/TypeAlgebra` | 76 pass / 5 fail | 76 pass / 5 fail | 81 |

The four assignment failures fixed by this migration were:

- `assertHardConditionalWithString`
- `assertVarRedefinedInOpWithAnd`
- `assertVarRedefinedInOpWithOr`
- `maintainTruthinessInsideAssignment`

They exercise assignments nested inside `&&`, `||`, negation, and a null
comparison. Mago now passes them using clause redefinition provenance and
ordered short-circuit replay. The five remaining TypeAlgebra failures include
branch-join defects, but some require separate assertion-provider work (for
example, `get_class()` comparison narrowing), so 53 is an impact signal rather
than an expected one-change reduction.

## Architecture comparison

| Layer | Pzoom | Mago | Port decision |
| --- | --- | --- | --- |
| Clause representation | Carries `redefined_vars` in addition to assertions | Now carries assignment provenance and the assigned value type | Implemented with provenance-aware hashing |
| Truth extraction | A redefinition replaces earlier truths for that variable | Now resets stale truths, restores the assignment-result type, then applies the path assertion | Implemented |
| CNF disjunction | Drops left/pre-assignment facts when the right clause redefines the variable | Now applies the same directional source-clause rule | Implemented without leaking provenance into synthesized clauses |
| `&&` / `||` analysis | Uses cloned operand contexts and ordered assignment replay | Retains mature cloned contexts and now replays syntactic assignments in source order | Implemented |
| `if` / `elseif` joins | Tracks removed, redefined, negatable, and conditionally assigned variables explicitly | Tracks richer Mago state, but some facts are reconstructed late | Port selected join invariants, not the file |
| Formula engine | Psalm-oriented and less configurable | Retains thresholds, nullsafe clauses, disjunctive equality support, and optimized saturation; now iterates to a bounded fixed point | Kept and extended |
| Analyzer context | Pzoom-specific types and string-based variable IDs | Arena-aware Mago types, interned `Word` IDs, plugins, data flow, references, initialization, and symbol-existence state | Keep Mago's context |

The core data structures already correspond closely: both analyzers have a
`Clause`, `BlockContext`, `IfConditionalScope`, `IfScope`, formula generation,
truth extraction, a reconciler, and separate logical/statement analysis. The
main adaptation work is converting Pzoom's `VarName`/`TUnion` APIs to Mago's
`Word`/Codex APIs and preserving Mago's additional context fields.

## Implementation sequence

### 1. Add clause assignment provenance — implemented

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

### 2. Consolidate short-circuit state transitions — implemented

Refactor `crates/analyzer/src/expression/binary/logical.rs` around explicit
left-truthy, left-falsy/right, and post-expression states. Use the clause
provenance from step 1 and an ordered replay helper backed by Mago's existing
expression-type artifacts.

The acceptance gate for this step is all 34
`AssignmentInConditional` cases plus Mago's native logical-expression tests.

### 3. Port selected branch-join invariants — follow-up

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

### 4. Extend the same state model to loops and switches — follow-up

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

The highest-value assignment-aware slice is now implemented. The remaining
TypeAlgebra and broader conditional failures should be handled as follow-up
changes, continuing to use focused Pzoom cases plus the full native and
imported suites as acceptance gates.
