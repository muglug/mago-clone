# Pzoom conditional-analysis migration

Compared revisions:

- Mago clone: clause/scope refactor based on `4d4001fc`
- Pzoom: `c8162978`

## Conclusion

Pzoom's assignment-aware conditional analysis has been migrated into Mago's
existing formula engine. The final representation now follows Pzoom's source
contract directly rather than approximating it with Mago-only clause state:

- assertion extraction marks assignment-derived variable IDs;
- formula construction strips the marker and records only `redefined_vars`;
- the clause's ordinary assertion possibilities describe the assigned value;
- truth extraction replaces stale pre-assignment truths with those ordinary
  possibilities.

Mago retains its thresholds, optimized saturation, richer context, and
Mago-specific branch handling.

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
short-circuited assignment as executed. Clauses do not carry reconciled
variables or a parallel assigned-type map.

## Implemented result

The migration includes:

- assignment provenance on source CNF clauses, with assignment-result types
  encoded as ordinary assertions;
- removal of Mago's post-hoc assignment AST walker and clause-owned
  `redefined_var_types` compatibility layer;
- replacement of stale pre-assignment truths during truth extraction;
- directional disjunction that drops facts invalidated by a later assignment;
- ordered `&&` and `||` assignment replay, limited to syntactic assignments so
  by-reference mutations retain Mago's existing invalidation behavior;
- precise propagation of assignments whose value controls an `||` branch;
- a bounded fixed-point saturation pass;
- native regressions for nested logical assignments, saved boolean conditions,
  by-reference mutation, assignments defined on only one path, cumulative
  `elseif` negations, and `isset`-guarded loop mutations;
- exact-class narrowing for `get_class($object) === ClassName::class`;
- assertion extraction through loose comparisons with `false`;
- preservation of provisional loop-`isset` types until the loop fixed point
  supplies the concrete element type;
- final implicit-else joins measured against the original outer context;
- explicit if/switch scope sets for removed, negatable, possibly-in-scope, and
  definitely assigned variables, including intersection semantics across
  continuing branches.

## Behavioral evidence

The imported checkpoint contains 4,021 Pzoom should-pass cases. With Mago's
normal error-level failure threshold, the result changed as follows:

| Corpus | Passing | Failing |
| --- | ---: | ---: |
| Before migration | 3,359 | 662 |
| Assignment-provenance checkpoint | 3,363 | 658 |
| Completed conditional migration | 3,372 | 649 |
| Pzoom-shaped clause/scope refactor | 3,371 | 650 |

The completed migration removes nine additional failures from the
assignment-provenance checkpoint. The representation refactor preserves both
focused acceptance suites and every native Mago test. One full-corpus case,
`Template/ClassTemplate/doNotForgetAssertion`, changes because explicit branch
removal now exposes an existing policy difference: Mago deliberately forgets
object-property narrowings after a potentially mutating call, while Pzoom's
default `remember_property_assignments_after_call` behavior retains them. The
test calls an unannotated method with the narrowed object before returning the
property. Weakening Mago's call invalidation would make that case pass, but is
not part of the formula port and would contradict Mago's native
`clear_narrowed_prop_after_call` behavior.

The two most focused suites show the gap directly:

| Suite | Before | After | Total |
| --- | ---: | ---: | ---: |
| `TypeReconciliation/AssignmentInConditional` | 30 pass / 4 fail | 34 pass / 0 fail | 34 |
| `TypeReconciliation/TypeAlgebra` | 76 pass / 5 fail | 81 pass / 0 fail | 81 |

The four assignment failures fixed by this migration were:

- `assertHardConditionalWithString`
- `assertVarRedefinedInOpWithAnd`
- `assertVarRedefinedInOpWithOr`
- `maintainTruthinessInsideAssignment`

They exercise assignments nested inside `&&`, `||`, negation, and a null
comparison. Mago now passes them using clause redefinition provenance and
ordered short-circuit replay. The five TypeAlgebra follow-ups exercised exact
`get_class()` comparisons, negated assertions expressed as `== false`, an
`isset`-guarded loop mutation, and cumulative `elseif` branch state. They now
pass through reusable analyzer invariants rather than case-specific handling.

## Architecture comparison

| Layer | Pzoom | Mago | Port decision |
| --- | --- | --- | --- |
| Clause representation | Carries only assertions plus `redefined_vars` | Now carries the same; no reconciled-variable or assigned-type side map | Ported directly, with provenance-aware hashing |
| Assertion/formula boundary | Marks assignment-derived IDs with `=`, then strips the marker into clause provenance | Now uses the same source marker contract | Ported directly |
| Truth extraction | A redefinition replaces earlier truths with that clause's ordinary assertions | Now performs the same replacement | Ported directly |
| CNF disjunction | Drops left/pre-assignment facts when the right clause redefines the variable | Now applies the same directional source-clause rule | Implemented without leaking provenance into synthesized clauses |
| `&&` / `||` analysis | Uses cloned operand contexts and ordered assignment replay | Retains mature cloned contexts and now replays syntactic assignments in source order | Implemented |
| `if` / `elseif` joins | Tracks removed, redefined, negatable, and conditionally assigned variables explicitly | Now carries explicit removed and negatable sets and intersects definite assignments, while retaining Mago-only state | Ported through an adapter, not a file replacement |
| Switch joins | Tracks continuing-case state explicitly | Now carries removed/possibly-in-scope sets and intersects definite assignments across continuing cases | Ported through Mago's switch analyzer |
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
variables. Assignment extraction uses Pzoom's temporary variable-name prefix;
formula construction immediately strips it and marks the resulting source
clause. The marker is not stored in the algebra.

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

An earlier checkpoint also stored `redefined_var_types` on each clause and
discovered assignments with a post-hoc AST walker. The final refactor removes
both. For an assignment-tested-against-null expression, assertion extraction
emits the precise non-null atomic type when it is unique, otherwise the normal
`not null` assertion. That ordinary assertion is the only assigned-result type
consumed by the formula engine.

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

### 3. Port selected branch-join invariants — implemented

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

This part cannot be copied literally. Pzoom's `reasonable_clauses` continuation
merge and Mago's late branch update are arranged differently. In particular,
clearing the negatable-variable set whenever an `elseif` exists (as in Pzoom's
flow) loses five valid TypeAlgebra facts in Mago. Mago instead carries the
explicit set and extends it only with variables actually changed by each else
reconciliation. That is an adapter around the same invariant, not a different
clause model.

### 4. Extend the same state model to loops and switches — implemented where exposed

The loop fixed-point path now preserves the analyzer's provisional
`isset-from-loop` marker through array access and arithmetic. Empty-array
variants that cannot satisfy positive `isset` are discarded, allowing values
written on earlier iterations to determine the element type. Existing
`continue`/`break` behavior remains intact. Switch analysis now uses the same
explicit join invariants for removed variables, variables introduced on only
some paths, and definite assignments. The imported TypeAlgebra checkpoint
exposes no remaining formula gap in this stage.

## Expected scope and risk

This is a cross-cutting refactor, but it does not require replacing Mago's type
system or reconciler. Expect changes across the algebra crate, formula and
assertion generation, logical-expression analysis, conditional scope, and
statement-level branch merging. The safest delivery is several reviewable PRs,
with clause provenance first; a single wholesale port would be difficult to
review and would carry a high regression risk.

The planned conditional-analysis migration and representation refactor are
complete. Broader imported corpus failures remain in other analyzer areas, but
the two focused acceptance suites pass completely. The one corpus delta is the
documented post-call property-invalidation policy boundary, not clause-owned
reconciliation state.
