impl Read for BufferedStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}
fn scaled(width: u32) -> u32 {
    scale(width, 2)
}
pub fn respond(body: &str, version: &str) -> Response { Response::new(body, version) }
