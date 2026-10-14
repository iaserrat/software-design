<?php
class IndexLookup {
    private int $maxPerRpc = 256;
    /** Returns the next object in the range, or null once the range is exhausted. */
    public function getNext(): ?Item { return $this->take(); }
}
