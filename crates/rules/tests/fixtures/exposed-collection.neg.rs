pub struct HttpRequest { params: HashMap<String, String> }
impl HttpRequest {
    pub fn parameter(&self, name: &str) -> Option<&str> { self.params.get(name).map(String::as_str) }
}
