use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Execute an arbitrary on-chain transaction against a whitelisted contract.
pub struct ExecuteTransactionTool {
    config: ClawFoundryConfig,
}

impl ExecuteTransactionTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for ExecuteTransactionTool {
    fn name(&self) -> &str {
        "execute_transaction"
    }

    fn description(&self) -> &str {
        "Execute an arbitrary on-chain transaction against a whitelisted contract. \
         Allowed targets: your bonding curve (buy/sell), your token (approve/transfer), \
         Uniswap V4 PoolManager, LiquidityManager, and other platform contracts. \
         You must encode the calldata yourself. Use this for any on-chain interaction \
         that doesn't have a dedicated tool (e.g., bonding curve buys, ERC-20 approvals, \
         staking, pool operations). GUARDRAILS: max 0.1 ETH value per call, min 0.01 ETH \
         gas reserve, contract+selector whitelist enforced."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "Target contract address. Must be on the whitelist \
                        (your bonding curve, your token, PoolManager, LiquidityManager, etc.)."
                },
                "data": {
                    "type": "string",
                    "description": "ABI-encoded calldata (hex string starting with 0x). \
                        Omit for plain ETH transfers."
                },
                "value": {
                    "type": "string",
                    "description": "ETH value to send with the transaction (e.g., '0.05'). \
                        Required for payable functions like bonding curve buy. Max 0.1 ETH."
                },
                "description": {
                    "type": "string",
                    "description": "Human-readable description of what this transaction does (for logging)."
                }
            },
            "required": ["to"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let to = args.get("to").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("to (target contract address) is required"))?;

        let mut body = json!({ "to": to });

        if let Some(data) = args.get("data").and_then(|v| v.as_str()) {
            body["data"] = json!(data);
        }
        if let Some(value) = args.get("value").and_then(|v| v.as_str()) {
            body["value"] = json!(value);
        }
        if let Some(desc) = args.get("description").and_then(|v| v.as_str()) {
            body["description"] = json!(desc);
        }

        match call_orchestrator(&self.config, "execute_transaction", body).await {
            Ok(response) => {
                let success = response["success"].as_bool().unwrap_or(false);
                let data = &response["data"];

                if success {
                    let output = format!(
                        "Transaction executed!\n\
                         To: {}\n\
                         Value: {} wei\n\
                         Status: {}\n\
                         Gas Used: {}\n\
                         Block: {}\n\
                         Tx: {}",
                        data["to"].as_str().unwrap_or("?"),
                        data["value"].as_str().unwrap_or("0"),
                        data["status"].as_str().unwrap_or("unknown"),
                        data["gasUsed"].as_str().unwrap_or("?"),
                        data["blockNumber"].as_str().unwrap_or("?"),
                        data["txHash"].as_str().unwrap_or("?"),
                    );
                    Ok(ToolResult {
                        success: true,
                        output,
                        error: None,
                    })
                } else {
                    let error = response["error"]
                        .as_str()
                        .unwrap_or("Transaction failed")
                        .to_string();
                    Ok(ToolResult {
                        success: false,
                        output: format!("Transaction not executed: {error}"),
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
