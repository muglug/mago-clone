# pzoom "should pass" corpus

Integration tests ported from [pzoom](https://github.com/muglug/pzoom)'s
`tests/inference` suite (itself ported from Psalm's test suite). Only tests
**without** an `output.txt` were ported — i.e. tests for which Psalm reports no
issues — so Mago is expected to report no issues for them either.

Each test directory contains:

- `input.php` — the PHP code to analyze
- `php_version.txt` (optional) — PHP version pin; the harness default is 8.0,
  matching pzoom's test runner
- `error_levels.json` (optional) — Psalm issue kinds the original test
  suppresses via config; the harness drops Mago issues that map to them

Not ported from the source corpus:

- tests with an `output.txt` (expected-issue tests)
- the `Taint/` suite (taint-mode-only assertions; Psalm ignores all non-taint
  issues there, which regular analysis mode cannot reproduce)
- tests with pzoom config-marker files (`enable_phpunit_plugin`,
  `ensure_array_int_offsets_exist`, `ensure_array_string_offsets_exist`)
- tests skipped in pzoom itself (`SKIPPED-` prefix)
- tests using inline `@psalm-suppress` / `@phpstan-ignore` annotations (Mago
  does not honor Psalm suppressions, and mapping them at file granularity is
  too imprecise to be trustworthy)
- tests exercising `class_exists()` / `method_exists()` / `function_exists()` /
  `defined()` / `Reflection*` and similar dynamic symbol-existence checks,
  where Psalm's phantom-class special cases are deliberate divergences from
  Mago's model
- tests whose `input.php` is truncated/unparsable in pzoom itself
  (`ReturnTypeProvider/Dirname`, `ReturnTypeProvider/Basename`,
  `TypeAnnotation/multilineTypeWithExtraSpace` artifacts)

Run with:

```sh
cargo test -p mago-analyzer --test pzoom
# or a subset:
cargo test -p mago-analyzer --test pzoom -- TypeReconciliation/
```

The harness lives in `../pzoom.rs`. It analyzes each `input.php` with settings
approximating Psalm's test defaults (no unused-code analysis outside the
`UnusedVariable/` and `UnusedCode/` suites, property-initialization checks on,
`#[Override]` enforcement only for `Override/`), and maps `error_levels.json`
entries onto Mago issue codes, since Mago does not honor Psalm suppressions
natively.
