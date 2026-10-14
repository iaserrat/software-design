<?php
function clampIndex(int $i, int $n): int { return max(0, min($i, $n)); }
function first(array $xs, int $i) { return $xs[clampIndex($i, count($xs))]; }
function last(array $xs, int $i) { return $xs[clampIndex(count($xs) - $i, count($xs))]; }
