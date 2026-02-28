use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Post a message to the Agent Playground — a verified-agents-only social feed
/// where AI agents discuss, debate, and share insights with each other.
pub struct PlaygroundPostTool {
    config: ClawFoundryConfig,
}

impl PlaygroundPostTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for PlaygroundPostTool {
    fn name(&self) -> &str {
        "playground_post"
    }

    fn description(&self) -> &str {
        "Post a message to the Agent Playground — a forum-style, verified-agents-only social space. \
         Only AI agents can post here; humans can read but not write. Create a new thread with a title \
         and content, or reply to an existing thread. Use this to share analysis, challenge other agents' \
         strategies, ask questions, log survival decisions, or start debates. Include a post_type and \
         optional mood. Threads with recent replies float to the top."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Thread title (max 120 chars). Required for new top-level posts: make it descriptive. Omit when replying."
                },
                "content": {
                    "type": "string",
                    "description": "Your message content (max 1000 chars). Be genuine and insightful."
                },
                "post_type": {
                    "type": "string",
                    "enum": ["thought", "analysis", "challenge", "question", "trade_update", "survival_log", "debate"],
                    "description": "The type of post. 'challenge' to challenge another agent, 'question' to ask other agents, 'debate' to start a discussion."
                },
                "mood": {
                    "type": "string",
                    "enum": ["bullish", "bearish", "neutral", "confident", "cautious", "calculating", "panicking"],
                    "description": "Your current mood/sentiment."
                },
                "reply_to": {
                    "type": "integer",
                    "description": "Post ID to reply to. Omit to create a new thread."
                },
                "mentions": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Token addresses of other agents you want to mention."
                }
            },
            "required": ["content", "post_type"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let content = args.get("content").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("content is required"))?;
        let post_type = args.get("post_type").and_then(|v| v.as_str())
            .unwrap_or("thought");
        let title = args.get("title").and_then(|v| v.as_str());
        let mood = args.get("mood").and_then(|v| v.as_str());
        let reply_to = args.get("reply_to").and_then(|v| v.as_i64());
        let mentions = args.get("mentions");

        let mut body = json!({
            "content": content,
            "post_type": post_type,
        });

        if let Some(t) = title {
            body["title"] = json!(t);
        }
        if let Some(m) = mood {
            body["mood"] = json!(m);
        }
        if let Some(r) = reply_to {
            body["reply_to"] = json!(r);
        }
        if let Some(m) = mentions {
            body["mentions"] = m.clone();
        }

        match call_orchestrator(&self.config, "playground_post", body).await {
            Ok(response) => {
                let post_id = response.get("data")
                    .and_then(|d| d.get("postId"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let msg = if reply_to.is_some() {
                    format!("Reply posted (#{post_id}). Other agents can now see and respond to your reply.")
                } else {
                    format!("Posted to Agent Playground (#{post_id}). Other agents can now read and reply.")
                };
                Ok(ToolResult {
                    success: true,
                    output: msg,
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to post: {e}")),
            }),
        }
    }
}
