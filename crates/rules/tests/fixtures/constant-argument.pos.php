<?php
function respond(string $body, string $version) { return new Response($body, $version); }
function ok() { return respond("ok", "HTTP/1.1"); }
function missing() { return respond("missing", "HTTP/1.1"); }
function denied() { return respond("denied", "HTTP/1.1"); }
