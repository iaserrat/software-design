impl TextDocument {
    pub fn insert_string(&mut self, text: &str, offset: usize) {
        self.area.insert_string(text, offset)
    }
}
