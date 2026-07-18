<?php

final class PdoFetchModeRow {}

/** @throws PDOException */
function pdo_prepare_retains_failure_type(PDO $pdo): PDOStatement|false
{
    return $pdo->prepare('SELECT 1');
}

/**
 * @return array<string, scalar|null>|false
 * @throws PDOException
 */
function pdo_fetch_assoc(PDO $pdo): array|false
{
    $statement = $pdo->prepare('SELECT 1');
    $statement->execute();

    return $statement->fetch(mode: PDO::FETCH_ASSOC);
}

/**
 * @return list<PdoFetchModeRow>
 * @throws PDOException
 */
function pdo_fetch_all_class(PDO $pdo): array
{
    $statement = $pdo->prepare('SELECT 1');
    $statement->execute();

    return $statement->fetchAll(PDO::FETCH_CLASS, PdoFetchModeRow::class);
}
