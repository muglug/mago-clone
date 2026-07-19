<?php

declare(strict_types=1);

function infinite_for_result(): int
{
    for (; 1;) {
        return 1;
    }
}

function infinite_object_condition_result(): string
{
    /** @mago-expect analysis:unused-statement */
    while (new stdClass()) {
        return '';
    }
}

/**
 * @param array<int, string> $values
 * @return non-empty-array
 */
function foreach_body_has_non_empty_array(array $values): array
{
    foreach ($values as $_value) {
        return $values;
    }

    return ['fallback'];
}

final class ForeachIssueStore
{
    /** @var array<int, stdClass> */
    public array $issues = [];
}

function consume_foreach_issue(stdClass $_issue): void {}

function foreach_property_is_non_empty(ForeachIssueStore $store): void
{
    foreach ($store->issues as $_issue) {
        $first = reset($store->issues);
        consume_foreach_issue($first);
    }
}

/** @param non-empty-array<string> $values */
function foreach_target_overwrites_back_edge(array $values): void
{
    $value = reset($values);
    foreach ($values as $value) {
        strlen($value);
        $value = 0;
    }
}

function mutate_loop_value(?string &$value): void {}

function do_while_by_reference_reaches_both_branches(): void
{
    $value = null;
    do {
        if ($value === null || $value === '') {
            mutate_loop_value($value);
        } else {
            mutate_loop_value($value);
        }
    } while (rand(0, 1));
}

function iterate_simple_xml_without_nullable_element(SimpleXMLElement $element): void
{
    foreach ($element as $child) {
        iterate_simple_xml_without_nullable_element($child);
    }
}
