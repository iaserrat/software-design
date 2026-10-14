<?php
function merge(array $lines): string {
    $result = implode("\n", $lines);
    return $result;
}
