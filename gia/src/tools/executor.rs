use genai::chat::{ToolCall, ToolResponse};

use crate::logging::{log_debug, log_error, log_info};

use super::registry::ToolRegistry;
use super::security::SecurityContext;

/// Tool executor orchestrates tool execution
///
/// KISS principle: Simple orchestration with centralized error handling.
/// DRY principle: Single execution logic used for all tools.
pub struct ToolExecutor {
    registry: ToolRegistry,
    security_context: SecurityContext,
}

impl ToolExecutor {
    /// Create a new tool executor
    pub fn new(registry: ToolRegistry, security_context: SecurityContext) -> Self {
        Self {
            registry,
            security_context,
        }
    }

    /// Execute a single tool call with error handling
    ///
    /// DRY: Centralized execution with consistent error handling
    pub async fn execute_tool_call(&self, tool_call: &ToolCall) -> ToolResponse {
        log_info(&format!(
            "Executing tool: {} (call_id: {})",
            tool_call.fn_name, tool_call.call_id
        ));
        log_debug(&format!("Tool arguments: {:?}", tool_call.fn_arguments));

        let result = match self.registry.get(&tool_call.fn_name) {
            Some(tool) => {
                match tool
                    .execute(tool_call.fn_arguments.clone(), &self.security_context)
                    .await
                {
                    Ok(output) => {
                        log_info(&format!(
                            "Tool {} succeeded, output length: {}",
                            tool_call.fn_name,
                            output.len()
                        ));
                        output
                    }
                    Err(e) => {
                        log_error(&format!("Tool {} failed: {}", tool_call.fn_name, e));
                        format!("Error executing {}: {}", tool_call.fn_name, e)
                    }
                }
            }
            None => {
                log_error(&format!("Unknown tool requested: {}", tool_call.fn_name));
                format!("Error: Unknown tool '{}'", tool_call.fn_name)
            }
        };

        ToolResponse::new(tool_call.call_id.clone(), result)
    }

    /// Execute multiple tool calls in parallel
    ///
    /// Uses futures::join_all for efficient parallel execution
    pub async fn execute_tool_calls(&self, tool_calls: &[ToolCall]) -> Vec<ToolResponse> {
        log_info(&format!("Executing {} tool call(s)", tool_calls.len()));

        // Execute tools in parallel using join_all
        let futures: Vec<_> = tool_calls
            .iter()
            .map(|tc| self.execute_tool_call(tc))
            .collect();

        futures::future::join_all(futures).await
    }

    /// Get a reference to the tool registry
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }
}
