//! Runner for the pzoom (Psalm) "should pass" corpus under `tests/pzoom/`.
//!
//! Each test is a directory ported from pzoom's `tests/inference` suite that has
//! no `output.txt` — i.e. Psalm reports no issues for it — containing:
//!
//! - `input.php`: the PHP code to analyze
//! - `php_version.txt` (optional): PHP version pin (the harness default is 8.0,
//!   matching pzoom's test runner)
//! - `error_levels.json` (optional): Psalm issue kinds the original test
//!   suppresses via config
//!
//! The harness analyzes `input.php` with settings approximating Psalm's test
//! defaults and fails if any issue is reported, after dropping issues that map
//! to Psalm kinds suppressed by `error_levels.json` (Mago does not honor Psalm
//! suppressions natively, so the mapping lives here; tests relying on inline
//! `@psalm-suppress` are not ported at all — see `pzoom/README.md`).
//!
//! The corpus intentionally contains known-failing tests, so trials are
//! ignored by default (keeping `cargo test --workspace` green). Run with
//! `cargo test -p mago-analyzer --test pzoom -- --ignored`; add a path
//! fragment to filter, e.g. `-- --ignored Closure/`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::unwrap_in_result,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::use_debug,
    clippy::naive_bytecount,
    clippy::panic_in_result_fn
)]

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;

use foldhash::HashSet;
use libtest_mimic::Arguments;
use libtest_mimic::Failed;
use libtest_mimic::Trial;

use mago_allocator::LocalArena;
use mago_analyzer::Analyzer;
use mago_analyzer::analysis_result::AnalysisResult;
use mago_analyzer::plugin::PluginRegistry;
use mago_analyzer::settings::Settings;
use mago_codex::populator::populate_codebase;
use mago_codex::scanner::scan_program;
use mago_database::DatabaseReader;
use mago_database::file::File;
use mago_names::resolver::NameResolver;
use mago_php_version::PHPVersion;
use mago_prelude::Prelude;
use mago_reporting::Level;
use mago_syntax::parser::parse_file;
use mago_word::WordSet;

static PRELUDE: LazyLock<Prelude> = LazyLock::new(Prelude::build);
static PLUGIN_REGISTRY: LazyLock<PluginRegistry> = LazyLock::new(PluginRegistry::with_library_providers);

fn main() {
    let args = Arguments::from_args();
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("pzoom");

    let mut dirs: Vec<PathBuf> = Vec::new();
    collect_test_dirs(&corpus, &mut dirs);
    dirs.sort();

    // Build the prelude once up front so it isn't racing trial threads.
    LazyLock::force(&PRELUDE);
    LazyLock::force(&PLUGIN_REGISTRY);

    // The corpus intentionally contains known-failing tests (Mago false
    // positives being tracked down), so trials are ignored by default to keep
    // plain `cargo test --workspace` runs green. Run the corpus explicitly
    // with `cargo test -p mago-analyzer --test pzoom -- --ignored`.
    let trials: Vec<Trial> = dirs
        .into_iter()
        .map(|dir| {
            let name = dir.strip_prefix(&corpus).unwrap().to_string_lossy().replace('\\', "/");
            let rel = name.clone();
            Trial::test(name, move || run_test_on_analysis_stack(dir, rel).map_err(Failed::from))
                .with_ignored_flag(true)
        })
        .collect();

    libtest_mimic::run(&args, trials).exit();
}

fn collect_test_dirs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join("input.php").is_file() {
            out.push(path);
        } else {
            collect_test_dirs(&path, out);
        }
    }
}

/// Mago's CLI runs analysis on threads with a 12 MiB stack
/// (`DEFAULT_STACK_SIZE`); libtest threads only get 2 MiB, which deeply
/// recursive analysis can overflow. Run each test on a matching thread so the
/// harness reproduces `mago analyze` rather than the harness's own limits.
const ANALYSIS_STACK_SIZE: usize = 12 * 1024 * 1024;

fn run_test_on_analysis_stack(dir: PathBuf, rel: String) -> Result<(), String> {
    std::thread::Builder::new()
        .stack_size(ANALYSIS_STACK_SIZE)
        .spawn(move || run_test(&dir, &rel))
        .expect("failed to spawn analysis thread")
        .join()
        .map_err(|panic| match panic.downcast_ref::<String>() {
            Some(message) => format!("panicked: {message}"),
            None => match panic.downcast_ref::<&str>() {
                Some(message) => format!("panicked: {message}"),
                None => "panicked".to_string(),
            },
        })?
}

fn run_test(dir: &Path, rel: &str) -> Result<(), String> {
    let input_path = dir.join("input.php");
    let content = fs::read(&input_path).map_err(|e| format!("failed to read {}: {e}", input_path.display()))?;

    let version = php_version_for(dir);
    let settings = settings_for(rel, version);
    let suppressed = suppressed_psalm_kinds(dir);

    let Prelude { mut database, mut metadata, mut symbol_references } = PRELUDE.clone();

    let file = File::ephemeral(Cow::Borrowed(b"input.php"), Cow::Owned(content.clone()));
    let file_id = database.add(file);
    let source_file = database.get_ref(&file_id).expect("file just added should exist");

    let arena = LocalArena::new();
    let program = parse_file(&arena, source_file);
    if program.has_errors() {
        let mut msg = String::from("parse errors:\n");
        for error in program.errors.iter().take(5) {
            let _ = writeln!(msg, "  {error:?}");
        }
        return Err(msg);
    }

    let resolver = NameResolver::new(&arena);
    let resolved_names = resolver.resolve(program);

    metadata.extend(scan_program(&arena, source_file, program, &resolved_names, settings.version));
    populate_codebase(&mut metadata, &mut symbol_references, WordSet::default(), HashSet::default());

    let mut analysis_result = AnalysisResult::new(symbol_references);
    let analyzer = Analyzer::new(&arena, source_file, &resolved_names, &metadata, &PLUGIN_REGISTRY, settings);
    analyzer.analyze(program, &mut analysis_result).map_err(|e| format!("analysis error: {e}"))?;

    let mut issues = std::mem::take(&mut analysis_result.issues);
    issues.extend(metadata.take_issues(true));

    let mut remaining: Vec<String> = Vec::new();
    for issue in issues {
        // Match `mago analyze`'s default failure threshold. Warnings, help,
        // and notes are still useful diagnostics, but they do not make an
        // analysis-mode run fail.
        if issue.level < Level::Error {
            continue;
        }

        let code = issue.code.as_deref().unwrap_or("<uncoded>").to_string();
        if is_suppressed(&code, &suppressed) {
            continue;
        }

        let line = issue
            .annotations
            .iter()
            .find(|a| a.kind.is_primary())
            .or_else(|| issue.annotations.first())
            .map(|a| line_of_offset(&content, a.span.start.offset as usize))
            .unwrap_or(0);

        remaining.push(format!("[{code}] input.php:{line} {}", issue.message));
    }

    if remaining.is_empty() {
        Ok(())
    } else {
        remaining.sort();
        let mut msg = format!("{} unexpected issue(s):\n", remaining.len());
        for entry in &remaining {
            let _ = writeln!(msg, "  {entry}");
        }
        Err(msg)
    }
}

/// The corpus default is PHP 8.0 (pzoom's test-runner default); individual
/// tests pin another version via `php_version.txt`.
fn php_version_for(dir: &Path) -> PHPVersion {
    let Ok(raw) = fs::read_to_string(dir.join("php_version.txt")) else {
        return PHPVersion::PHP80;
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return PHPVersion::PHP80;
    }

    let mut parts = raw.split('.');
    let major: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(8);
    let minor: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    PHPVersion::new(major, minor, patch)
}

/// Settings approximating Psalm's test-suite defaults, with per-suite opt-ins
/// mirroring pzoom's test runner (which mirrors Psalm's per-class setUp).
fn settings_for(rel: &str, version: PHPVersion) -> Settings {
    let mut settings = Settings::new(version);

    // Psalm does not run unused-code analysis by default; only the dedicated
    // suites opt in (Psalm's UnusedVariableTest / UnusedCodeTest).
    settings.find_unused_expressions = false;
    settings.find_unused_definitions = false;
    // Psalm reports PropertyNotSetInConstructor/MissingConstructor by default.
    settings.check_property_initialization = true;

    if rel.starts_with("UnusedVariable/") {
        settings.find_unused_expressions = true;
    }
    if rel.starts_with("UnusedCode/") {
        settings.find_unused_expressions = true;
        settings.find_unused_definitions = true;
        settings.find_unused_parameters = true;
    }
    // Psalm's OverrideTest runs with ensureOverrideAttribute enabled.
    if rel.starts_with("Override/") {
        settings.check_missing_override = true;
    }

    settings
}

/// Psalm issue kinds this test suppresses via `error_levels.json`
/// (config-level suppression in the original test).
///
/// Tests using inline `@psalm-suppress` annotations are not ported at all
/// (Mago has no `@psalm-suppress` support), so config suppressions are the
/// only kind the harness needs to honor.
fn suppressed_psalm_kinds(dir: &Path) -> BTreeSet<String> {
    let mut kinds = BTreeSet::new();

    if let Ok(raw) = fs::read_to_string(dir.join("error_levels.json"))
        && let Ok(list) = serde_json::from_str::<Vec<String>>(&raw)
    {
        kinds.extend(list);
    }

    kinds
}

fn is_suppressed(mago_code: &str, suppressed: &BTreeSet<String>) -> bool {
    if suppressed.is_empty() {
        return false;
    }
    if suppressed.contains("all") {
        return true;
    }

    suppressed.iter().any(|kind| pascal_to_kebab(kind) == mago_code || psalm_kind_aliases(kind).contains(&mago_code))
}

fn pascal_to_kebab(kind: &str) -> String {
    let mut out = String::with_capacity(kind.len() + 8);
    for (i, c) in kind.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Mago issue codes that correspond to a Psalm issue kind whose mechanical
/// PascalCase -> kebab-case rename does not line up with Mago's naming.
///
/// Kinds where Mago simply has no analogous check map to nothing (the
/// mechanical conversion already returns a non-existent code).
fn psalm_kind_aliases(kind: &str) -> &'static [&'static str] {
    match kind {
        // Return types
        "InvalidReturnType" | "InvalidReturnStatement" => &[
            "invalid-return-statement",
            "less-specific-return-statement",
            "less-specific-nested-return-statement",
            "missing-return-statement",
            "nullable-return-statement",
            "falsable-return-statement",
        ],
        "LessSpecificReturnStatement" | "LessSpecificReturnType" | "MoreSpecificReturnType" => {
            &["less-specific-return-statement", "less-specific-nested-return-statement"]
        }
        "MixedReturnTypeCoercion" | "MixedInferredReturnType" => {
            &["mixed-return-statement", "less-specific-return-statement"]
        }
        "ImplementedReturnTypeMismatch" => &["incompatible-return-type"],
        "MismatchingDocblockReturnType" => &["docblock-type-mismatch"],
        "InvalidToString" => &["invalid-return-statement"],

        // Mixed family
        "MixedMethodCall" => &["mixed-method-access"],
        "MixedPropertyFetch" => &["mixed-property-access"],
        "MixedPropertyAssignment" => &["mixed-property-access", "mixed-property-type-coercion"],
        "MixedArrayOffset" => &["mixed-array-index"],
        "MixedArgumentTypeCoercion" => {
            &["mixed-argument", "less-specific-argument", "less-specific-nested-argument-type"]
        }
        "MixedArrayTypeCoercion" => &["mixed-array-index", "less-specific-argument"],
        "MixedStringOffsetAssignment" => &["mixed-array-assignment"],

        // Arguments
        "ArgumentTypeCoercion" => &["less-specific-argument", "less-specific-nested-argument-type"],
        "InvalidScalarArgument" | "InvalidLiteralArgument" => {
            &["invalid-argument", "possibly-invalid-argument", "less-specific-argument"]
        }
        "InvalidArgument" => &["invalid-argument", "possibly-invalid-argument", "null-argument", "false-argument"],
        "PossiblyInvalidArgument" => &["possibly-invalid-argument", "possibly-false-argument"],
        "InvalidNamedArgument" => &["invalid-named-argument", "duplicate-named-argument"],
        "TooManyArguments" => &["too-many-arguments"],
        "TooFewArguments" => &["too-few-arguments"],
        "InvalidPassByReference" => &["invalid-pass-by-reference"],

        // Undefined symbols
        "UndefinedClass" | "UndefinedDocblockClass" => &[
            "non-existent-class",
            "non-existent-class-like",
            "unknown-class-instantiation",
            "non-existent-catch-type",
            "non-existent-attribute-class",
        ],
        "UndefinedTrait" => &["non-existent-class-like"],
        "UndefinedConstant" => &["non-existent-constant", "non-existent-class-constant", "unresolvable-class-constant"],
        "UndefinedFunction" => &["non-existent-function", "unavailable-function"],
        "UndefinedMethod" | "UndefinedInterfaceMethod" => &["non-existent-method", "possibly-non-existent-method"],
        "UndefinedMagicMethod" => &["non-existent-method", "missing-magic-method"],
        "UndefinedPropertyFetch" | "UndefinedThisPropertyFetch" | "UndefinedMagicPropertyFetch" => {
            &["non-existent-property", "possibly-non-existent-property"]
        }
        "UndefinedPropertyAssignment" | "UndefinedThisPropertyAssignment" | "UndefinedMagicPropertyAssignment" => {
            &["non-existent-property"]
        }
        "NoInterfaceProperties" => &["non-existent-property", "possibly-non-existent-property"],
        "UndefinedGlobalVariable" => &["undefined-variable"],
        "PossiblyUndefinedGlobalVariable" => &["possibly-undefined-variable"],
        "UndefinedVariable" => &["undefined-variable", "undefined-variable-in-closure-use"],

        // Conditions
        "TypeDoesNotContainType" => &[
            "impossible-condition",
            "impossible-type-comparison",
            "impossible-null-type-comparison",
            "impossible-key-check",
            "impossible-nonnull-entry-check",
            "never-matching-switch-case",
            "unreachable-match-arm",
        ],
        "TypeDoesNotContainNull" => {
            &["impossible-condition", "impossible-null-type-comparison", "impossible-type-comparison"]
        }
        "RedundantCondition" | "RedundantConditionGivenDocblockType" => &[
            "redundant-condition",
            "redundant-type-comparison",
            "redundant-comparison",
            "redundant-nonnull-type-comparison",
            "redundant-isset-check",
            "redundant-key-check",
            "redundant-nonnull-entry-check",
            "redundant-null-coalesce",
            "redundant-logical-operation",
            "always-matching-switch-case",
            "match-arm-always-true",
            "match-default-arm-always-executed",
        ],
        "DocblockTypeContradiction" => &[
            "impossible-condition",
            "impossible-type-comparison",
            "impossible-null-type-comparison",
            "redundant-condition",
            "redundant-type-comparison",
            "docblock-type-mismatch",
        ],
        "RedundantPropertyInitializationCheck" => &["redundant-isset-check", "redundant-condition"],
        "RedundantCastGivenDocblockType" => &["redundant-cast"],
        "RiskyTruthyFalsyComparison" => &[],

        // Arrays
        "InvalidArrayOffset" => &[
            "invalid-array-index",
            "invalid-array-access",
            "impossible-array-access",
            "mismatched-array-index",
            "undefined-int-array-index",
            "undefined-string-array-index",
            "invalid-array-element-key",
        ],
        "PossiblyUndefinedArrayOffset" => &[
            "possibly-undefined-array-index",
            "possibly-undefined-int-array-index",
            "possibly-undefined-string-array-index",
        ],
        "PossiblyUndefinedIntArrayOffset" => &["possibly-undefined-int-array-index", "possibly-undefined-array-index"],
        "PossiblyUndefinedStringArrayOffset" => {
            &["possibly-undefined-string-array-index", "possibly-undefined-array-index"]
        }
        "PossiblyNullArrayOffset" => &["possibly-null-array-index"],
        "NullArrayOffset" => &["null-array-index"],
        "PossiblyNullArrayAccess" => &["possibly-null-array-access"],
        "PossiblyNullArrayAssignment" => &["possibly-null-array-access", "null-array-access"],
        "PossiblyInvalidArrayAccess" => &["possibly-invalid-array-access", "possibly-false-array-access"],
        "PossiblyInvalidArrayAssignment" => &["possibly-invalid-array-access", "impossible-array-assignment"],
        "EmptyArrayAccess" => &["impossible-array-access", "invalid-array-access"],
        "MismatchedArrayOffset" => &["mismatched-array-index"],

        // Properties
        "PropertyNotSetInConstructor" => &["uninitialized-property"],
        "InaccessibleProperty" => &["invalid-property-access", "incompatible-property-access"],
        "InvalidPropertyAssignmentValue" | "PossiblyInvalidPropertyAssignmentValue" => {
            &["invalid-property-assignment-value", "invalid-property-write", "possibly-invalid-property-write"]
        }
        "PossiblyNullPropertyFetch" => &["possibly-null-property-access", "null-property-access"],
        "PossiblyNullPropertyAssignment" => &["possibly-null-property-access", "null-property-access"],
        "NonInvariantDocblockPropertyType" | "NonInvariantPropertyType" => {
            &["incompatible-property-type", "incompatible-property-override"]
        }
        "MissingPropertyType" => &["missing-property-type"],
        "ImpurePropertyAssignment" => &[],

        // Methods / signatures
        "MethodSignatureMismatch" | "ImplementedParamTypeMismatch" => &[
            "incompatible-parameter-type",
            "incompatible-parameter-count",
            "incompatible-parameter-name",
            "incompatible-return-type",
            "incompatible-visibility",
            "incompatible-static-modifier",
        ],
        "ParamNameMismatch" => &["incompatible-parameter-name"],
        "ConstructorSignatureMismatch" => &["incompatible-parameter-count", "incompatible-parameter-type"],
        "UnimplementedInterfaceMethod" | "UnimplementedAbstractMethod" => &["unimplemented-abstract-method"],
        "PossiblyNullReference" => &[
            "possibly-null-property-access",
            "method-access-on-null",
            "possible-method-access-on-null",
            "null-property-access",
        ],
        "NullReference" => &["method-access-on-null", "null-property-access"],
        "PossiblyUndefinedMethod" => &["possibly-non-existent-method"],
        "PossiblyFalseReference" | "PossiblyInvalidMethodCall" => &["possibly-non-existent-method"],

        // Classes / instantiation
        "InvalidExtendClass" => &["invalid-extend", "extend-final-class"],
        "InvalidCatch" => &[
            "invalid-catch-type",
            "catch-type-not-throwable",
            "invalid-catch-type-not-class-or-interface",
            "non-existent-catch-type",
        ],
        "InvalidAttribute" => &[
            "non-existent-attribute-class",
            "non-class-used-as-attribute",
            "abstract-class-used-as-attribute",
            "class-not-marked-as-attribute",
            "invalid-attribute-target",
        ],
        "InvalidStringClass" => &["invalid-class-string-expression", "invalid-class-constant-on-string"],
        "RawObjectIteration" => &["generic-object-iteration", "non-iterable-object-iteration"],
        "InvalidTemplateParam" | "IncompatibleTypeParameters" => {
            &["invalid-template-parameter", "incompatible-template-lower-bound"]
        }
        "TooManyTemplateParams" => &["excess-template-parameter"],
        "MissingTemplateParam" => &["missing-template-parameter"],
        "InvalidTraitStatement" | "MissingDependency" => &["invalid-trait-use"],

        // Constants
        "InvalidConstantAssignmentValue" => &["incompatible-constant-type", "invalid-constant-value"],
        "MissingClassConstType" => &["missing-constant-type"],

        // Docblocks
        "MismatchingDocblockParamType" => &["docblock-type-mismatch", "docblock-parameter-narrowing"],
        "UnnecessaryVarAnnotation" => &["redundant-docblock-type"],
        "InvalidDocblock" | "InvalidDocblockParamName" => &["invalid-docblock"],

        // Scope / misc
        "InvalidScope" => &[
            "invalid-scope-keyword-context",
            "self-outside-class-scope",
            "static-outside-class-scope",
            "parent-outside-class-scope",
        ],
        "InvalidGlobal" => &["invalid-global"],
        "StringIncrement" => &["invalid-operand"],
        "MissingParamType" | "MissingClosureParamType" => &["missing-parameter-type"],
        "MissingReturnType" | "MissingClosureReturnType" => &["missing-return-type"],
        "MissingThrowsDocblock" => &["unhandled-thrown-type"],
        "UncaughtThrowInGlobalScope" => &["unhandled-thrown-type"],
        "ImplicitToStringCast" => &["implicit-to-string-cast", "implicit-resource-to-string-cast"],
        "UnusedParam" | "PossiblyUnusedParam" => &["unused-parameter"],
        "UnusedVariable" => &[],
        "UnusedClass" => &[],
        "PossiblyUnusedMethod" | "UnusedMethod" => &["unused-method"],
        "PossiblyUnusedProperty" | "UnusedProperty" => &["unused-property"],
        "PossiblyUnusedReturnValue" | "UnusedReturnValue" => &["unused-method-call", "unused-function-call"],
        "UnusedFunctionCall" => &["unused-function-call"],
        "UnusedMethodCall" => &["unused-method-call"],

        _ => &[],
    }
}

fn line_of_offset(content: &[u8], offset: usize) -> usize {
    let end = offset.min(content.len());
    1 + content[..end].iter().filter(|&&b| b == b'\n').count()
}
