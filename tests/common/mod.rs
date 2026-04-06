use std::sync::{Mutex, MutexGuard};

#[allow(dead_code)]
pub mod db;
#[allow(dead_code)]
pub mod shared_root;

#[allow(dead_code)]
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[allow(dead_code)]
pub fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
