use crate::plugin::types::PluginEventBus;
use crate::core::error::Result;
use serde_json::Value;

pub struct DefaultPluginEventBus;

impl DefaultPluginEventBus {
    pub fn new() -> Self {
        Self
    }
}

impl PluginEventBus for DefaultPluginEventBus {
    fn publish(&self, _event_type: &str, _data: Value) -> Result<()> {
        // TODO: Implement event bus
        Ok(())
    }
    fn subscribe(&self, _event_type: &str, _handler: Box<dyn Fn(Value) + Send + Sync>) -> Result<String> {
        Ok("subscription_id".to_string())
    }
    fn unsubscribe(&self, _subscription_id: &str) -> Result<()> {
        Ok(())
    }
}
