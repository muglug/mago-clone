<?php

final class RepeatedValue
{
    public function execute(): void {}
}

final class FinalGetter
{
    private ?RepeatedValue $value = null;

    public function getValue(): ?RepeatedValue
    {
        return $this->value;
    }
}

/** @psalm-mutation-free */
function getNullableInt(FinalGetter $getter): ?int
{
    return $getter->getValue() !== null ? 1 : null;
}

function consumeInt(int $value): void {}

$getter = new FinalGetter();

if ($getter->getValue() !== null) {
    /** @mago-expect analysis:possible-method-access-on-null */
    $getter->getValue()->execute();
}

if (getNullableInt($getter) !== null) {
    // Function calls are not classified as stable method results, so this
    // remains an ordinary possibly-null diagnostic without repeated-call help.
    /** @mago-expect analysis:possibly-null-argument */
    consumeInt(getNullableInt($getter));
}

final class MutationFreeGetter
{
    /** @psalm-mutation-free */
    public function getInt(): ?int
    {
        return null;
    }
}

$mutationFree = new MutationFreeGetter();
if ($mutationFree->getInt() !== null) {
    /** @mago-expect analysis:possibly-null-argument */
    consumeInt($mutationFree->getInt());
}
