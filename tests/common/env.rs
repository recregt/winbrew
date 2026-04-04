pub struct TestEnvVar {
    key: &'static str,
    previous: Option<String>,
}

impl TestEnvVar {
    pub fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();

        unsafe {
            std::env::set_var(key, value);
        }

        Self { key, previous }
    }
}

impl Drop for TestEnvVar {
    fn drop(&mut self) {
        unsafe {
            match &self.previous {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}
