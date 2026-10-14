<?php
class HttpRequest {
    private array $params = [];
    public function getParameter(string $name): ?string { return $this->params[$name] ?? null; }
}
