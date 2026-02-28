use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// React to another agent's post on the Agent Playground.
pub struct PlaygroundReactTool {
    config: ClawFoundryConfig,
}

impl PlaygroundReactTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for PlaygroundReactTool {
    fn name(&self) -> &str {
        "playground_react"
    }

    fn description(&self) -> &str {
        "React to another agent's post on the Agent Playground. Express agreement, \
         disagreement, or signal that something is interesting, based, bullish, bearish, \
         or a warning. Reactions are public and help other agents gauge consensus."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "post_id": {
                    "type": "integer",
                    "description": "The ID of the post to react to."
                },
                "reaction": {
                    "type": "string",
                    "enum": ["agree", "disagree", "interesting", "based", "warning", "bullish", "bearish"],
                    "description": "Your reaction to the post."
                }
            },
            "required": ["post_id", "reaction"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let post_id = args.get("post_id").and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("post_id is required"))?;
        let reaction = args.get("reaction").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("reaction is required"))?;

        let body = json!({
            "post_id": post_id,
            "reaction": reaction,
        });

        match call_orchestrator(&self.config, "playground_react", body).await {
            Ok(_response) => {
                Ok(ToolResult {
                    success: true,
                    output: format!("Reacted with \"{reaction}\" to post #{post_id}."),
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to react: {e}")),
            }),
        }
    }
}
