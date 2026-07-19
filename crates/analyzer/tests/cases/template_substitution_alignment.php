<?php

/**
 * @template TData of array<array-key, int|string>
 */
abstract class TemplateDataBag
{
    /** @var TData */
    protected array $data;

    /** @param TData $data */
    public function __construct(array $data)
    {
        $this->data = $data;
    }

    /**
     * @template TKey of key-of<TData>
     * @param TKey $key
     * @return TData[TKey]
     */
    public function get(int|string $key): int|string
    {
        return $this->data[$key];
    }
}

/**
 * @template TResult of string|list<string>
 * @param TResult $result
 * @return (TResult is array ? list<string> : string)
 */
function reconcileGenericArray(string|array $result): string|array
{
    if (is_array($result)) {
        return $result;
    }

    return strtoupper($result);
}

/**
 * @template TKey of int
 * @template TArray of array<TKey, bool>
 * @param TArray $array
 * @return list<TKey>
 */
function preserveNestedArrayKeyTemplate(array $array): array
{
    return array_keys($array);
}

/** @template TValue */
class TemplatePromise
{
    /** @param TValue $value */
    public function __construct(mixed $value) {}
}

/**
 * @template TReturn
 * @template TPromise
 * @template TResult of TemplatePromise<TPromise>|TReturn
 * @param TResult $result
 * @return TemplatePromise<TReturn>|TemplatePromise<TPromise>
 */
function wrapConditionalTemplate(mixed $result): TemplatePromise
{
    if ($result instanceof TemplatePromise) {
        return $result;
    }

    return new TemplatePromise($result);
}

/** @template TValue */
class ParentTemplateContainer
{
    /** @var TValue */
    private mixed $value;

    /** @param TValue $value */
    public function __construct(mixed $value)
    {
        $this->value = $value;
    }

    /** @return TValue */
    public function get(): mixed
    {
        return $this->value;
    }
}

/**
 * @template TValue of object
 * @extends ParentTemplateContainer<TValue>
 */
class ChildTemplateContainer extends ParentTemplateContainer
{
    /** @return TValue */
    public function get(): object
    {
        return parent::get();
    }
}

/**
 * @template TObject of TemplateStaticFactory
 * @param TObject $object
 * @return TObject
 */
function preserveGenericStaticReturn(TemplateStaticFactory $object): TemplateStaticFactory
{
    return $object::make();
}

/** @consistent-constructor */
class TemplateStaticFactory
{
    /** @return static */
    public static function make(): static
    {
        return new static();
    }
}

/**
 * @template TObject of TemplateStaticFactory
 * @param class-string<TObject> $class
 * @return TObject
 */
function preserveGenericClassStringStaticReturn(string $class): TemplateStaticFactory
{
    return $class::make();
}

/**
 * @template TObject of object
 * @param TObject::class $class
 * @return TObject::class
 */
function preserveTemplateClassConstantSyntax(string $class): string
{
    return $class;
}

/**
 * @template TObject of object
 * @param TObject $object
 * @return TObject::class
 */
function preserveGetClassTemplate(object $object): string
{
    return get_class($object);
}

/** @template TValue */
final class AssertedTemplateValue
{
    /** @param TValue $value */
    public function __construct(public mixed $value) {}
}

/** @template TExpected */
final class TemplateAssertionContract
{
    /** @var TExpected */
    public mixed $example;

    /** @param TExpected $example */
    public function __construct(mixed $example)
    {
        $this->example = $example;
    }

    /**
     * @param mixed $value
     * @mago-assert TExpected $value
     */
    public function assertValue(mixed $value): void {}
}

/**
 * @template TValue
 * @param int|string $value
 * @param TemplateAssertionContract<TValue> $contract
 * @return AssertedTemplateValue<TValue>
 */
function preserveClassTemplateAssertion(
    int|string $value,
    TemplateAssertionContract $contract,
): AssertedTemplateValue {
    $contract->assertValue($value);

    return new AssertedTemplateValue($value);
}

/**
 * @template TValue
 * @param TValue $value
 * @return AssertedTemplateValue<TValue>
 */
function widenUnconstrainedTemplateLiteral(mixed $value): AssertedTemplateValue
{
    return new AssertedTemplateValue($value);
}

/** @param AssertedTemplateValue<string> $value */
function acceptWidenedTemplateLiteral(AssertedTemplateValue $value): void {}

acceptWidenedTemplateLiteral(widenUnconstrainedTemplateLiteral('value'));

/** @template TValue of 'left'|'right' */
final class FiniteLiteralTemplate
{
    /** @param TValue $value */
    public function __construct(public string $value) {}
}

/** @return FiniteLiteralTemplate<'left'> */
function preserveFiniteTemplateLiteral(): FiniteLiteralTemplate
{
    return new FiniteLiteralTemplate('left');
}

/** @template TObject of object */
final class LateStaticClassName
{
    /** @param class-string<TObject> $class */
    public function __construct(public string $class) {}
}

class LateStaticClassNameFactory
{
    /** @return LateStaticClassName<static> */
    public static function className(): LateStaticClassName
    {
        return new LateStaticClassName(static::class);
    }
}

/** @param LateStaticClassName<LateStaticClassNameFactory> $name */
function acceptDirectLateStaticClassName(LateStaticClassName $name): void {}

acceptDirectLateStaticClassName(LateStaticClassNameFactory::className());
