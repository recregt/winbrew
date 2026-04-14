use mockito::{Mock, Server, ServerGuard};

pub struct MockServer {
    server: ServerGuard,
    url: String,
}

impl MockServer {
    pub fn new() -> Self {
        let server = Server::new();
        let url = server.url().to_string();
        Self { server, url }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn mock_get(&mut self, path: &str, body: impl AsRef<[u8]>) -> Mock {
        self.mock_get_with_status(path, 200, body)
    }

    pub fn mock_get_with_status(
        &mut self,
        path: &str,
        status: usize,
        body: impl AsRef<[u8]>,
    ) -> Mock {
        self.server
            .mock("GET", path)
            .with_status(status)
            .with_body(body)
            .expect(1)
            .create()
    }
}

impl Default for MockServer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for MockServer {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.url
    }
}
