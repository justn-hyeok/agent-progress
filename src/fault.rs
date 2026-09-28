//! Explicit opt-in process-boundary injection for private CLI fault fixtures.
pub(crate) fn checkpoint(stage: &str) {
    if std::env::var("AP_FAULT_STAGE").as_deref() == Ok(stage) {
        // Exit without unwinding: test persistence after abrupt process loss.
        std::process::exit(86);
    }
}
