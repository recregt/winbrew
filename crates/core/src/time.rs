use chrono::Utc;

pub fn now() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}
