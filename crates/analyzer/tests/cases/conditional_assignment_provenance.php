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
