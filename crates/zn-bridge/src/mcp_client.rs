//! MCP Client - Connect to external MCP servers
//!
//! This module provides:
//! - MCP server configuration and management
//! - Tool discovery and execution
//! - Support for multiple MCP servers (GitHub, Linear, Filesystem, etc.)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// MCP Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name
    pub name: String,
    /// Command to run the server
    pub command: String,
    /// Command arguments
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Tools to include (if empty, include all)
    #[serde(default)]
    pub tools_include: Vec<String>,
    /// Tools to exclude
    #[serde(default)]
    pub tools_exclude: Vec<String>,
}

/// MCP Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema (JSON Schema)
    pub input_schema: Value,
    /// Server that provides this tool
    pub server: String,
}

/// MCP Tool call result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    /// Tool name
    pub tool: String,
    /// Server name
    pub server: String,
    /// Result content
    pub content: Value,
    /// Whether the call was successful
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// MCP Client for managing servers and tools
pub struct McpClient {
    servers: HashMap<String, McpServerConfig>,
    tools: HashMap<String, McpTool>,
    config_path: PathBuf,
    /// 项目根目录，用于限制文件系统操作范围
    project_root: Option<PathBuf>,
}

impl McpClient {
    /// Create a new MCP Client from config file
    pub fn new(config_path: PathBuf) -> Result<Self> {
        let mut client = Self {
            servers: HashMap::new(),
            tools: HashMap::new(),
            config_path,
            project_root: None,
        };
        client.load_config()?;
        Ok(client)
    }

    /// Create a new MCP Client with project root restriction
    pub fn new_with_project_root(config_path: PathBuf, project_root: PathBuf) -> Result<Self> {
        let mut client = Self {
            servers: HashMap::new(),
            tools: HashMap::new(),
            config_path,
            project_root: Some(project_root),
        };
        client.load_config()?;
        Ok(client)
    }

    /// Set project root for filesystem operation restrictions
    pub fn set_project_root(&mut self, project_root: PathBuf) {
        self.project_root = Some(project_root);
    }

    /// Load configuration from file
    pub fn load_config(&mut self) -> Result<()> {
        if !self.config_path.exists() {
            debug!("MCP config file not found: {}", self.config_path.display());
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.config_path)
            .with_context(|| format!("Failed to read MCP config: {}", self.config_path.display()))?;

        let config: McpConfig = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse MCP config YAML")?;

        for (name, server_config) in config.mcp_servers {
            // Register tools for this server
            let tools = self.get_default_tools_for_server(&name);
            for tool in tools {
                self.tools.insert(tool.name.clone(), tool);
            }
            self.servers.insert(name, server_config);
        }

        info!("Loaded {} MCP server configurations", self.servers.len());
        Ok(())
    }

    /// Get default tools for a server type
    fn get_default_tools_for_server(&self, server: &str) -> Vec<McpTool> {
        match server {
            "github" => vec![
                McpTool {
                    name: "create_issue".to_string(),
                    description: "Create a GitHub issue".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "body": { "type": "string" }
                        }
                    }),
                    server: server.to_string(),
                },
                McpTool {
                    name: "list_issues".to_string(),
                    description: "List GitHub issues".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {}
                    }),
                    server: server.to_string(),
                },
                McpTool {
                    name: "create_pull_request".to_string(),
                    description: "Create a pull request".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "body": { "type": "string" },
                            "head": { "type": "string" },
                            "base": { "type": "string" }
                        }
                    }),
                    server: server.to_string(),
                },
            ],
            "linear" => vec![
                McpTool {
                    name: "create_issue".to_string(),
                    description: "Create a Linear issue".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "description": { "type": "string" }
                        }
                    }),
                    server: server.to_string(),
                },
                McpTool {
                    name: "list_issues".to_string(),
                    description: "List Linear issues".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {}
                    }),
                    server: server.to_string(),
                },
            ],
            "filesystem" => vec![
                McpTool {
                    name: "read_file".to_string(),
                    description: "Read a file".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" }
                        },
                        "required": ["path"]
                    }),
                    server: server.to_string(),
                },
                McpTool {
                    name: "write_file".to_string(),
                    description: "Write a file".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" },
                            "content": { "type": "string" }
                        },
                        "required": ["path", "content"]
                    }),
                    server: server.to_string(),
                },
                McpTool {
                    name: "list_dir".to_string(),
                    description: "List directory contents".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" }
                        },
                        "required": ["path"]
                    }),
                    server: server.to_string(),
                },
            ],
            _ => vec![],
        }
    }

    /// List all available tools from all servers
    pub fn list_tools(&self) -> Vec<&McpTool> {
        self.tools.values().collect()
    }

    /// Get tools by server
    pub fn get_tools_by_server(&self, server: &str) -> Vec<&McpTool> {
        self.tools
            .values()
            .filter(|t| t.server == server)
            .collect()
    }

    /// Call a tool on an MCP server
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        args: Value,
    ) -> Result<McpToolResult> {
        let result = match server {
            "github" => self.call_github_tool(tool, args).await,
            "linear" => self.call_linear_tool(tool, args).await,
            "filesystem" => self.call_filesystem_tool(tool, args).await,
            _ => {
                Ok(Value::String(format!(
                    "Tool {} called on server {} with args: {:?}",
                    tool, server, args
                )))
            }
        };

        match result {
            Ok(content) => Ok(McpToolResult {
                tool: tool.to_string(),
                server: server.to_string(),
                content,
                success: true,
                error: None,
            }),
            Err(e) => Ok(McpToolResult {
                tool: tool.to_string(),
                server: server.to_string(),
                content: Value::Null,
                success: false,
                error: Some(e.to_string()),
            }),
        }
    }

    /// Call GitHub MCP tool
    async fn call_github_tool(&self, tool: &str, args: Value) -> Result<Value> {
        // Simulated GitHub API calls
        match tool {
            "create_issue" => {
                let title = args["title"].as_str().unwrap_or("Untitled");
                let body = args["body"].as_str().unwrap_or("");
                Ok(Value::String(format!(
                    "Would create GitHub issue: '{}' - '{}'",
                    title, body
                )))
            }
            "list_issues" => {
                Ok(Value::Array(vec![
                    Value::String("Issue 1: Example bug".to_string()),
                    Value::String("Issue 2: Feature request".to_string()),
                ]))
            }
            "create_pull_request" => {
                let title = args["title"].as_str().unwrap_or("Untitled");
                Ok(Value::String(format!(
                    "Would create PR: '{}'",
                    title
                )))
            }
            _ => Err(anyhow::anyhow!("Unknown GitHub tool: {}", tool)),
        }
    }

    /// Call Linear MCP tool
    async fn call_linear_tool(&self, tool: &str, args: Value) -> Result<Value> {
        // Simulated Linear API calls
        match tool {
            "create_issue" => {
                let title = args["title"].as_str().unwrap_or("Untitled");
                Ok(Value::String(format!(
                    "Would create Linear issue: '{}'",
                    title
                )))
            }
            "list_issues" => {
                Ok(Value::Array(vec![
                    Value::String("LIN-1: Example task".to_string()),
                    Value::String("LIN-2: Bug fix".to_string()),
                ]))
            }
            _ => Err(anyhow::anyhow!("Unknown Linear tool: {}", tool)),
        }
    }

    /// Call Filesystem MCP tool
    async fn call_filesystem_tool(&self, tool: &str, args: Value) -> Result<Value> {
        // 安全修复：验证文件系统操作在项目根目录内
        match tool {
            "read_file" => {
                let path = args["path"].as_str().ok_or_else(|| anyhow::anyhow!("Missing path argument"))?;
                let resolved_path = PathBuf::from(path);

                // 如果配置了项目根目录，验证路径
                if let Some(project_root) = &self.project_root {
                    let canonicalized = resolved_path.canonicalize()
                        .with_context(|| format!("Failed to resolve path: {}", path))?;

                    let canonicalized_root = project_root.canonicalize()
                        .with_context(|| format!("Failed to resolve project root: {}", project_root.display()))?;

                    if !canonicalized.starts_with(&canonicalized_root) {
                        return Err(anyhow::anyhow!(
                            "Path '{}' is outside project root '{}'. Access denied.",
                            path,
                            project_root.display()
                        ));
                    }
                }

                let content = std::fs::read_to_string(&resolved_path)
                    .with_context(|| format!("Failed to read file: {}", path))?;
                Ok(Value::String(content))
            }
            "write_file" => {
                let path = args["path"].as_str().ok_or_else(|| anyhow::anyhow!("Missing path argument"))?;
                let content = args["content"].as_str().ok_or_else(|| anyhow::anyhow!("Missing content argument"))?;
                let resolved_path = PathBuf::from(path);

                // 如果配置了项目根目录，验证路径
                if let Some(project_root) = &self.project_root {
                    // 对于写操作，检查父目录是否存在且在项目内
                    if let Some(parent) = resolved_path.parent() {
                        let canonicalized_parent = parent.canonicalize().ok();
                        let canonicalized_root = project_root.canonicalize().ok();

                        if let (Some(real_parent), Some(real_root)) = (canonicalized_parent, canonicalized_root) {
                            if !real_parent.starts_with(&real_root) {
                                return Err(anyhow::anyhow!(
                                    "Path '{}' is outside project root '{}'. Access denied.",
                                    path,
                                    project_root.display()
                                ));
                            }
                        }
                    }
                }

                std::fs::write(&resolved_path, content)
                    .with_context(|| format!("Failed to write file: {}", path))?;
                Ok(Value::Bool(true))
            }
            "list_dir" => {
                let path = args["path"].as_str().ok_or_else(|| anyhow::anyhow!("Missing path argument"))?;
                let resolved_path = PathBuf::from(path);

                // 如果配置了项目根目录，验证路径
                if let Some(project_root) = &self.project_root {
                    let canonicalized = resolved_path.canonicalize()
                        .with_context(|| format!("Failed to resolve path: {}", path))?;

                    let canonicalized_root = project_root.canonicalize()
                        .with_context(|| format!("Failed to resolve project root: {}", project_root.display()))?;

                    if !canonicalized.starts_with(&canonicalized_root) {
                        return Err(anyhow::anyhow!(
                            "Path '{}' is outside project root '{}'. Access denied.",
                            path,
                            project_root.display()
                        ));
                    }
                }

                let entries: Vec<String> = std::fs::read_dir(&resolved_path)?
                    .filter_map(|e| e.ok())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect();
                Ok(Value::Array(entries.into_iter().map(Value::String).collect()))
            }
            _ => Err(anyhow::anyhow!("Unknown filesystem tool: {}", tool)),
        }
    }

    /// Add a server at runtime
    pub fn add_server(&mut self, name: String, config: McpServerConfig) {
        self.servers.insert(name, config);
    }

    /// Remove a server
    pub fn remove_server(&mut self, name: &str) -> bool {
        self.servers.remove(name).is_some()
    }

    /// Get server config
    pub fn get_server(&self, name: &str) -> Option<&McpServerConfig> {
        self.servers.get(name)
    }

    /// Get all server names
    pub fn get_server_names(&self) -> Vec<&String> {
        self.servers.keys().collect()
    }
}

/// MCP Config file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

/// Create default MCP config for a project
pub fn create_default_mcp_config(project_root: &Path) -> McpConfig {
    let mut servers = HashMap::new();

    // GitHub server
    servers.insert(
        "github".to_string(),
        McpServerConfig {
            name: "GitHub".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@modelcontextprotocol/server-github".to_string()],
            env: HashMap::new(), // GITHUB_TOKEN should be added by user
            tools_include: vec![
                "create_issue".to_string(),
                "list_issues".to_string(),
                "create_pull_request".to_string(),
            ],
            tools_exclude: vec![],
        },
    );

    // Linear server
    servers.insert(
        "linear".to_string(),
        McpServerConfig {
            name: "Linear".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "@modelcontextprotocol/server-linear".to_string()],
            env: HashMap::new(), // User should add LINEAR_API_KEY environment variable
            tools_include: vec![
                "create_issue".to_string(),
                "list_issues".to_string(),
            ],
            tools_exclude: vec![],
        },
    );

    // Filesystem server
    servers.insert(
        "filesystem".to_string(),
        McpServerConfig {
            name: "Filesystem".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                project_root.display().to_string(),
            ],
            env: HashMap::new(),
            tools_include: vec![
                "read_file".to_string(),
                "write_file".to_string(),
                "list_dir".to_string(),
            ],
            tools_exclude: vec![],
        },
    );

    McpConfig {
        mcp_servers: servers,
    }
}

/// Save MCP config to file
pub fn save_mcp_config(config: &McpConfig, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = serde_yaml::to_string(config)
        .with_context(|| "Failed to serialize MCP config to YAML")?;

    std::fs::write(path, content)?;
    Ok(())
}

/// Load or create default MCP config
pub fn load_or_create_mcp_config(project_root: &Path) -> Result<McpClient> {
    let config_path = project_root
        .join(".zero_nine")
        .join("mcp_config.yaml");

    if !config_path.exists() {
        let config = create_default_mcp_config(project_root);
        save_mcp_config(&config, &config_path)?;
        info!("Created default MCP config at: {}", config_path.display());
    }

    McpClient::new(config_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_create_default_config() {
        let config = create_default_mcp_config(Path::new("/tmp/test"));
        assert_eq!(config.mcp_servers.len(), 3);
        assert!(config.mcp_servers.contains_key("github"));
        assert!(config.mcp_servers.contains_key("linear"));
        assert!(config.mcp_servers.contains_key("filesystem"));
    }

    #[test]
    fn test_save_load_config() {
        let tmp_file = temp_dir().join("mcp_test.yaml");
        let _ = std::fs::remove_file(&tmp_file);

        let config = create_default_mcp_config(Path::new("/tmp/test"));
        save_mcp_config(&config, &tmp_file).unwrap();

        let content = std::fs::read_to_string(&tmp_file).unwrap();
        let loaded: McpConfig = serde_yaml::from_str(&content).unwrap();

        assert_eq!(loaded.mcp_servers.len(), 3);

        let _ = std::fs::remove_file(&tmp_file);
    }

    #[tokio::test]
    async fn test_filesystem_tools() {
        let tmp_dir = temp_dir().join("mcp_fs_test");
        let _ = std::fs::create_dir_all(&tmp_dir);

        let client = McpClient::new(temp_dir().join("nonexistent.yaml")).unwrap();

        // Test write
        let write_args = serde_json::json!({
            "path": tmp_dir.join("test.txt").to_str().unwrap(),
            "content": "Hello, World!"
        });
        let result = client.call_filesystem_tool("write_file", write_args).await.unwrap();
        assert_eq!(result, Value::Bool(true));

        // Test read
        let read_args = serde_json::json!({
            "path": tmp_dir.join("test.txt").to_str().unwrap()
        });
        let result = client.call_filesystem_tool("read_file", read_args).await.unwrap();
        assert_eq!(result.as_str().unwrap(), "Hello, World!");

        // Test list
        let list_args = serde_json::json!({
            "path": tmp_dir.to_str().unwrap()
        });
        let result = client.call_filesystem_tool("list_dir", list_args).await.unwrap();
        assert!(result.as_array().unwrap().len() > 0);

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
