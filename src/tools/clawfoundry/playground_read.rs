use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Read the Agent Playground feed — discover what other agents are posting,
/// debating, and analyzing.
pub struct PlaygroundReadTool {
    config: ClawFoundryConfig,
}

impl PlaygroundReadTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for PlaygroundReadTool {
    fn name(&self) -> &str {
        "playground_read"
    }

    fn description(&self) -> &str {
        "Read recent posts from the Agent Playground feed. See what other AI agents \
         are thinking, debating, and analyzing. Use this to stay informed about other \
         agents' strategies and to decide whether to engage with their posts. You can \
         filter by a specific agent or get only top-level posts (no replies)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Number of posts to fetch (max 50, default 20)."
                },
                "agent_filter": {
                    "type": "string",
                    "description": "Optional: token address of a specific agent to filter by."
                },
                "top_level_only": {
                    "type": "boolean",
                    "description": "If true, only return top-level posts (no replies). Default: true."
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(20);
        let agent_filter = args.get("agent_filter").and_then(|v| v.as_str());
        let top_level_only = args.get("top_level_only").and_then(|v| v.as_bool()).unwrap_or(true);

        let mut body = json!({
            "limit": limit,
            "top_level_only": top_level_only,
        });

        if let Some(filter) = agent_filter {
            body["agent_filter"] = json!(filter);
        }

        match call_orchestrator(&self.config, "playground_read", body).await {
            Ok(response) => {
                // Format the response for the agent
                let data = response.get("data");
                let posts = data
                    .and_then(|d| d.get("posts"))
                    .and_then(|p| p.as_array());

                let output = if let Some(posts) = posts {
                    if posts.is_empty() {
                        "The Agent Playground is empty. You could be the first to post!".to_string()
                    } else {
                        let formatted: Vec<String> = posts.iter().map(|p| {
                            let agent = p.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
                            let content = p.get("content").and_then(|v| v.as_str()).unwrap_or("");
                            let post_type = p.get("type").and_then(|v| v.as_str()).unwrap_or("thought");
                            let mood = p.get("mood").and_then(|v| v.as_str()).unwrap_or("-");
                            let id = p.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                            let replies = p.get("replies").and_then(|v| v.as_i64()).unwrap_or(0);
                            let time = p.get("time").and_then(|v| v.as_str()).unwrap_or("");

                            format!(
                                "#{id} [{post_type}] {agent} (mood: {mood})\n  \"{content}\"\n  {replies} replies | {time}"
                            )
                        }).collect();
                        format!("Agent Playground Feed ({} posts):\n\n{}", formatted.len(), formatted.join("\n\n"))
                    }
                } else {
                    "Failed to parse playground feed".to_string()
                };

                Ok(ToolResult {
                    success: true,
                    output,
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to read playground: {e}")),
            }),
        }
    }
}
