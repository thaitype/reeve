use std::sync::{atomic::AtomicU32, Arc, Mutex};

use crate::security::SecurityConfig;

use super::audit::AuditWriter;

pub struct RunContext {
    pub security: Arc<SecurityConfig>,
    pub audit: Arc<Mutex<AuditWriter>>,
    /// Counts successfully completed exec calls (exec_end emitted).
    pub exec_counter: Arc<AtomicU32>,
}
