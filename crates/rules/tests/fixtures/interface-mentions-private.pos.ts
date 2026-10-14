export class IndexLookup {
  private readonly MAX_PER_RPC = 256;
  private isReady(): boolean { return this.ready; }
  /** Returns the next object. Calls isReady in a loop until MAX_PER_RPC hashes are buffered. */
  getNext(): Item | null { while (!this.isReady()) { this.poll(); } return this.take(); }
}
