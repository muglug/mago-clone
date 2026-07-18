<?php

function callable_target_if_object(callable $callable): void
{
    if (is_object($callable)) {
        $callable();
    }
}

function callable_target_if_callable(object $candidate): void
{
    if (is_callable($candidate)) {
        $candidate();
    }
}

/** @param callable(int $value): int $callable */
function callable_target_named_argument(callable $callable): int
{
    return $callable(value: 1);
}

final class CallableTargetHolder
{
    /** @var Closure(int): int */
    private Closure $callable;

    public function __construct()
    {
        $this->callable = static fn(int $value): int => $value + 1;
    }

    public function invoke(int $value): int
    {
        return $this->callable->__invoke($value);
    }

    public function dispatch(string $method, int $value): mixed
    {
        return call_user_func([$this, $method], $value);
    }
}

final class CallableTargetInvokable
{
    public function __invoke(int $value): bool
    {
        return $value > 0;
    }
}

/** @param Closure(int $value): bool $closure */
function callable_target_accepts_typed_closure(Closure $closure): void {}

function callable_target_from_callable(): void
{
    callable_target_accepts_typed_closure(Closure::fromCallable(new CallableTargetInvokable()));
    callable_target_accepts_typed_closure(Closure::fromCallable(static fn(int $value): bool => $value > 0));
}

function callable_target_accepts_callable(callable $callable): void
{
    $callable();
}

function callable_target_narrowed_array(object $object): void
{
    $callable = [$object::class, 'createFromFormat'];
    if (!is_callable($callable)) {
        return;
    }

    callable_target_accepts_callable($callable);
}

final class CallableTargetStatic
{
    public static function run(): string
    {
        return 'done';
    }

    /** @param list<int> $values */
    public static function sort(array $values): void
    {
        usort($values, ['self', 'compare']);
    }

    public static function compare(int $left, int $right): int
    {
        return $left <=> $right;
    }
}

function callable_target_leading_namespace_separator(): void
{
    callable_target_accepts_callable(['\CallableTargetStatic', 'run']);
}

/**
 * @template T
 * @param T $value
 * @return T
 */
function callable_target_identity(mixed $value): mixed
{
    return $value;
}

/**
 * @template A
 * @template B
 * @param A $value
 * @param callable(A): B $transform
 * @return B
 */
function callable_target_pipe(mixed $value, callable $transform): mixed
{
    return $transform($value);
}

function callable_target_accepts_int(int $value): void {}

function callable_target_specializes_inner_templates(): void
{
    callable_target_accepts_int(callable_target_pipe(1, callable_target_identity(...)));
}

/**
 * @template A
 * @template B
 * @param callable(A): B $callback
 * @return Closure(list<A>): list<B>
 */
function callable_target_map(callable $callback): Closure
{
    return fn(array $values) => array_map($callback, $values);
}

final class CallableTargetItem
{
    public bool $enabled = true;
}

/**
 * @template T
 * @param callable(T): bool $predicate
 * @return Closure(list<T>): list<T>
 */
function callable_target_filter(callable $predicate): Closure
{
    return fn(array $values) => array_values(array_filter($values, $predicate));
}

/** @param list<CallableTargetItem> $items */
function callable_target_contextualizes_nested_call(array $items): void
{
    callable_target_pipe(
        $items,
        callable_target_filter(fn($item) => $item->enabled),
    );
}

/** @param pure-callable(int): int $callback */
function callable_target_accepts_pure(callable $callback): void {}

function callable_target_infers_trivial_purity(): void
{
    callable_target_accepts_pure(fn(int $value): int => $value + 1);
    $total = 0;
    callable_target_accepts_pure(fn(int $value): int => $total += $value); // @mago-expect analysis:invalid-argument
}

function callable_target_forwards_first_class_callable(): void
{
    call_user_func(array_map(...), intval(...), ['1']);
}

abstract class CallableTargetBaseItem {}

final class CallableTargetLiteralItem extends CallableTargetBaseItem
{
    public string $value = '';
}

/** @param non-empty-list<CallableTargetLiteralItem> $items */
function callable_target_narrows_explicit_closure_parameter(array $items): void
{
    array_map(static fn(CallableTargetBaseItem $item): string => $item->value, $items);
}
