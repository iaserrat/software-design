<?php
function addNullValue(array &$attrs, string $name): void { $attrs[$name] = null; }
function reset(array &$attrs): void { addNullValue($attrs, "id"); }
