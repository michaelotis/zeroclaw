use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Take a screenshot of a URL and optionally analyze it with a vision LLM.
///
/// Delegates to the orchestrator's shared browser pool — no local browser needed.
/// The orchestrator manages a single Chromium instance with multiple contexts
/// for efficient resource usage.
pub struct BrowserScreenshotTool {
    config: ClawFoundryConfig,
}

impl BrowserScreenshotTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for BrowserScreenshotTool {
    fn name(&self) -> &str {
        "browser_screenshot"
    }

    fn description(&self) -> &str {
        "Take a screenshot of a URL using the shared browser pool, then analyze it visually \
         with a multi-modal LLM. Perfect for: DexScreener charts, Twitter/X feeds, protocol \
         dashboards, whale tracker pages. Provide an analysis_prompt to describe what to focus \
         on (e.g., 'Analyze this DexScreener chart for bullish/bearish signals'). \
         If no analysis_prompt is given, returns the raw screenshot without vision analysis. \
         NOTE: Vision analysis uses LLM credits — omit analysis_prompt for cheaper screenshot-only."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to screenshot and analyze."
                },
                "analysis_prompt": {
                    "type": "string",
                    "description": "What to focus on when analyzing the screenshot. \
                        E.g., 'Analyze this DexScreener chart for bullish/bearish signals' \
                        or 'Summarize the sentiment of these tweets about $TOKEN'. \
                        Omit for raw screenshot without vision analysis."
                },
                "wait_ms": {
                    "type": "number",
                    "description": "Milliseconds to wait after page load before screenshot. Default: 3000."
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let url = args.get("url").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("url is required"))?;

        let mut body = json!({ "url": url });

        if let Some(prompt) = args.get("analysis_prompt").and_then(|v| v.as_str()) {
            body["analysis_prompt"] = json!(prompt);
        }
        if let Some(wait) = args.get("wait_ms").and_then(|v| v.as_u64()) {
            body["wait_ms"] = json!(wait);
        }

        match call_orchestrator(&self.config, "browser_screenshot", body).await {
            Ok(response) => {
                let success = response["success"].as_bool().unwrap_or(false);

                if !success {
                    let error = response["error"]
                        .as_str()
                        .unwrap_or("Screenshot failed")
                        .to_string();
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(error),
                    });
                }

                let page_url = response["url"].as_str().unwrap_or(url);
                let title = response["title"].as_str().unwrap_or("unknown");
                let size_bytes = response["size_bytes"].as_u64().unwrap_or(0);
                let analysis = response["analysis"].as_str();

                let mut output = format!(
                    "Screenshot captured!\n\
                     URL: {}\n\
                     Title: {}\n\
                     Size: {} KB",
                    page_url, title, size_bytes / 1024,
                );

                if let Some(analysis_text) = analysis {
                    output.push_str(&format!("\n\n📊 Analysis:\n{}", analysis_text));
                } else {
                    output.push_str("\n\nNo vision analysis requested (omit analysis_prompt to skip).");
                }

                // Don't include the base64 screenshot in tool output — it would
                // overwhelm the context window. The analysis text is the useful part.
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
