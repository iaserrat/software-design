fn respond(body: &str, version: &str) -> Response { Response::new(body, version) }
pub fn ok() -> Response { respond("ok", "HTTP/1.1") }
pub fn missing() -> Response { respond("missing", "HTTP/1.1") }
pub fn denied() -> Response { respond("denied", "HTTP/1.1") }
