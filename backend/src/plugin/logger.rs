use crate::plugin::types::PluginLogger;
use tracing::{debug, info, warn, error};

pub struct DefaultPluginLogger {
    plugin_name: String,
}

impl DefaultPluginLogger {
    pub fn new(plugin_name: String) -> Self {
        Self { plugin_name }
    }
}

impl PluginLogger for DefaultPluginLogger {
    fn debug(&self, message: &str) {
        debug!(plugin = %self.plugin_name, "{}", message);
    }
    fn info(&self, message: &str) {
        info!(plugin = %self.plugin_name, "{}", message);
    }
    fn warn(&self, message: &str) {
        warn!(plugin = %self.plugin_name, "{}", message);
    }
    fn error(&self, message: &str) {
        error!(plugin = %self.plugin_name, "{}", message);
    }
}
