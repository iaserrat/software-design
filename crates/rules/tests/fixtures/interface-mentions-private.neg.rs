pub struct IndexLookup { pending_hashes: Vec<u64> }
impl IndexLookup {
    /// Returns the next object in the range, or `None` once the range is exhausted.
    pub fn next_object(&mut self) -> Option<Item> { self.take() }
}
