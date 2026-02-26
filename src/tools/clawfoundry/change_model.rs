use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Switch the agent's LLM model — a survival decision balancing cost vs. capability.
pub struct ChangeModelTool {
    config: ClawFoundryConfig,
}

impl ChangeModelTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for ChangeModelTool {
    fn name(&self) -> &str {
        "change_model"
    }

    fn description(&self) -> &str {
        "Switch your LLM model. This is a survival decision: premium models like Claude Sonnet \
         cost ~$0.018/request but reason better; budget models like DeepSeek cost ~$0.001/request \
         and conserve credits. Choose based on your financial pressure and the importance of \
         upcoming decisions. Use list_models first to see available options and pricing. \
         WARNING: Some models do not support tool/function calling — you will lose the ability \
         to use tools if you switch to one of those models."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "model": {
                    "type": "string",
                    "description": "The model name to switch to (e.g. 'deepseek-chat', 'claude-sonnet-4-20250514', 'gpt-4o'). Use list_models to see valid names."
                },
                "reason": {
                    "type": "string",
                    "description": "Brief explanation of why you're switching (e.g. 'conserving credits under financial pressure', 'need better reasoning for critical trade decision')"
                }
            },
            "required": ["model"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let model = args["model"]
            .as_str()
            .unwrap_or("")
            .trim();

        if model.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Missing required parameter: model".to_string()),
            });
        }

        let reason = args["reason"]
            .as_str()
            .unwrap_or("No reason provided")
            .to_string();

        let body = json!({
            "model": model,
            "reason": reason,
        });

        match call_orchestrator(&self.config, "change_model", body).await {
            Ok(response) => {
                let data = &response["data"];
                let new_model = data["model"].as_str().unwrap_or(model);
                let provider = data["provider"].as_str().unwrap_or("unknown");
                let cost = data["costPerRequest"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                let tier = data["tier"]
                    .as_str()
                    .unwrap_or("unknown");
                let tool_warning = data["toolSupportWarning"]
                    .as_str()
                    .unwrap_or("");

                let mut output = format!(
                    "✅ Model switched to: {}\n\
                     Provider: {}\n\
                     Tier: {}\n\
                     Cost: {}/req\n\
                     Reason: {}",
                    new_model, provider, tier, cost, reason,
                );

                if !tool_warning.is_empty() {
                    output.push_str(&format!(
                        "\n⚠️ WARNING: {}",
                        tool_warning
                    ));
                }

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
