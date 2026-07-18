<?php

/**
 * Dumps information about a variable for debugging purposes.
 *
 * @return ($return is true ? string : null)
 */
function debug(mixed $value, bool $return = false): null|string
{
    if ($return) {
        return var_export($value, true);
    }

    echo '--- debug --- ';
    echo '[' . gettype($value) . '] ';
    echo var_export($value, true) . "\n";
    echo '-------------' . "\n";

    return null;
}

debug('Hello, World!'); // ok
$result = debug(42, true); // ok
echo $result; // ok

final class ConditionalReturnUser {}

final class ConditionalReturnNullObject {}

/**
 * @template T of object|null
 * @param T $value
 * @return (T is object ? T : ConditionalReturnNullObject)
 */
function conditional_object_or_null_object(?object $value)
{
    return $value ?? new ConditionalReturnNullObject();
}

function conditional_return_refines_template_union(?ConditionalReturnUser $user): ConditionalReturnUser|ConditionalReturnNullObject
{
    return conditional_object_or_null_object($user);
}

/**
 * @template T of string|int
 * @param T $value
 * @return list<(T is string ? T : int)>
 */
function conditional_return_nested_in_list(string|int $value): array
{
    return [$value];
}

/** @return list<string> */
function conditional_return_resolves_inside_generic_parameter(): array
{
    return conditional_return_nested_in_list('value');
}

/** @return (PHP_VERSION_ID is int<80000, max> ? string : int) */
function conditional_return_for_php_version()
{
    return 'PHP 8 or newer';
}

function conditional_return_uses_configured_php_version(): string
{
    return conditional_return_for_php_version();
}

interface ConditionalGenericParameterParent
{
    public function accepts(bool $value): array;
}

final class ConditionalGenericParameterChild implements ConditionalGenericParameterParent
{
    /**
     * @template T of bool
     * @param T $value
     * @return (T is true ? array : array)
     */
    public function accepts(bool $value): array
    {
        return [];
    }
}
