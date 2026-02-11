/// Tools module for function calling / tool use capabilities
///
/// This module provides the infrastructure for LLM tool calling:
/// - Tool trait and registry for defining and managing tools
/// - Security context for sandboxing tool execution
/// - Tool executor for orchestrating tool calls
/// - Built-in tool implementations (file operations, command execution, web search)
mod executor;
mod implementations;
mod registry;
mod security;

pub use executor::ToolExecutor;
pub use implementations::{
    ExecuteCommandTool, ListDirectoryTool, ReadFileTool, SearchWebTool, WriteFileTool,
};
pub use registry::ToolRegistry;
pub use security::SecurityContext;
