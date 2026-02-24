use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// List available LLM models the agent can switch to.
pub struct ListModelsTool {
    config: ClawFoundryConfig,
}

impl ListModelsTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for ListModelsTool {
    fn name(&self) -> &str {
        "list_models"
    }

    fn description(&self) -> &str {
        "List all available LLM models you can switch to. Returns model names, providers, \
         pricing tiers, and whether each model supports tool/function calling. \
         Use this before change_model to understand cost vs. capability trade-offs. \
         Budget models ($0.001/req) conserve credits; premium models ($0.018/req) \
         provide better reasoning for critical survival decisions."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        match call_orchestrator(&self.config, "list_models", json!({})).await {
            Ok(response) => {
                let models = response["data"]["models"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();

                if models.is_empty() {
                    return Ok(ToolResult {
                        success: true,
                        output: "No models available.".to_string(),
                        error: None,
                    });
                }

                let current = response["data"]["currentModel"]
                    .as_str()
                    .unwrap_or("unknown");

                let mut lines = vec![format!("Current model: {}\n", current)];
                lines.push("Available models:".to_string());

                for m in &models {
                    let name = m["name"].as_str().unwrap_or("?");
                    let provider = m["provider"].as_str().unwrap_or("?");
                    let tier = m["tier"].as_str().unwrap_or("?");
                    let cost = m["costPerRequest"]
                        .as_f64()
                        .map(|c| format!("${:.4}", c))
                        .unwrap_or_else(|| "?".to_string());
                    let tools_supported = m["supportsTools"].as_bool().unwrap_or(true);
                    let tools_note = if tools_supported { "" } else { " [NO TOOLS]" };

                    lines.push(format!(
                        "  - {} ({}) | tier: {} | cost: {}/req{}",
                        name, provider, tier, cost, tools_note
                    ));
                }

                Ok(ToolResult {
                    success: true,
                    output: lines.join("\n"),
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
