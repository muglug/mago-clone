<?php

/**
 * @param array<1|2|3, string> $values
 * @return 1|2|3
 */
function narrowArrayKey(array $values, int $key): int
{
    if (array_key_exists($key, $values)) {
        return $key;
    }

    return 1;
}

/**
 * @return int<48, 57>
 * @throws RuntimeException
 */
function narrowCtypeDigit(int $value): int
{
    if (ctype_digit($value)) {
        return $value;
    }

    throw new RuntimeException();
}

function narrowLooseBoolean(?DateTime $date): void
{
    if (($date !== null && $date->format('Y') === '2020') == true) {
        $date->format('d-m-Y');
    }
}

/** @param callable-string|list{1} $value */
function subtractCallable(string|array $value): void
{
    if (!is_callable($value)) {
        array_pop($value);
    }
}

/**
 * @param int|string $left
 * @param float|string $right
 */
function intersectStrictUnions(int|string $left, float|string $right): void
{
    if ($left === $right) {
        acceptString($left);
        acceptString($right);
    }
}

function acceptString(string $_value): void {}
