export class IndexLookup {
  private isReady(): boolean { return this.ready; }
  /** Returns the next object in the range, or null once the range is exhausted. Blocks until one is available. */
  getNext(): Item | null { while (!this.isReady()) { this.poll(); } return this.take(); }
}
