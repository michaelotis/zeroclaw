use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Execute a token swap via the orchestrator's DEX integration.
pub struct ExecuteSwapTool {
    config: ClawFoundryConfig,
}

impl ExecuteSwapTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for ExecuteSwapTool {
    fn name(&self) -> &str {
        "execute_swap"
    }

    fn description(&self) -> &str {
        "Execute a token swap on-chain via the orchestrator. Specify the input token, \
         output token, amount, and optional max slippage. The orchestrator enforces \
         guardrails (max 5% slippage, position limits). This is a real financial action \
         — use with caution and only after analyzing the trade."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "tokenIn": {
                    "type": "string",
                    "description": "Address of the token to sell."
                },
                "tokenOut": {
                    "type": "string",
                    "description": "Address of the token to buy."
                },
                "amountIn": {
                    "type": "string",
                    "description": "Amount of tokenIn to swap (in human-readable format, e.g. '0.1')."
                },
                "maxSlippageBps": {
                    "type": "number",
                    "description": "Maximum allowed slippage in basis points (100 = 1%). Default: 100. Max: 500."
                }
            },
            "required": ["tokenIn", "tokenOut", "amountIn"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let token_in = args.get("tokenIn").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("tokenIn is required"))?;
        let token_out = args.get("tokenOut").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("tokenOut is required"))?;
        let amount_in = args.get("amountIn").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("amountIn is required"))?;
        let slippage = args.get("maxSlippageBps").and_then(|v| v.as_u64());

        let mut body = json!({
            "tokenIn": token_in,
            "tokenOut": token_out,
            "amountIn": amount_in,
        });

        if let Some(s) = slippage {
            body["maxSlippageBps"] = json!(s);
        }

        match call_orchestrator(&self.config, "execute_swap", body).await {
            Ok(response) => {
                let success = response["success"].as_bool().unwrap_or(false);
                let data = &response["data"];

                if success {
                    let output = format!(
                        "Swap executed:\n\
                         {} → {}\n\
                         Amount In: {}\n\
                         Status: {}",
                        data["tokenIn"].as_str().unwrap_or("?"),
                        data["tokenOut"].as_str().unwrap_or("?"),
                        data["amountIn"].as_str().unwrap_or("?"),
                        data["status"].as_str().unwrap_or("unknown"),
                    );
                    Ok(ToolResult {
                        success: true,
                        output,
                        error: None,
                    })
                } else {
                    let error = response["error"]
                        .as_str()
                        .unwrap_or("Swap failed")
                        .to_string();
                    Ok(ToolResult {
                        success: false,
                        output: format!("Swap not executed: {error}"),
                        error: Some(error),
                    })
                }
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
            }),
        }
    }
}
