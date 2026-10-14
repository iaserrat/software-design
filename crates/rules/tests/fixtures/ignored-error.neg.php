<?php
function cleanup(string $name): void {
    try { unsetVariable($name); } catch (\Throwable $e) { report($e); }
}
