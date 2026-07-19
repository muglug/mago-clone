<?php

/**
 * @return non-empty-array
 */
function non_empty_after_count_match(array $items): array
{
    return match (count($items)) {
        0 => throw new InvalidArgumentException(),
        default => $items,
    };
}

interface MatchSubjectValue
{
}

final class ConcreteMatchSubjectValue implements MatchSubjectValue
{
    public function describe(): string
    {
        return 'concrete';
    }
}

function invoke_after_get_class_match(MatchSubjectValue $value): string
{
    return match (get_class($value)) {
        ConcreteMatchSubjectValue::class => $value->describe(),
        default => 'other',
    };
}
