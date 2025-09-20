use std::sync::Mutex;

// Global mutex to ensure tests run one at a time
// This prevents race conditions when tests use shared resources like working directories
pub static TEST_MUTEX: Mutex<()> = Mutex::new(());
