<?php

/**
 * @param array<int, string> $values
 * @return list<string>
 */
function merge_reindexes_integer_keys(array $values): array
{
    return array_merge($values);
}

/**
 * @param array{host?: string} $overrides
 * @return array{host: int|string}
 */
function merge_optional_override(array $overrides): array
{
    return array_merge(['host' => 0], $overrides);
}

/**
 * @param list<string> $values
 * @return array{0: string, 1: string}
 * @throws RuntimeException
 */
function reconcile_exact_list_count(array $values): array
{
    if (count($values) !== 2) {
        throw new RuntimeException('Expected exactly two values');
    }

    return $values;
}

/**
 * @param object{id: int} $row
 * @return non-empty-list<int>
 */
function column_preserves_required_object_property(object $row): array
{
    return array_column([$row], 'id');
}

function column_accepts_rows_php_will_skip(): array
{
    return array_column([['value' => 1], 42, ['value' => 2]], 'value');
}
