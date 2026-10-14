<?php
class IndexLookup {
    private int $maxPerRpc = 256;
    /** Returns the next object, buffering up to maxPerRpc hashes. */
    public function getNext(): ?Item { return $this->take(); }
}
