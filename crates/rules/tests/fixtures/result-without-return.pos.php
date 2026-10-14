<?php
function render(array $lines): void {
    $result = implode("\n", $lines);
    echo $result;
}
