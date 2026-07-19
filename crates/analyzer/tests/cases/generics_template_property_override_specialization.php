<?php

declare(strict_types=1);

class PropertyAnimal {}
class PropertyDog extends PropertyAnimal {}

/** @template-covariant T */
class CovariantPropertyBox
{
    /** @var T|null */
    public $value = null;
}

/** @extends CovariantPropertyBox<PropertyAnimal> */
class DogPropertyBox extends CovariantPropertyBox
{
    /** @var PropertyDog|null */
    public $value = null;
}

/** @template T of PropertyAnimal */
class PropertyList
{
    /** @var list<T> */
    protected array $items = [];
}

/**
 * @template T of PropertyDog
 * @extends PropertyList<T>
 */
class DogPropertyList extends PropertyList
{
    /** @var list<T> */
    protected array $items = [];
}

/** @extends DogPropertyList<PropertyDog> */
class ConcreteDogPropertyList extends DogPropertyList
{
    /** @var list<PropertyDog> */
    protected array $items = [];
}

class ReadonlyPropertyParent
{
    /**
     * @readonly
     * @var null|string
     */
    protected $label = null;
}

class ReadonlyPropertyChild extends ReadonlyPropertyParent
{
    /** @var string */
    protected $label = '';
}

class NativeReadonlyPropertyParent
{
    public function __construct(public readonly string $label) {}
}

class NativeReadonlyPropertyChild extends NativeReadonlyPropertyParent
{
    /** @mago-expect analysis:incompatible-readonly-modifier */
    public string $label = '';
}
