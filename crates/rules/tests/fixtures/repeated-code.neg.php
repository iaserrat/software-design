<?php
function onRead($packet) { if ($packet->length < READ_SIZE) { drop($packet); } }
function onWrite($frame) { if ($frame->length < WRITE_SIZE) { drop($frame); } }
