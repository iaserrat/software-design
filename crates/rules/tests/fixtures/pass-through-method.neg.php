<?php
class BufferedStream implements Stream {
    public function read(int $count): string { return $this->inner->read($count); }
}
class Child extends Base {
    public function __construct(string $name) { parent::__construct($name); }
}
