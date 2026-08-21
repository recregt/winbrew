use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

pub fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

#[cfg(test)]
mod tests {
    use super::*;

    // `env_lock()` is what makes the unsafe `set_var`/`remove_var` calls above
    // sound under concurrent test execution -- without it, two tests mutating
    // process env vars in parallel threads would be a data race. Miri's race
    // detector will catch a regression here if that lock is ever dropped.

    #[test]
    fn set_applies_value_and_drop_restores_previous() {
        let _guard = env_lock();
        let key = "WINBREW_TESTING_ENV_ROUNDTRIP_RESTORE";
        unsafe {
            std::env::set_var(key, "before");
        }

        {
            let var = TestEnvVar::set(key, "after");
            assert_eq!(std::env::var(key).as_deref(), Ok("after"));
            drop(var);
        }

        assert_eq!(std::env::var(key).as_deref(), Ok("before"));

        unsafe {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn set_applies_value_and_drop_removes_when_previously_unset() {
        let _guard = env_lock();
        let key = "WINBREW_TESTING_ENV_ROUNDTRIP_UNSET";
        unsafe {
            std::env::remove_var(key);
        }

        {
            let var = TestEnvVar::set(key, "after");
            assert_eq!(std::env::var(key).as_deref(), Ok("after"));
            drop(var);
        }

        assert!(std::env::var(key).is_err());
    }
}
