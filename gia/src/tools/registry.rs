use anyhow::Result;
use async_trait::async_trait;
use genai::chat::Tool;
use serde_json::Value;
use std::collections::HashMap;

use super::security::SecurityContext;

/// Trait for implementing gia tools
///
/// Each tool implements this trait to define its name, description, JSON schema,
/// and execution logic. The trait follows the KISS principle: simple, focused methods.
#[async_trait]
pub trait GiaTool: Send + Sync {
    /// Tool name (e.g., "read_file")
    fn name(&self) -> &str;

    /// Human-readable description for the LLM
    fn description(&self) -> &str;

    /// JSON Schema for parameters (JSON Schema Draft 7)
    fn schema(&self) -> Value;

    /// Execute the tool with given arguments
    ///
    /// # Arguments
    /// * `args` - JSON value containing the tool arguments
    /// * `context` - Security context for validation
    ///
    /// # Returns
    /// Result with the tool output as a string, or an error
    async fn execute(&self, args: Value, context: &SecurityContext) -> Result<String>;

    /// Convert to genai::Tool for API (DRY: common conversion logic)
    fn to_genai_tool(&self) -> Tool {
        Tool::new(self.name())
            .with_description(self.description())
            .with_schema(self.schema())
    }
}

/// Registry of available tools
///
/// Simple HashMap-based registry following the KISS principle.
/// Tools register themselves and can be retrieved by name.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn GiaTool>>,
}

impl ToolRegistry {
    /// Create a new empty tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: Box<dyn GiaTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<&dyn GiaTool> {
        self.tools.get(name).map(|b| b.as_ref())
    }

    /// Convert all registered tools to genai::Tool format
    pub fn to_genai_tools(&self) -> Vec<Tool> {
        self.tools
            .values()
            .map(|tool| tool.to_genai_tool())
            .collect()
    }

    /// Get the number of registered tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
