pub struct IndexLookup { pending_hashes: Vec<u64> }
impl IndexLookup {
    /// Returns the next object, refilling pending_hashes when it runs dry.
    pub fn next_object(&mut self) -> Option<Item> { self.take() }
}
