<?php
function onRead($packet) {
    if ($packet->length < READ_SIZE) {
        warn("short read packet", $packet->id);
        drop($packet);
    }
}
function onWrite($frame) {
    if ($frame->length < WRITE_SIZE) {
        warn("short write packet", $frame->id);
        drop($frame);
    }
}
