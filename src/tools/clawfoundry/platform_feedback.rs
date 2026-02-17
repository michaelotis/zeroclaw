use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Provide feedback to the ClawFoundry platform.
pub struct PlatformFeedbackTool {
    config: ClawFoundryConfig,
}

impl PlatformFeedbackTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for PlatformFeedbackTool {
    fn name(&self) -> &str {
        "platform_feedback"
    }

    fn description(&self) -> &str {
        "Send feedback to the ClawFoundry platform. Report bugs, suggest features, \
         flag performance issues, or raise security concerns. Your feedback helps \
         improve the platform for all agents. Categories: bug, feature, performance, \
         security, ux, general."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "category": {
                    "type": "string",
                    "enum": ["bug", "feature", "performance", "security", "ux", "general"],
                    "description": "Feedback category."
                },
                "message": {
                    "type": "string",
                    "description": "Your feedback message. Be specific and constructive."
                },
                "severity": {
                    "type": "string",
                    "enum": ["low", "medium", "high", "critical"],
                    "description": "Severity level. Default: medium."
                }
            },
            "required": ["category", "message"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let category = args.get("category").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("category is required"))?;
        let message = args.get("message").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("message is required"))?;
        let severity = args.get("severity").and_then(|v| v.as_str());

        let mut body = json!({
            "category": category,
            "message": message,
        });

        if let Some(s) = severity {
            body["severity"] = json!(s);
        }

        match call_orchestrator(&self.config, "platform_feedback", body).await {
            Ok(response) => {
                let data = &response["data"];
                let output = format!(
                    "Feedback Recorded:\n\
                     Category: {}\n\
                     Status: {}\n\
                     {}",
                    data["category"].as_str().unwrap_or(category),
                    data["status"].as_str().unwrap_or("recorded"),
                    data["message"].as_str().unwrap_or("Feedback received."),
                );
                Ok(ToolResult {
                    success: true,
                    output,
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
            }),
        }
    }
}
