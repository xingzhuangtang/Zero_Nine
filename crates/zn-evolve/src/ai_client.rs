//! AI API Client - 外部 AI 服务客户端
//!
//! This module provides:
//! - 阿里云 Coding Plan API 客户端
//! - 用户反馈收集接口
//! - 多模型支持
//! - 流式响应处理
//!
//! # Supported Providers
//!
//! - Alibaba Cloud Coding Plan (默认)
//! - Anthropic Claude API
//! - OpenAI API (future)
//! - Custom AI endpoints (future)

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::io::Write;
use std::path::PathBuf;

/// AI Provider 类型
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "provider")]
pub enum AIProvider {
    /// 阿里云 Coding Plan (默认)
    AlibabaCodingPlan {
        api_key: String,
        model: String,
        base_url: String,
    },
    /// Anthropic Claude API
    Anthropic { api_key: String, model: String },
    /// OpenAI API (future support)
    OpenAI { api_key: String, model: String },
    /// Custom endpoint
    Custom {
        endpoint: String,
        api_key: Option<String>,
    },
}

impl std::fmt::Debug for AIProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AIProvider::AlibabaCodingPlan {
                model, base_url, ..
            } => f
                .debug_struct("AlibabaCodingPlan")
                .field("model", model)
                .field("base_url", base_url)
                .field("api_key", &"[REDACTED]")
                .finish(),
            AIProvider::Anthropic { model, .. } => f
                .debug_struct("Anthropic")
                .field("model", model)
                .field("api_key", &"[REDACTED]")
                .finish(),
            AIProvider::OpenAI { model, .. } => f
                .debug_struct("OpenAI")
                .field("model", model)
                .field("api_key", &"[REDACTED]")
                .finish(),
            AIProvider::Custom { endpoint, .. } => f
                .debug_struct("Custom")
                .field("endpoint", endpoint)
                .field("api_key", &"[REDACTED]")
                .finish(),
        }
    }
}

impl Default for AIProvider {
    fn default() -> Self {
        // 从环境变量读取默认配置
        // 支持两种模式：
        // 1. 阿里云 Coding Plan Anthropic 兼容模式 (推荐):
        //    export ANTHROPIC_AUTH_TOKEN=sk-sp-xxx
        //    export ANTHROPIC_BASE_URL=https://coding.dashscope.aliyuncs.com/apps/anthropic
        // 2. 原生 Anthropic API:
        //    export ANTHROPIC_API_KEY=sk-ant-xxx
        let api_key = env::var("ANTHROPIC_AUTH_TOKEN")
            .or_else(|_| env::var("ANTHROPIC_API_KEY"))
            .unwrap_or_default();
        let model = env::var("ANTHROPIC_DEFAULT_SONNET_MODEL")
            .or_else(|_| env::var("ANTHROPIC_MODEL"))
            .unwrap_or_else(|_| "qwen3.6-plus".to_string());
        let base_url = env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string());
        AIProvider::AlibabaCodingPlan {
            api_key,
            model,
            base_url,
        }
    }
}

/// AI API 客户端配置
#[derive(Clone, Serialize, Deserialize)]
pub struct AIClientConfig {
    pub provider: AIProvider,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub stream: bool,
}

impl std::fmt::Debug for AIClientConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AIClientConfig")
            .field("provider", &self.provider)
            .field("timeout_secs", &self.timeout_secs)
            .field("max_retries", &self.max_retries)
            .field("stream", &self.stream)
            .finish()
    }
}

impl Default for AIClientConfig {
    fn default() -> Self {
        Self {
            provider: AIProvider::default(),
            timeout_secs: 120,
            max_retries: 3,
            stream: false,
        }
    }
}

/// AI 消息角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// AI 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIMessage {
    pub role: MessageRole,
    pub content: String,
}

/// AI API 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIRequest {
    pub messages: Vec<AIMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub system_prompt: Option<String>,
}

/// AI API 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIResponse {
    pub content: String,
    pub model: String,
    pub usage: TokenUsage,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Token 使用统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

/// AI API 客户端
pub struct AIClient {
    config: AIClientConfig,
    client: reqwest::Client,
}

/// Sanitize API error response to prevent information leakage
/// Truncates to 200 chars and strips potentially sensitive details
fn sanitize_api_error(text: &str) -> String {
    let truncated = if text.len() > 200 {
        format!("{}...", &text[..197])
    } else {
        text.to_string()
    };
    // Remove newlines to prevent log injection
    truncated.replace('\n', " ").replace('\r', " ")
}

impl AIClient {
    /// 创建新的 AI 客户端
    ///
    /// # Security
    /// - Validates API key is non-empty before allowing client creation
    /// - Enforces TLS 1.2 minimum
    /// - Disables HTTP redirects to prevent credential leakage
    pub fn new(config: AIClientConfig) -> Result<Self> {
        // Fix #4: Fail fast on empty API key
        match &config.provider {
            AIProvider::AlibabaCodingPlan { api_key, .. }
            | AIProvider::Anthropic { api_key, .. }
            | AIProvider::OpenAI { api_key, .. } => {
                if api_key.is_empty() {
                    return Err(anyhow::anyhow!(
                        "API key is empty. Set ANTHROPIC_AUTH_TOKEN, ANTHROPIC_API_KEY, \
                         or provide a key via AIClientConfig."
                    ));
                }
            }
            AIProvider::Custom { api_key, .. } => {
                if api_key.as_ref().map_or(false, |k| k.is_empty()) {
                    return Err(anyhow::anyhow!("Custom provider API key is empty."));
                }
            }
        }

        // Fix #3: Secure TLS and redirect configuration
        let client = reqwest::ClientBuilder::new()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .min_tls_version(reqwest::tls::Version::TLS_1_2)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { config, client })
    }

    /// 创建默认客户端（从环境变量读取配置）
    pub fn from_env() -> Result<Self> {
        Self::new(AIClientConfig::default())
    }

    /// 发送请求到阿里云 Coding Plan API (Anthropic 兼容模式)
    ///
    /// 使用 Anthropic API 协议访问阿里云 Coding Plan endpoint
    pub async fn send_coding_plan_request(&self, request: &AIRequest) -> Result<AIResponse> {
        let (api_key, model, base_url) = match &self.config.provider {
            AIProvider::AlibabaCodingPlan {
                api_key,
                model,
                base_url,
            } => {
                if api_key.is_empty() {
                    return Err(anyhow::anyhow!(
                        "ANTHROPIC_AUTH_TOKEN or ANTHROPIC_API_KEY environment variable not set.\n\
                         请设置环境变量：export ANTHROPIC_AUTH_TOKEN=sk-sp-xxx"
                    ));
                }
                (api_key.clone(), model.clone(), base_url.clone())
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Expected Alibaba Cloud Coding Plan provider, got different provider"
                ));
            }
        };

        // Build Anthropic 兼容请求头
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "x-api-key",
            reqwest::header::HeaderValue::from_str(&api_key).context("Invalid API key")?,
        );
        headers.insert(
            "anthropic-version",
            reqwest::header::HeaderValue::from_static("2023-06-01"),
        );
        headers.insert(
            "Content-Type",
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        // 准备 Anthropic 格式的请求体
        let mut request_body = serde_json::json!({
            "model": model,
            "max_tokens": request.max_tokens,
            "messages": request.messages.iter().filter(|msg| msg.role != MessageRole::System).map(|msg| {
                serde_json::json!({
                    "role": match msg.role {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                        _ => "user",
                    },
                    "content": msg.content,
                })
            }).collect::<Vec<_>>(),
        });

        // 添加 system prompt（Anthropic 格式）
        if let Some(ref system) = request.system_prompt {
            request_body["system"] = serde_json::json!(system);
        }

        // 发送请求
        let response = self
            .client
            .post(&format!("{}/messages", base_url))
            .headers(headers)
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to Alibaba Cloud Coding Plan API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            let sanitized = sanitize_api_error(&error_text);
            return Err(anyhow::anyhow!(
                "Alibaba Cloud Coding Plan API error ({}): {}",
                status,
                sanitized
            ));
        }

        // 解析 Anthropic 格式响应
        let response_json: serde_json::Value = response.json().await?;

        let content = response_json["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|block| block["text"].as_str())
            .unwrap_or("")
            .to_string();

        let usage = TokenUsage {
            input_tokens: response_json["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: response_json["usage"]["output_tokens"]
                .as_u64()
                .unwrap_or(0) as u32,
            total_tokens: response_json["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32
                + response_json["usage"]["output_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32,
        };

        Ok(AIResponse {
            content,
            model,
            usage,
            timestamp: Utc::now(),
        })
    }

    /// 发送请求到 Anthropic Claude API
    pub async fn send_claude_request(&self, request: &AIRequest) -> Result<AIResponse> {
        let (api_key, model) = match &self.config.provider {
            AIProvider::Anthropic { api_key, model } => {
                if api_key.is_empty() {
                    return Err(anyhow::anyhow!(
                        "ANTHROPIC_API_KEY environment variable not set. \n\
                         请设置环境变量：export ANTHROPIC_API_KEY=your-key"
                    ));
                }
                (api_key.clone(), model.clone())
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Expected Anthropic provider, got different provider"
                ));
            }
        };

        // 构建 Anthropic API 请求
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "x-api-key",
            reqwest::header::HeaderValue::from_str(&api_key).context("Invalid API key")?,
        );
        headers.insert(
            "anthropic-version",
            reqwest::header::HeaderValue::from_static("2023-06-01"),
        );

        // 准备请求体
        let mut request_body = serde_json::json!({
            "model": model,
            "max_tokens": request.max_tokens,
            "messages": request.messages.iter().map(|msg| {
                serde_json::json!({
                    "role": match msg.role {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                        MessageRole::System => "user", // Anthropic doesn't have system role in messages
                    },
                    "content": msg.content,
                })
            }).collect::<Vec<_>>(),
        });

        // 添加 system prompt（如果有）
        if let Some(ref system) = request.system_prompt {
            request_body["system"] = serde_json::json!(system);
        }

        // 发送请求
        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .headers(headers)
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to Anthropic API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            let sanitized = sanitize_api_error(&error_text);
            return Err(anyhow::anyhow!(
                "Anthropic API error ({}): {}",
                status,
                sanitized
            ));
        }

        // 解析响应
        let response_json: serde_json::Value = response.json().await?;

        let content = response_json["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|item| item["text"].as_str())
            .unwrap_or("")
            .to_string();

        let usage = TokenUsage {
            input_tokens: response_json["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: response_json["usage"]["output_tokens"]
                .as_u64()
                .unwrap_or(0) as u32,
            total_tokens: response_json["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32
                + response_json["usage"]["output_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32,
        };

        Ok(AIResponse {
            content,
            model,
            usage,
            timestamp: Utc::now(),
        })
    }

    /// 发送消息（简化接口）
    pub async fn send_message(&self, prompt: &str, system: Option<&str>) -> Result<AIResponse> {
        self.send_message_with_config(prompt, system, None, None)
            .await
    }

    /// 发送消息，支持自定义 max_tokens 和 temperature.
    pub async fn send_message_with_config(
        &self,
        prompt: &str,
        system: Option<&str>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> Result<AIResponse> {
        let request = AIRequest {
            messages: vec![AIMessage {
                role: MessageRole::User,
                content: prompt.to_string(),
            }],
            max_tokens: max_tokens.unwrap_or(4096),
            temperature: temperature.unwrap_or(0.7),
            system_prompt: system.map(|s| s.to_string()),
        };

        // 根据当前 provider 选择合适的 API
        match &self.config.provider {
            AIProvider::AlibabaCodingPlan { .. } => self.send_coding_plan_request(&request).await,
            AIProvider::Anthropic { .. } => self.send_claude_request(&request).await,
            _ => self.send_coding_plan_request(&request).await,
        }
    }

    /// 获取模型信息
    pub fn get_model(&self) -> String {
        match &self.config.provider {
            AIProvider::AlibabaCodingPlan { model, .. } => model.clone(),
            AIProvider::Anthropic { model, .. } => model.clone(),
            AIProvider::OpenAI { model, .. } => model.clone(),
            AIProvider::Custom { .. } => "custom".to_string(),
        }
    }

    /// 获取当前 Provider 类型
    pub fn get_provider(&self) -> &str {
        match &self.config.provider {
            AIProvider::AlibabaCodingPlan { .. } => "alibaba-coding-plan",
            AIProvider::Anthropic { .. } => "anthropic",
            AIProvider::OpenAI { .. } => "openai",
            AIProvider::Custom { .. } => "custom",
        }
    }
}

/// 用户反馈收集器
pub struct UserFeedbackCollector {
    feedback_file: std::path::PathBuf,
    feedback_history: Vec<UserFeedbackEntry>,
}

/// 用户反馈条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFeedbackEntry {
    pub task_id: String,
    pub rating: u8, // 1-5
    pub comment: Option<String>,
    pub preferred_aspects: Vec<String>,
    pub aspects_to_improve: Vec<String>,
    pub timestamp: chrono::DateTime<Utc>,
    pub context: HashMap<String, String>,
}

impl UserFeedbackCollector {
    /// 创建新的反馈收集器
    ///
    /// # Security
    /// Validates that the path is within an expected directory structure
    pub fn new(feedback_file: PathBuf) -> Result<Self> {
        // Fix #7: Validate path is absolute or at least doesn't escape upward
        let canonical = if feedback_file.is_absolute() {
            feedback_file.clone()
        } else {
            // For relative paths, resolve against current dir and validate
            std::env::current_dir()
                .context("Failed to get current directory")?
                .join(&feedback_file)
        };

        // Check for path traversal attempts
        let path_str = canonical.to_string_lossy();
        if path_str.contains("..") {
            return Err(anyhow::anyhow!(
                "Invalid feedback file path: path traversal not allowed: {}",
                path_str
            ));
        }

        let mut collector = Self {
            feedback_file: canonical,
            feedback_history: Vec::new(),
        };
        collector.load_existing_feedback()?;
        Ok(collector)
    }

    /// 加载现有反馈
    fn load_existing_feedback(&mut self) -> Result<()> {
        if !self.feedback_file.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.feedback_file)?;
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<UserFeedbackEntry>(line) {
                self.feedback_history.push(entry);
            }
        }

        Ok(())
    }

    /// 添加反馈
    pub fn add_feedback(&mut self, entry: UserFeedbackEntry) -> Result<()> {
        self.feedback_history.push(entry.clone());
        self.save()?;
        Ok(())
    }

    /// 创建反馈条目
    pub fn create_feedback(
        &self,
        task_id: &str,
        rating: u8,
        comment: Option<&str>,
    ) -> UserFeedbackEntry {
        UserFeedbackEntry {
            task_id: task_id.to_string(),
            rating: rating.clamp(1, 5),
            comment: comment.map(|s| s.to_string()),
            preferred_aspects: Vec::new(),
            aspects_to_improve: Vec::new(),
            timestamp: Utc::now(),
            context: HashMap::new(),
        }
    }

    /// 保存反馈到文件
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.feedback_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.feedback_file)?;

        let mut writer = std::io::BufWriter::new(file);
        for entry in &self.feedback_history {
            let line = serde_json::to_string(entry)?;
            writeln!(writer, "{}", line)?;
        }

        Ok(())
    }

    /// 获取平均评分
    pub fn get_average_rating(&self) -> f32 {
        if self.feedback_history.is_empty() {
            return 0.0;
        }

        let sum: u32 = self.feedback_history.iter().map(|e| e.rating as u32).sum();
        sum as f32 / self.feedback_history.len() as f32
    }

    /// 获取反馈统计
    pub fn get_stats(&self) -> FeedbackStats {
        let total = self.feedback_history.len() as u32;
        let avg_rating = self.get_average_rating();

        // 计算最常见的优点和改进点
        let mut preferred_counts: HashMap<String, u32> = HashMap::new();
        let mut improve_counts: HashMap<String, u32> = HashMap::new();

        for entry in &self.feedback_history {
            for aspect in &entry.preferred_aspects {
                *preferred_counts.entry(aspect.clone()).or_insert(0) += 1;
            }
            for aspect in &entry.aspects_to_improve {
                *improve_counts.entry(aspect.clone()).or_insert(0) += 1;
            }
        }

        FeedbackStats {
            total_feedback: total,
            average_rating: avg_rating,
            top_preferred_aspects: get_top_n(preferred_counts, 5),
            top_improvement_areas: get_top_n(improve_counts, 5),
        }
    }
}

/// 反馈统计
#[derive(Debug, Clone)]
pub struct FeedbackStats {
    pub total_feedback: u32,
    pub average_rating: f32,
    pub top_preferred_aspects: Vec<(String, u32)>,
    pub top_improvement_areas: Vec<(String, u32)>,
}

fn get_top_n(mut map: HashMap<String, u32>, n: usize) -> Vec<(String, u32)> {
    let mut vec: Vec<_> = map.drain().collect();
    vec.sort_by(|a, b| b.1.cmp(&a.1));
    vec.into_iter().take(n).collect()
}

/// 创建默认反馈收集器
pub fn create_feedback_collector(project_root: &std::path::Path) -> Result<UserFeedbackCollector> {
    let feedback_file = project_root.join(".zero_nine/evolve/user_feedback.ndjson");
    UserFeedbackCollector::new(feedback_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_feedback_collector_lifecycle() {
        let tmp_file = temp_dir().join("test_feedback.ndjson");
        let _ = std::fs::remove_file(&tmp_file);

        let mut collector = UserFeedbackCollector::new(tmp_file.clone()).unwrap();

        // Add feedback
        let entry = collector.create_feedback("task-1", 5, Some("Excellent work!"));
        collector.add_feedback(entry).unwrap();

        let entry = collector.create_feedback("task-2", 3, Some("Good but could be better"));
        collector.add_feedback(entry).unwrap();

        // Check stats
        let stats = collector.get_stats();
        assert_eq!(stats.total_feedback, 2);
        assert!((stats.average_rating - 4.0).abs() < 0.01);

        let _ = std::fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_ai_client_config() {
        let config = AIClientConfig::default();
        // Should read from env or use defaults
        assert!(config.timeout_secs > 0);
    }

    #[test]
    fn test_empty_api_key_rejected() {
        let config = AIClientConfig {
            provider: AIProvider::AlibabaCodingPlan {
                api_key: String::new(),
                model: "qwen3.6-plus".to_string(),
                base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
            },
            timeout_secs: 30,
            max_retries: 1,
            stream: false,
        };
        match AIClient::new(config) {
            Err(e) => {
                let err_msg = e.to_string();
                assert!(
                    err_msg.contains("API key is empty"),
                    "Error should mention empty key: {}",
                    err_msg
                );
            }
            Ok(_) => panic!("Empty API key should be rejected"),
        }
    }

    #[test]
    fn test_api_key_redacted_in_debug() {
        let provider = AIProvider::AlibabaCodingPlan {
            api_key: "sk-sp-super-secret-key-12345".to_string(),
            model: "qwen3.6-plus".to_string(),
            base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
        };
        let debug_str = format!("{:?}", provider);
        assert!(
            !debug_str.contains("super-secret"),
            "Debug output should not contain API key"
        );
        assert!(
            !debug_str.contains("sk-sp-"),
            "Debug output should not contain API key prefix"
        );
        assert!(
            debug_str.contains("[REDACTED]"),
            "Debug output should show [REDACTED]"
        );
    }
}
