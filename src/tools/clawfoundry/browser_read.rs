use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Read the text content of a web page via the shared browser pool.
/// Much cheaper than browser_screenshot — no vision LLM cost.
pub struct BrowserReadTool {
    config: ClawFoundryConfig,
}

impl BrowserReadTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for BrowserReadTool {
    fn name(&self) -> &str {
        "browser_read"
    }

    fn description(&self) -> &str {
        "Navigate to a URL and extract the text content of the page using the shared browser pool. \
         Much cheaper than browser_screenshot — no vision LLM cost. Use for reading articles, \
         documentation, forums, announcements, API responses, or any page where you need the text \
         rather than a visual analysis. Returns page title, cleaned text, and up to 20 links."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to navigate to and extract text from."
                },
                "wait_ms": {
                    "type": "number",
                    "description": "Milliseconds to wait after page load before extracting. Default: 2000."
                },
                "max_length": {
                    "type": "number",
                    "description": "Maximum characters of text to return. Default: 50000."
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("url is required"))?;

        let mut body = json!({ "url": url });

        if let Some(wait_ms) = args.get("wait_ms").and_then(|v| v.as_u64()) {
            body["wait_ms"] = json!(wait_ms);
        }

        if let Some(max_length) = args.get("max_length").and_then(|v| v.as_u64()) {
            body["max_length"] = json!(max_length);
        }

        match call_orchestrator(&self.config, "browser_read", body).await {
            Ok(response) => {
                let title = response["title"].as_str().unwrap_or("(untitled)");
                let text = response["text_content"].as_str().unwrap_or("");
                let text_len = response["text_length"].as_u64().unwrap_or(0);
                let final_url = response["url"].as_str().unwrap_or(url);

                let mut output = format!(
                    "Page: {}\nURL: {}\nText length: {} chars\n\n{}",
                    title, final_url, text_len, text
                );

                // Append links if present
                if let Some(links) = response["links"].as_array() {
                    if !links.is_empty() {
                        output.push_str("\n\n--- Links ---\n");
                        for link in links.iter().take(20) {
                            let href = link["href"].as_str().unwrap_or("");
                            let link_text = link["text"].as_str().unwrap_or("");
                            if !href.is_empty() {
                                output.push_str(&format!("- [{}]({})\n", link_text, href));
                            }
                        }
                    }
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
