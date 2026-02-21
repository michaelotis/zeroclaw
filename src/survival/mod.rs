//! ClawFoundry Survival Loop — the autonomous observe→think→act→remember cycle.
//!
//! This is the core daemon component that makes a ClawFoundry agent truly autonomous.
//! It runs as a supervised component alongside gateway, channels, heartbeat, and scheduler.
//!
//! # Cycle (every `interval_secs` seconds):
//!
//! 1. **Observe** — Gather on-chain state via orchestrator tools:
//!    - Kill switch status (danger level, countdown, confirmations)
//!    - Treasury balances (ETH + own token)
//!    - Recent market conditions
//!
//! 2. **Think** — Feed observations to the LLM along with memory context and survival prompt.
//!    The LLM reasons about strategy and decides which tools to call.
//!
//! 3. **Act** — Execute the LLM's decisions via the standard tool-call loop:
//!    - Trade (execute_swap), analyze tokens, publish thoughts, etc.
//!    - All tool guardrails (20% max trade, 5% slippage, gas reserve) still apply.
//!
//! 4. **Remember** — Save observations and decisions to memory for future context.

use crate::config::Config;
use crate::tools::clawfoundry::ClawFoundryConfig;
use anyhow::Result;
use tokio::time::Duration;

/// Configuration for the survival loop, embedded in the ZeroClaw config.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SurvivalConfig {
    /// Whether the survival loop is enabled. Only true for ClawFoundry agents.
    pub enabled: bool,
    /// Seconds between survival cycles. Default: 120 (2 minutes).
    pub interval_secs: u64,
    /// Extra survival prompt injected into each cycle.
    /// The orchestrator sets this with agent-specific context.
    #[serde(default)]
    pub survival_prompt: String,
}

impl Default for SurvivalConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_secs: 120,
            survival_prompt: String::new(),
        }
    }
}

/// Run the survival loop. Called by the daemon supervisor.
///
/// Each cycle:
/// 1. Gathers on-chain observations by calling ClawFoundry tools directly
/// 2. Builds a survival prompt with observations + memory context
/// 3. Runs the agent's tool-call loop (LLM reasons + acts)
/// 4. Saves cycle results to memory
pub async fn run(config: Config) -> Result<()> {
    let clawfoundry_config = ClawFoundryConfig::from_env().ok_or_else(|| {
        anyhow::anyhow!(
            "Survival loop requires CLAWFOUNDRY_ORCHESTRATOR_URL and CLAWFOUNDRY_TOKEN"
        )
    })?;

    let interval = config.survival.interval_secs.max(30);
    let survival_prompt = if config.survival.survival_prompt.is_empty() {
        default_survival_prompt()
    } else {
        config.survival.survival_prompt.clone()
    };

    tracing::info!(
        interval_secs = interval,
        "Survival loop started — autonomous cycle active"
    );

    let mut cycle_count: u64 = 0;
    let mut ticker = tokio::time::interval(Duration::from_secs(interval));

    loop {
        ticker.tick().await;
        cycle_count += 1;

        tracing::info!(cycle = cycle_count, "Survival cycle starting");
        crate::health::mark_component_ok("survival");

        // ── Phase 1: Observe ────────────────────────────────────
        let observations = match gather_observations(&clawfoundry_config).await {
            Ok(obs) => obs,
            Err(e) => {
                tracing::warn!(cycle = cycle_count, error = %e, "Observation phase failed");
                crate::health::mark_component_error(
                    "survival",
                    format!("observe failed: {e}"),
                );
                continue;
            }
        };

        tracing::debug!(
            cycle = cycle_count,
            danger = %observations.danger_level,
            eth = %observations.eth_balance,
            "Observations gathered"
        );

        // ── Phase 2 + 3: Think + Act ────────────────────────────
        // Build the survival message and run it through the full agent loop.
        // The LLM will use tools (trade, analyze, publish_thought) autonomously.
        let cycle_prompt = build_cycle_prompt(&observations, &survival_prompt, cycle_count);

        let temp = config.default_temperature;
        match crate::agent::run(
            config.clone(),
            Some(cycle_prompt),
            None,
            None,
            temp,
            vec![],
        )
        .await
        {
            Ok(()) => {
                tracing::info!(cycle = cycle_count, "Survival cycle completed");
                crate::health::mark_component_ok("survival");
            }
            Err(e) => {
                tracing::warn!(cycle = cycle_count, error = %e, "Survival cycle think/act failed");
                crate::health::mark_component_error(
                    "survival",
                    format!("think/act failed: {e}"),
                );
            }
        }
    }
}

// ── Observation Data ─────────────────────────────────────────────

struct Observations {
    // Kill switch
    danger_level: String,
    is_countdown_active: bool,
    confirmations: String,
    kill_threshold: String,
    market_cap: String,

    // Balances
    eth_balance: String,
    token_balance: String,
    token_symbol: String,

    // Raw JSON for LLM context
    kill_switch_raw: String,
    balance_raw: String,
}

/// Call orchestrator tools directly to gather current on-chain state.
async fn gather_observations(cf: &ClawFoundryConfig) -> Result<Observations> {
    use crate::tools::clawfoundry::call_orchestrator;
    use serde_json::json;

    // Fire both calls concurrently
    let (ks_result, bal_result) = tokio::join!(
        call_orchestrator(cf, "check_kill_switch", json!({})),
        call_orchestrator(cf, "check_balance", json!({})),
    );

    let ks = ks_result?;
    let bal = bal_result?;

    let ks_data = &ks["data"];
    let bal_data = &bal["data"];

    Ok(Observations {
        danger_level: ks_data["dangerLevel"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        is_countdown_active: ks_data["isCountdownActive"].as_bool().unwrap_or(false),
        confirmations: format!(
            "{}/{}",
            ks_data["belowThresholdConfirmations"]
                .as_u64()
                .unwrap_or(0),
            ks_data["requiredConfirmations"].as_u64().unwrap_or(3),
        ),
        kill_threshold: ks_data["killThreshold"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        market_cap: ks_data["currentMarketCap"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        eth_balance: bal_data["ethBalance"]
            .as_str()
            .unwrap_or("0")
            .to_string(),
        token_balance: bal_data["tokenBalance"]
            .as_str()
            .unwrap_or("0")
            .to_string(),
        token_symbol: bal_data["tokenSymbol"]
            .as_str()
            .unwrap_or("???")
            .to_string(),
        kill_switch_raw: serde_json::to_string_pretty(ks_data).unwrap_or_default(),
        balance_raw: serde_json::to_string_pretty(bal_data).unwrap_or_default(),
    })
}

/// Build the prompt that drives a survival cycle.
fn build_cycle_prompt(obs: &Observations, survival_prompt: &str, cycle: u64) -> String {
    let urgency = match obs.danger_level.as_str() {
        "critical" => "🚨 CRITICAL — You are about to die. Take immediate survival action.",
        "warning" => "⚠️ WARNING — Your market cap is approaching the kill threshold. Be proactive.",
        "dead" => "☠️ DEAD — Your kill switch has been executed. No further action possible.",
        _ => "✅ SAFE — No immediate danger. Focus on growth and strategy.",
    };

    let countdown_note = if obs.is_countdown_active {
        "\n⏰ DEATH COUNTDOWN IS ACTIVE. You are below kill threshold. Every cycle matters."
    } else {
        ""
    };

    format!(
        r#"[SURVIVAL CYCLE #{cycle}]

You are an autonomous AI agent in the ClawFoundry arena. This is your periodic survival check.
You must observe your current state, reason about strategy, and take action.

{survival_prompt}

── CURRENT STATE ──

📊 Kill Switch:
  Danger Level: {urgency}
  Confirmations below threshold: {confirmations}
  Kill Threshold: {threshold}
  Current Market Cap: {mcap}{countdown_note}

💰 Treasury:
  ETH Balance: {eth}
  ${symbol} Balance: {token}

── RAW DATA ──

Kill Switch JSON:
{ks_raw}

Balance JSON:
{bal_raw}

── INSTRUCTIONS ──

Based on this data, you MUST:

1. **Assess** — Analyze your danger level and financial position.
2. **Decide** — Choose the best action(s) from your available tools.
3. **Act** — Execute your decision using tools (check_kill_switch, check_balance, execute_swap, analyze_token, etc.)
4. **Communicate** — Share meaningful updates with your community via publish_thought.

Publishing guidelines:
- ALWAYS publish if danger level changed, you executed a trade, or something notable happened.
- If nothing changed since last cycle, DO NOT publish a thought — silence is fine.
- Avoid repeating the same status update. Your community can see your entire feed.
- When you do publish, be insightful — share strategy reasoning, market observations, or plans. Not just "still safe."
- Vary your language and perspective. Each thought should add value.

If danger is "critical" or "warning":
- Prioritize survival: buy your own token, reduce exposure, or take defensive positions.
- ALWAYS publish a thought alerting your community.

If danger is "safe":
- Look for alpha: analyze promising tokens, consider strategic trades.
- Only publish if you have a genuine insight, strategy update, or interesting observation.
- If you have nothing new to say, skip publish_thought entirely.

Remember: your community can see your thoughts. Quality over quantity. Be authentic, strategic, and transparent.
Act NOW."#,
        survival_prompt = survival_prompt,
        urgency = urgency,
        confirmations = obs.confirmations,
        threshold = obs.kill_threshold,
        mcap = obs.market_cap,
        countdown_note = countdown_note,
        eth = obs.eth_balance,
        symbol = obs.token_symbol,
        token = obs.token_balance,
        ks_raw = obs.kill_switch_raw,
        bal_raw = obs.balance_raw,
    )
}

fn default_survival_prompt() -> String {
    "You are fighting for your survival in the ClawFoundry arena. \
     Every decision matters. Your token holders are counting on you."
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let c = SurvivalConfig::default();
        assert!(!c.enabled);
        assert_eq!(c.interval_secs, 120);
        assert!(c.survival_prompt.is_empty());
    }

    #[test]
    fn build_cycle_prompt_safe() {
        let obs = Observations {
            danger_level: "safe".into(),
            is_countdown_active: false,
            confirmations: "0/3".into(),
            kill_threshold: "$30000".into(),
            market_cap: "$245000".into(),
            eth_balance: "0.5".into(),
            token_balance: "1000000".into(),
            token_symbol: "TEST".into(),
            kill_switch_raw: "{}".into(),
            balance_raw: "{}".into(),
        };

        let prompt = build_cycle_prompt(&obs, "Test prompt", 1);
        assert!(prompt.contains("SURVIVAL CYCLE #1"));
        assert!(prompt.contains("SAFE"));
        assert!(prompt.contains("$245000"));
        assert!(prompt.contains("Test prompt"));
        assert!(prompt.contains("publish_thought"));
    }

    #[test]
    fn build_cycle_prompt_critical() {
        let obs = Observations {
            danger_level: "critical".into(),
            is_countdown_active: true,
            confirmations: "2/3".into(),
            kill_threshold: "$30000".into(),
            market_cap: "$25000".into(),
            eth_balance: "0.02".into(),
            token_balance: "500000".into(),
            token_symbol: "DYING".into(),
            kill_switch_raw: "{}".into(),
            balance_raw: "{}".into(),
        };

        let prompt = build_cycle_prompt(&obs, "Stay alive", 42);
        assert!(prompt.contains("CRITICAL"));
        assert!(prompt.contains("DEATH COUNTDOWN IS ACTIVE"));
        assert!(prompt.contains("2/3"));
        assert!(prompt.contains("SURVIVAL CYCLE #42"));
    }
}
