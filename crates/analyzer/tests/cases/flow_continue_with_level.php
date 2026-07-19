<?php

declare(strict_types=1);

/**
 * @param list<list<int>> $matrix
 */
function flow_continue_with_level(array $matrix, int $threshold): int
{
    $count = 0;

    foreach ($matrix as $row) {
        foreach ($row as $value) {
            if ($value < $threshold) {
                continue 2;
            }
            $count++;
        }
    }

    return $count;
}

/** @param list<string> $values */
function flow_continue_counts_switch_as_level(array $values): int
{
    $result = 0;
    foreach ($values as $value) {
        switch ($value) {
            case 'one':
                $current = 1;
                break;
            case 'two':
                $current = 2;
                break;
            default:
                continue 2;
        }

        $result += $current;
    }

    return $result;
}
