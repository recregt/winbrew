#![allow(dead_code)]

pub struct MockServer {
    server: mockito::ServerGuard,
}

impl MockServer {
    pub fn new() -> Self {
        Self {
            server: mockito::Server::new(),
        }
    }

    pub fn url(&self) -> String {
        self.server.url()
    }

    pub fn get_json(&mut self, path: &str, body: &str) -> mockito::Mock {
        self.server
            .mock("GET", path)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create()
    }

    pub fn get_text(&mut self, path: &str, body: &str) -> mockito::Mock {
        self.get_text_status(path, 200, body)
    }

    pub fn get_text_status(&mut self, path: &str, status: usize, body: &str) -> mockito::Mock {
        self.server
            .mock("GET", path)
            .with_status(status)
            .with_header("content-type", "text/plain")
            .with_body(body)
            .create()
    }

    pub fn get_json_with_query(
        &mut self,
        path: &str,
        query: mockito::Matcher,
        body: &str,
    ) -> mockito::Mock {
        self.get_json_with_query_status(path, query, 200, body)
    }

    pub fn get_json_with_query_status(
        &mut self,
        path: &str,
        query: mockito::Matcher,
        status: usize,
        body: &str,
    ) -> mockito::Mock {
        self.server
            .mock("GET", path)
            .match_query(query)
            .with_status(status)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create()
    }
}
