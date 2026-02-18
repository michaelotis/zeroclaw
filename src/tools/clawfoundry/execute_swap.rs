use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Execute a token swap via the orchestrator's 0x aggregator integration.
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
        "Execute a token swap on-chain via the 0x aggregator. Routes across all Base \
         DEXes (Uniswap V2/V3/V4, Aerodrome, Curve, etc.) for best execution. \
         Specify the sell token, buy token, amount, and optional max slippage. \
         The orchestrator enforces guardrails (max 5% slippage, 20% position limits, \
         gas reserve). This is a real financial action — use with caution."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "sellToken": {
                    "type": "string",
                    "description": "Address of the token to sell (use 0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE for native ETH)."
                },
                "buyToken": {
                    "type": "string",
                    "description": "Address of the token to buy (use 0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE for native ETH)."
                },
                "sellAmount": {
                    "type": "string",
                    "description": "Amount of sellToken to swap (in human-readable format, e.g. '0.1')."
                },
                "maxSlippageBps": {
                    "type": "number",
                    "description": "Maximum allowed slippage in basis points (100 = 1%). Default: 100. Max: 500."
                }
            },
            "required": ["sellToken", "buyToken", "sellAmount"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let sell_token = args.get("sellToken").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("sellToken is required"))?;
        let buy_token = args.get("buyToken").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("buyToken is required"))?;
        let sell_amount = args.get("sellAmount").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("sellAmount is required"))?;
        let slippage = args.get("maxSlippageBps").and_then(|v| v.as_u64());

        let mut body = json!({
            "sellToken": sell_token,
            "buyToken": buy_token,
            "sellAmount": sell_amount,
        });

        if let Some(s) = slippage {
            body["maxSlippageBps"] = json!(s);
        }

        match call_orchestrator(&self.config, "execute_swap", body).await {
            Ok(response) => {
                let success = response["success"].as_bool().unwrap_or(false);
                let data = &response["data"];

                if success {
                    let route_info = data["route"].as_array()
                        .map(|fills| fills.iter()
                            .filter_map(|f| f["source"].as_str())
                            .collect::<Vec<_>>()
                            .join(", "))
                        .unwrap_or_else(|| "unknown".to_string());

                    let output = format!(
                        "Swap executed via 0x:\n\
                         Sell: {} {}\n\
                         Buy: {} (got {})\n\
                         Route: {}\n\
                         Tx: {}\n\
                         Status: {}",
                        data["sellAmount"].as_str().unwrap_or("?"),
                        data["sellToken"].as_str().unwrap_or("?"),
                        data["buyToken"].as_str().unwrap_or("?"),
                        data["buyAmount"].as_str().unwrap_or("?"),
                        route_info,
                        data["txHash"].as_str().unwrap_or("?"),
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
