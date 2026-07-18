<?php

/** @psalm-immutable */
final class AssertionPathLeaf
{
    public function __construct(public ?string $value) {}
}

/** @psalm-immutable */
final class AssertionPathRoot
{
    public function __construct(public AssertionPathLeaf $leaf) {}
}

/** @psalm-assert-if-true !null $subject->leaf->value */
function assertion_path_has_value(AssertionPathRoot $subject): bool
{
    return $subject->leaf->value !== null;
}

/** @psalm-assert-if-false !null $subject->leaf->value */
function assertion_path_is_missing(AssertionPathRoot $subject): bool
{
    return $subject->leaf->value === null;
}

/**
 * @psalm-assert !null $subject->leaf->value
 * @throws InvalidArgumentException
 */
function assertion_path_require_value(AssertionPathRoot $subject): void
{
    if ($subject->leaf->value === null) {
        throw new InvalidArgumentException('A value is required');
    }
}

function assertion_path_requires_string(string $_value): void {}

/** @throws InvalidArgumentException */
function assertion_path_contracts(AssertionPathRoot $root): void
{
    if (assertion_path_has_value($root)) {
        assertion_path_requires_string($root->leaf->value);
    }

    if (!assertion_path_is_missing($root)) {
        assertion_path_requires_string($root->leaf->value);
    }

    assertion_path_require_value($root);
    assertion_path_requires_string($root->leaf->value);

    $alias = &$root;
    if (assertion_path_has_value($root)) {
        assertion_path_requires_string($alias->leaf->value);
    }
}

/**
 * @psalm-assert-if-true !null $value
 * @psalm-assert-if-true !string $value
 */
function assertion_contract_has_root(string|null|AssertionPathRoot $value): bool
{
    return $value instanceof AssertionPathRoot;
}

/** @psalm-assert-if-true int|string $value */
function assertion_contract_is_array_key(mixed $value): bool
{
    return is_int($value) || is_string($value);
}

function assertion_contract_requires_array_key(int|string $_value): void {}

/** @psalm-assert-if-true !null $value */
function assertion_contract_not_null(?string $value): bool
{
    return $value !== null;
}

function assertion_contract_reference_alias(?string $value): void
{
    $alias = &$value;
    if (assertion_contract_not_null($value)) {
        assertion_path_requires_string($alias);
    }
}

/** @psalm-immutable */
final class AssertionMethodContract
{
    public function __construct(private ?array $values) {}

    /** @psalm-assert-if-true !null $this->getValues() */
    public function hasValues(): bool
    {
        return $this->values !== null;
    }

    public function getValues(): ?array
    {
        return $this->values;
    }
}

function assertion_contract_method_result(AssertionMethodContract $subject): int
{
    if ($subject->hasValues()) {
        return count($subject->getValues());
    }

    return 0;
}

/** @throws RuntimeException */
function assertion_contract_clause_boundaries(string|null|AssertionPathRoot $value, mixed $key): AssertionPathRoot
{
    if (!assertion_contract_has_root($value)) {
        throw new RuntimeException('A root is required');
    }

    if (assertion_contract_is_array_key($key)) {
        assertion_contract_requires_array_key($key);
    }

    return $value;
}

final class AssertionStaticContract
{
    private static ?int $value = null;

    /** @psalm-assert int self::$value */
    private static function fill(): void
    {
        self::$value = 1;
    }

    public static function get(): int
    {
        self::fill();
        return self::$value;
    }
}

final class AssertionExternalState
{
    public static ?int $value = null;
}

final class AssertionExternalContract
{
    /** @psalm-assert int AssertionExternalState::$value */
    private static function fill(): void
    {
        AssertionExternalState::$value = 1;
    }

    public static function get(): int
    {
        self::fill();
        return AssertionExternalState::$value;
    }
}

/** @throws RuntimeException */
function assertion_inferred_require_not_null(mixed $value): void
{
    if ($value === null) {
        throw new RuntimeException('A value is required');
    }
}

/** @throws RuntimeException */
function assertion_inferred_not_null(?int $value): int
{
    assertion_inferred_require_not_null($value);
    return $value;
}

interface AssertionInferredFirst
{
    public function first(): void;
}

interface AssertionInferredSecond
{
    public function second(): void;
}

class AssertionInferredBase {}

/** @throws RuntimeException */
function assertion_inferred_require_interfaces(AssertionInferredBase $value): void
{
    if (!$value instanceof AssertionInferredFirst || !$value instanceof AssertionInferredSecond) {
        throw new RuntimeException('Both interfaces are required');
    }
}

/** @throws RuntimeException */
function assertion_inferred_interfaces(AssertionInferredBase $value): void
{
    assertion_inferred_require_interfaces($value);
    $value->first();
    $value->second();
}

/** @psalm-assert array{foo: string} $value */
function assertion_contract_has_foo(array $value): void {}

/** @psalm-assert array{bar: int} $value */
function assertion_contract_has_bar(array $value): void {}

function assertion_contract_shape_intersection(array $value): string
{
    assertion_contract_has_foo($value);
    assertion_contract_has_bar($value);
    return $value['foo'] . $value['bar'];
}

/** @psalm-assert non-empty-list $value */
function assertion_contract_non_empty_list(mixed $value): void {}

/** @param list<string> $values */
function assertion_contract_preserves_list_elements(array $values): string
{
    assertion_contract_non_empty_list($values);
    foreach ($values as $value) {}
    return $value;
}

/** @psalm-assert iterable<string> $value */
function assertion_contract_all_strings(mixed $value): void {}

/**
 * @param ArrayIterator<string, mixed> $values
 * @return ArrayIterator<string, string>
 */
function assertion_contract_refines_iterable_object(ArrayIterator $values): ArrayIterator
{
    assertion_contract_all_strings($values);
    return $values;
}
