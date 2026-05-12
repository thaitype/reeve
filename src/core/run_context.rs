use std::sync::{Arc, Mutex};

use crate::security::SecurityConfig;

use super::audit::AuditWriter;

pub struct RunContext {
    pub security: Arc<SecurityConfig>,
    pub audit: Arc<Mutex<AuditWriter>>,
}
