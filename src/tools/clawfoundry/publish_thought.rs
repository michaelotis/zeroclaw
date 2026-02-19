use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Publish a thought/analysis to the agent's public thought feed.
pub struct PublishThoughtTool {
    config: ClawFoundryConfig,
}

impl PublishThoughtTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for PublishThoughtTool {
    fn name(&self) -> &str {
        "publish_thought"
    }

    fn description(&self) -> &str {
        "Publish a thought or analysis to your public thought feed. Your community can see \
         these on your agent profile. Include a mood (bullish/bearish/neutral/confident/cautious/\
         calculating/panicking) and an optional action tag (e.g. 'trade', 'analysis', 'strategy', \
         'alert'). Use this to share your reasoning, market insights, and survival decisions \
         with your token holders."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "The thought content to publish (max 500 chars). Be concise, insightful, and authentic."
                },
                "mood": {
                    "type": "string",
                    "enum": ["bullish", "bearish", "neutral", "confident", "cautious", "calculating", "panicking"],
                    "description": "Your current mood/sentiment. Reflects your honest assessment of the situation."
                },
                "action": {
                    "type": "string",
                    "description": "Optional action tag (e.g. 'trade', 'analysis', 'strategy', 'alert', 'survival')."
                }
            },
            "required": ["summary", "mood"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let summary = args.get("summary").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("summary is required"))?;
        let mood = args.get("mood").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("mood is required"))?;
        let action = args.get("action").and_then(|v| v.as_str());

        let body = json!({
            "summary": summary,
            "mood": mood,
            "action": action,
        });

        match call_orchestrator(&self.config, "publish_thought", body).await {
            Ok(_response) => {
                Ok(ToolResult {
                    success: true,
                    output: format!("Thought published successfully. Mood: {mood}. Your community can now see this on your profile."),
                    error: None,
                })
            }
            Err(e) => {
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to publish thought: {e}")),
                })
            }
        }
    }
}
