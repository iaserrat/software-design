pub struct HttpRequest { params: HashMap<String, String> }
impl HttpRequest {
    pub fn params(&self) -> &HashMap<String, String> { &self.params }
}
