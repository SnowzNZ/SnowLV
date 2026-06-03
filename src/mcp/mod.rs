//! MCP (Model Context Protocol) server module for SnowLV
//!
//! This module implements an MCP server that allows LLMs like Claude to
//! interact with the SnowLV application, controlling channel visualization,
//! computing derived channels, and analyzing ECU log data.
//!
//! The server runs as an HTTP service and Claude Desktop can connect via:
//! `http://localhost:52385/mcp` (default port)

pub mod client;
pub mod server;

pub use server::{start_mcp_server, McpServerHandle, SnowLVMcpServer, DEFAULT_MCP_PORT};
