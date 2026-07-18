<?php

declare(strict_types=1);

interface ConditionalConverter
{
    public function convert(string $value): ?ConditionalObject;
}

interface ConditionalObject
{
    public function isValid(): bool;
}

final class ConditionalObjectImpl implements ConditionalObject
{
    public function isValid(): bool
    {
        return true;
    }
}

function assignment_argument_order(ConditionalConverter $converter, string $value): ConditionalObject
{
    if (($value = $converter->convert($value)) === null || !$value->isValid()) {
        return new ConditionalObjectImpl();
    }

    return $value;
}

function assignment_in_and(mixed $value): bool
{
    return is_string($value)
        && (($value = rand(0, 1) ? new ConditionalObjectImpl() : null) !== null)
        && $value->isValid();
}

function assignment_in_or(mixed $value): bool
{
    return !is_string($value)
        || (($value = rand(0, 1) ? new ConditionalObjectImpl() : null) === null)
        || $value->isValid();
}

final class ConditionalContainer
{
    public ?ConditionalObject $value = null;
}

function assignment_reaches_else(?ConditionalContainer $container): void
{
    if (!$container || !($value = $container->value)) {
        return;
    } else {
        $value->isValid();
    }
}

class ConditionalParent {}

final class ConditionalChild extends ConditionalParent
{
    public function childMethod(): void {}
}

function get_class_narrows_object(ConditionalParent $object): void
{
    if (get_class($object) === ConditionalChild::class) {
        $object->childMethod();
    }
}

function negated_get_class_narrows_else(ConditionalParent $object): void
{
    if (get_class($object) !== ConditionalChild::class) {
        return;
    }

    $object->childMethod();
}

final class ConditionalFalsyProperties
{
    public ?string $first = null;
    public ?string $second = null;
}

function loose_false_comparison_negates_nested_assertion(ConditionalFalsyProperties $object): string
{
    if (($object->first === null) == false) {
        return $object->first;
    } elseif (is_null($object->second) == false) {
        return $object->second;
    }

    return 'fallback';
}

function isset_array_creation_across_loop_iterations(): void
{
    $values = [];

    foreach ([0, 1, 2, 3] as $_) {
        $key = (int) (rand(0, 1) ? 5 : '010');

        if (!isset($values[$key])) {
            $values[$key] = 5;
        } else {
            $values[$key] += 4;
        }
    }
}

final class ConditionalRangeEndpoint
{
    public function accepts(self $endpoint): void {}

    public function use(): void {}
}

function elseif_join_retains_cumulative_negations(
    ?ConditionalRangeEndpoint $from,
    ?ConditionalRangeEndpoint $to,
): void {
    if (!$to && !$from) {
        $to = new ConditionalRangeEndpoint();
        $from = new ConditionalRangeEndpoint();
    } elseif (!$from) {
        $from = new ConditionalRangeEndpoint();
        $from->accepts($to);
    } elseif (!$to) {
        $to = new ConditionalRangeEndpoint();
        $to->accepts($from);
    }

    $from->use();
    $to->use();
}

function assignment_on_one_if_branch_is_not_definite(bool $condition): void
{
    if ($condition) {
        $value = 1;
    } else {
        // The other continuing branch does not assign $value.
    }

    /** @mago-expect analysis:possibly-undefined-variable,mixed-argument */
    echo $value;
}

function assignment_on_one_switch_case_is_not_definite(int $selector): void
{
    switch ($selector) {
        case 0:
            $value = 1;
            break;

        default:
            break;
    }

    /** @mago-expect analysis:possibly-undefined-variable,mixed-argument */
    echo $value;
}
