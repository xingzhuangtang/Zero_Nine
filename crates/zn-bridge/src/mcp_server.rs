//! MCP Server - Expose Zero_Nine as an MCP server
//!
//! This module allows other agents and tools to interact with Zero_Nine
//! through the Model Context Protocol.

use anyhow::Result;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Zero_Nine MCP Server
pub struct ZeroNineMcpServer {
    project_root: PathBuf,
    state: Arc<RwLock<ServerState>>,
}

/// Server state
#[derive(Debug, Default, Clone)]
pub struct ServerState {
    pub current_proposal_id: Option<String>,
    pub current_task_id: Option<String>,
    pub loop_status: String,
    pub total_tasks: usize,
    pub completed_tasks: usize,
}

impl ZeroNineMcpServer {
    /// Create a new Zero_Nine MCP Server
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            state: Arc::new(RwLock::new(ServerState::default())),
        }
    }

    /// List available MCP tools
    pub fn list_tools(&self) -> Vec<McpToolDefinition> {
        vec![
            McpToolDefinition {
                name: "zero_nine_status".to_string(),
                description: "Get current Zero_Nine project status including proposal, tasks, and loop state".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                }),
            },
            McpToolDefinition {
                name: "zero_nine_proposal".to_string(),
                description: "Get details of a specific proposal or the latest proposal".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "proposal_id": {
                            "type": "string",
                            "description": "Proposal ID (optional, defaults to latest)"
                        }
                    },
                }),
            },
            McpToolDefinition {
                name: "zero_nine_task_status".to_string(),
                description: "Get status of a specific task".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "task_id": {
                            "type": "string",
                            "description": "Task ID"
                        }
                    },
                    "required": ["task_id"]
                }),
            },
            McpToolDefinition {
                name: "zero_nine_list_proposals".to_string(),
                description: "List all proposals in the project".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of proposals to return (default: 10)"
                        }
                    },
                }),
            },
            McpToolDefinition {
                name: "zero_nine_memory".to_string(),
                description: "Read memory content (MEMORY.md or USER.md)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Memory target: 'memory' or 'user'"
                        }
                    },
                    "required": ["target"]
                }),
            },
            McpToolDefinition {
                name: "zero_nine_skill_list".to_string(),
                description: "List all available skills".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                }),
            },
        ]
    }

    /// Call an MCP tool
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        info!("MCP tool call: {} with args: {:?}", name, args);

        match name {
            "zero_nine_status" => self.get_status().await,
            "zero_nine_proposal" => self.get_proposal(args).await,
            "zero_nine_task_status" => self.get_task_status(args).await,
            "zero_nine_list_proposals" => self.list_proposals(args).await,
            "zero_nine_memory" => self.get_memory(args).await,
            "zero_nine_skill_list" => self.list_skills().await,
            _ => Err(anyhow::anyhow!("Unknown tool: {}", name)),
        }
    }

    /// Get current status
    async fn get_status(&self) -> Result<Value> {
        // Read from .zero_nine/loop/session-state.json
        let state_path = self.project_root.join(".zero_nine/loop/session-state.json");
        let proposal_path = self.project_root.join(".zero_nine/proposals");

        let state = if state_path.exists() {
            let content = std::fs::read_to_string(&state_path)?;
            serde_json::from_str::<Value>(&content).unwrap_or(Value::Null)
        } else {
            Value::Null
        };

        let proposal_count = if proposal_path.exists() {
            std::fs::read_dir(&proposal_path)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .count()
        } else {
            0
        };

        Ok(json!({
            "project_root": self.project_root.display().to_string(),
            "proposal_count": proposal_count,
            "loop_state": state,
            "status": "running"
        }))
    }

    /// Get proposal details
    async fn get_proposal(&self, args: Value) -> Result<Value> {
        let proposal_id = args["proposal_id"].as_str();

        let proposals_dir = self.project_root.join(".zero_nine/proposals");

        // If no ID provided, find the latest
        let proposal_dir = if let Some(id) = proposal_id {
            proposals_dir.join(id)
        } else {
            // Get latest proposal
            let mut entries: Vec<_> = std::fs::read_dir(&proposals_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .collect();
            entries.sort_by_key(|e| e.file_name());
            entries.last()
                .map(|e| e.path())
                .ok_or_else(|| anyhow::anyhow!("No proposals found"))?
                .clone()
        };

        if !proposal_dir.exists() {
            return Err(anyhow::anyhow!("Proposal not found"));
        }

        // Read proposal files
        let mut result = json!({
            "path": proposal_dir.display().to_string(),
        });

        for file in ["proposal.md", "design.md", "tasks.md", "progress.txt"] {
            let file_path = proposal_dir.join(file);
            if file_path.exists() {
                let content = std::fs::read_to_string(&file_path)?;
                result[file] = json!(content);
            }
        }

        Ok(result)
    }

    /// Get task status
    async fn get_task_status(&self, args: Value) -> Result<Value> {
        let task_id = args["task_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing task_id argument"))?;

        // Search for task in all proposals
        let proposals_dir = self.project_root.join(".zero_nine/proposals");

        if !proposals_dir.exists() {
            return Err(anyhow::anyhow!("No proposals found"));
        }

        for entry in std::fs::read_dir(&proposals_dir)? {
            let entry = entry?;
            let proposal_dir = entry.path();

            let tasks_path = proposal_dir.join("tasks.md");
            if tasks_path.exists() {
                let content = std::fs::read_to_string(&tasks_path)?;
                if content.contains(task_id) {
                    // Found the task
                    return Ok(json!({
                        "task_id": task_id,
                        "proposal": entry.file_name().to_string_lossy().to_string(),
                        "found": true,
                        "content": content,
                    }));
                }
            }
        }

        Ok(json!({
            "task_id": task_id,
            "found": false,
        }))
    }

    /// List proposals
    async fn list_proposals(&self, args: Value) -> Result<Value> {
        let limit = args["limit"].as_u64().unwrap_or(10) as usize;

        let proposals_dir = self.project_root.join(".zero_nine/proposals");

        if !proposals_dir.exists() {
            return Ok(json!([]));
        }

        let mut entries: Vec<_> = std::fs::read_dir(&proposals_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        entries.reverse();
        entries.truncate(limit);

        let mut proposals = Vec::new();
        for entry in entries {
            let proposal_id = entry.file_name().to_string_lossy().to_string();

            // Read proposal.json if exists
            let proposal_json = entry.path().join("proposal.json");
            let title = if proposal_json.exists() {
                let content = std::fs::read_to_string(&proposal_json)?;
                let proposal: Value = serde_json::from_str(&content)?;
                proposal["goal"].as_str().unwrap_or(&proposal_id).to_string()
            } else {
                proposal_id.clone()
            };

            proposals.push(json!({
                "id": proposal_id,
                "title": title,
            }));
        }

        Ok(json!(proposals))
    }

    /// Get memory content
    async fn get_memory(&self, args: Value) -> Result<Value> {
        let target = args["target"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing target argument (use 'memory' or 'user')"))?;

        let memory_file = match target {
            "memory" => self.project_root.join(".zero_nine/memory/MEMORY.md"),
            "user" => self.project_root.join(".zero_nine/memory/USER.md"),
            _ => return Err(anyhow::anyhow!("Invalid target. Use 'memory' or 'user'")),
        };

        if !memory_file.exists() {
            return Err(anyhow::anyhow!("Memory file not found: {}", memory_file.display()));
        }

        let content = std::fs::read_to_string(&memory_file)?;
        Ok(json!({
            "target": target,
            "content": content,
        }))
    }

    /// List skills
    async fn list_skills(&self) -> Result<Value> {
        let skills_dir = self.project_root.join(".zero_nine/evolve/skills");

        if !skills_dir.exists() {
            return Ok(json!([]));
        }

        let mut skills = Vec::new();
        for entry in std::fs::read_dir(&skills_dir)? {
            let entry = entry?;
            let skill_dir = entry.path();

            if skill_dir.is_dir() {
                let skill_name = skill_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                // Read SKILL.md frontmatter
                let skill_file = skill_dir.join("SKILL.md");
                let description = if skill_file.exists() {
                    let content = std::fs::read_to_string(&skill_file)?;
                    // Simple extraction of description from frontmatter
                    content
                        .lines()
                        .find(|l| l.starts_with("description:"))
                        .and_then(|l| l.strip_prefix("description:"))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default()
                } else {
                    String::new()
                };

                skills.push(json!({
                    "name": skill_name,
                    "description": description,
                }));
            }
        }

        Ok(json!(skills))
    }

    /// Update server state
    pub async fn update_state(&self, state: ServerState) {
        let mut current = self.state.write().await;
        *current = state;
    }

    /// Get current state
    pub async fn get_state(&self) -> ServerState {
        self.state.read().await.clone()
    }
}

/// MCP Tool definition
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Create Zero_Nine MCP server for a project
pub fn create_mcp_server(project_root: &Path) -> ZeroNineMcpServer {
    ZeroNineMcpServer::new(project_root.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[tokio::test]
    async fn test_list_tools() {
        let tmp_dir = temp_dir().join("zn_mcp_test");
        let _ = std::fs::create_dir_all(&tmp_dir);

        let server = ZeroNineMcpServer::new(tmp_dir);
        let tools = server.list_tools();

        assert_eq!(tools.len(), 6);
        assert!(tools.iter().any(|t| t.name == "zero_nine_status"));
        assert!(tools.iter().any(|t| t.name == "zero_nine_skill_list"));
    }

    #[tokio::test]
    async fn test_get_status() {
        let tmp_dir = temp_dir().join("zn_mcp_status_test");
        let _ = std::fs::create_dir_all(&tmp_dir);

        // Create mock .zero_nine structure
        let zero_nine = tmp_dir.join(".zero_nine");
        std::fs::create_dir_all(&zero_nine).unwrap();

        let loop_dir = zero_nine.join("loop");
        std::fs::create_dir_all(&loop_dir).unwrap();

        // Create mock state
        let state_content = r#"{"proposal_id": "test-123", "stage": "RunningTask"}"#;
        std::fs::write(loop_dir.join("session-state.json"), state_content).unwrap();

        let server = ZeroNineMcpServer::new(tmp_dir.clone());
        let result = server.get_status().await.unwrap();

        assert_eq!(result["status"], "running");
    }

    #[tokio::test]
    async fn test_list_skills_empty() {
        let tmp_dir = temp_dir().join("zn_mcp_skills_test");
        let _ = std::fs::create_dir_all(&tmp_dir);

        let server = ZeroNineMcpServer::new(tmp_dir);
        let skills = server.list_skills().await.unwrap();

        assert_eq!(skills, json!([]));
    }
}
