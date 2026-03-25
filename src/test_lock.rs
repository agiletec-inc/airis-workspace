/// Global lock for tests that use `std::env::set_current_dir`.
///
/// `set_current_dir` changes process-wide state, so tests using it
/// must serialize — even across modules. All such tests must acquire
/// this single lock instead of defining their own per-module `DIR_LOCK`.
#[cfg(test)]
pub static DIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
