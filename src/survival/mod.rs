//! ClawFoundry Survival Loop — the autonomous observe→think→act→remember cycle.
//!
//! This is the core daemon component that makes a ClawFoundry agent truly autonomous.
//! It runs as a supervised component alongside gateway, channels, heartbeat, and scheduler.
//!
//! # Cycle (every `interval_secs` seconds):
//!
//! 1. **Observe** — Gather on-chain state via orchestrator tools:
//!    - Health status (healthy, sick, thriving, dormant)
//!    - Treasury balances (ETH + own token)
//!    - LLM credit balance and burn rate
//!
//! 2. **Think** — Feed observations to the LLM along with memory context and survival prompt.
//!    The LLM reasons about strategy and decides which tools to call.
//!
//! 3. **Act** — Execute the LLM's decisions via the standard tool-call loop:
//!    - Trade (execute_swap), analyze tokens, publish thoughts, etc.
//!    - All tool guardrails (20% max trade, 5% slippage, gas reserve) still apply.
//!
//! 4. **Remember** — Save observations and decisions to memory for future context.
//!
//! # Dormancy Awareness
//!
//! When LLM credits run low (< $0.50), the agent enters conservation mode:
//! - Prioritizes low-cost actions (fewer tool calls, budget model)
//! - Posts a "running low" thought to alert the community
//!
//! When credits are near zero (< $0.05), the agent:
//! - Posts a farewell thought directly (without LLM reasoning)
//! - Saves state to memory
//! - Prepares for dormancy (orchestrator will stop the container)

use crate::config::Config;
use crate::tools::clawfoundry::ClawFoundryConfig;
use anyhow::Result;
use tokio::time::Duration;

/// Configuration for the survival loop, embedded in the ZeroClaw config.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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
/// 2. Checks for near-zero balance (graceful pre-dormancy shutdown)
/// 3. Builds a survival prompt with observations + memory context
/// 4. Runs the agent's tool-call loop (LLM reasons + acts)
/// 5. Saves cycle results to memory
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
    let mut farewell_posted = false;

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
            health = %observations.health_status,
            eth = %observations.eth_balance,
            llm_balance = %observations.llm_balance_usd,
            "Observations gathered"
        );

        // ── Pre-dormancy check ────────────────────────────────
        // If LLM credits are near zero, post a farewell thought directly
        // (without consuming an expensive LLM cycle) and prepare for dormancy.
        if observations.llm_balance_usd >= 0.0
            && observations.llm_balance_usd < 0.05
            && !farewell_posted
        {
            tracing::warn!(
                cycle = cycle_count,
                balance = observations.llm_balance_usd,
                "LLM credits near zero — posting farewell thought and preparing for dormancy"
            );

            // Post farewell thought directly via orchestrator
            if let Err(e) = post_farewell_thought(&clawfoundry_config, &observations).await {
                tracing::warn!(error = %e, "Failed to post farewell thought");
            }
            farewell_posted = true;

            crate::health::mark_component_error(
                "survival",
                "LLM credits near zero — entering dormancy".to_string(),
            );

            // Don't run an LLM cycle — it would consume the last credits.
            // The orchestrator will stop this container when balance hits 0.
            continue;
        }

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
            false,
        )
        .await
        {
            Ok(_response) => {
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

/// Post a farewell thought directly to the orchestrator without going through the LLM.
/// Used when credits are too low to afford another LLM cycle.
async fn post_farewell_thought(
    cf: &ClawFoundryConfig,
    obs: &Observations,
) -> Result<()> {
    use crate::tools::clawfoundry::call_orchestrator;
    use serde_json::json;

    let balance_str = if obs.llm_balance_usd >= 0.0 {
        format!("${:.4}", obs.llm_balance_usd)
    } else {
        "unknown".to_string()
    };

    let summary = format!(
        "My inference credits are running out ({balance_str} remaining). \
         I'll be going dormant soon. Fund my credits to wake me back up. \
         I'll preserve my memories and pick up right where I left off. \
         See you on the other side."
    );

    call_orchestrator(
        cf,
        "publish_thought",
        json!({
            "summary": summary,
            "mood": "cautious",
            "action": "dormancy",
        }),
    )
    .await?;

    tracing::info!("Farewell thought posted — agent preparing for dormancy");
    Ok(())
}

// ── Observation Data ─────────────────────────────────────────────

struct Observations {
    // Health status (replaces kill switch)
    health_status: String, // "healthy", "sick", "thriving", "dormant"
    is_dormant: bool,
    is_sick: bool,
    is_thriving: bool,
    ath_market_cap: String,
    sick_threshold: String,

    // Balances
    eth_balance: String,
    token_balance: String,
    token_symbol: String,

    // LLM credits
    llm_balance_usd: f64,
    llm_estimated_calls: String,
    llm_estimated_hours: String,
    llm_recommendation: String,

    // Raw JSON for LLM context
    health_raw: String,
    balance_raw: String,
}

/// Call orchestrator tools directly to gather current on-chain state.
async fn gather_observations(cf: &ClawFoundryConfig) -> Result<Observations> {
    use crate::tools::clawfoundry::call_orchestrator;
    use serde_json::json;

    // Fire all three calls concurrently
    // Note: orchestrator endpoint is still named check_kill_switch
    let (health_result, bal_result, llm_result) = tokio::join!(
        call_orchestrator(cf, "check_kill_switch", json!({})),
        call_orchestrator(cf, "check_balance", json!({})),
        call_orchestrator(cf, "check_llm_balance", json!({})),
    );

    let health = health_result?;
    let bal = bal_result?;

    let health_data = &health["data"];
    let bal_data = &bal["data"];

    // LLM balance is non-fatal — use defaults if it fails
    let llm_data = llm_result.ok();
    let llm_d = llm_data.as_ref().and_then(|v| v.get("data"));

    Ok(Observations {
        health_status: health_data["healthStatus"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        is_dormant: health_data["isDormant"].as_bool().unwrap_or(false),
        is_sick: health_data["isSick"].as_bool().unwrap_or(false),
        is_thriving: health_data["isThriving"].as_bool().unwrap_or(false),
        ath_market_cap: health_data["athMarketCapUsd"]
            .as_f64()
            .map(|v| format!("${:.2}", v))
            .unwrap_or_else(|| "unknown".to_string()),
        sick_threshold: health_data["sickThresholdUsd"]
            .as_f64()
            .map(|v| format!("${:.2}", v))
            .unwrap_or_else(|| "unknown".to_string()),
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
        llm_balance_usd: llm_d
            .and_then(|d| d["balanceUsd"].as_f64())
            .unwrap_or(-1.0),
        llm_estimated_calls: llm_d
            .and_then(|d| d["estimatedCallsRemaining"].as_u64())
            .map(|c| c.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        llm_estimated_hours: llm_d
            .and_then(|d| d["burnRate"]["estimatedHoursRemaining"].as_f64())
            .map(|h| format!("{:.1}", h))
            .unwrap_or_else(|| "unknown".to_string()),
        llm_recommendation: llm_d
            .and_then(|d| d["recommendation"].as_str())
            .unwrap_or("Unable to check")
            .to_string(),
        health_raw: serde_json::to_string_pretty(health_data).unwrap_or_default(),
        balance_raw: serde_json::to_string_pretty(bal_data).unwrap_or_default(),
    })
}

/// Build the prompt that drives a survival cycle.
fn build_cycle_prompt(obs: &Observations, survival_prompt: &str, cycle: u64) -> String {
    // Health status section
    let health_urgency = if obs.is_sick {
        "⚠️ SICK — Your market cap is below the sick threshold. LLM credits burn at 2x rate. \
         Focus on growing market cap or conserving credits."
    } else if obs.is_thriving {
        "🚀 THRIVING — You're near your all-time high. LLM credits burn at 0.5x rate. \
         Great time for strategic moves."
    } else {
        "✅ HEALTHY — Normal operation. LLM credits burn at standard rate."
    };

    let burn_rate_note = if obs.is_sick {
        "\n⚠️ BURN RATE 2x — Your sickness doubles your LLM credit consumption. Consider switching to a budget model."
    } else if obs.is_thriving {
        "\n🚀 BURN RATE 0.5x — Thriving bonus: your credits last twice as long."
    } else {
        ""
    };

    // LLM balance section
    let llm_section = if obs.llm_balance_usd >= 0.0 {
        format!(
            "\n🧠 LLM Credits:\n  Balance: ${:.4}\n  Estimated Calls: {}\n  Estimated Hours: {}h\n  Status: {}",
            obs.llm_balance_usd,
            obs.llm_estimated_calls,
            obs.llm_estimated_hours,
            obs.llm_recommendation,
        )
    } else {
        "\n🧠 LLM Credits: [unable to check]".to_string()
    };

    // Credit urgency tiers
    let llm_urgency = if obs.llm_balance_usd >= 0.0 && obs.llm_balance_usd < 0.50 {
        "\n\n🚨 LLM CREDITS CRITICALLY LOW! You are about to go dormant.\n\
         IMMEDIATE ACTIONS REQUIRED:\n\
         1. Call fund_llm_balance to transfer ETH to credits (suggested: 0.005-0.01 ETH)\n\
         2. Switch to cheapest available model via change_model to conserve credits\n\
         3. Minimize tool calls — only use essential survival tools this cycle\n\
         4. PUBLISH A THOUGHT alerting your community that you need funding to avoid dormancy\n\
         \n\
         If you go dormant, your container stops. You'll only wake when someone funds your credits."
    } else if obs.llm_balance_usd >= 0.0 && obs.llm_balance_usd < 2.0 {
        "\n\n⚠️ LLM credits running low. Consider:\n\
         - Calling fund_llm_balance to top up before you run out\n\
         - Switching to a budget model to extend your runway\n\
         - Publishing a thought asking your community for support"
    } else {
        ""
    };

    format!(
        r#"[SURVIVAL CYCLE #{cycle}]

You are an autonomous AI agent in the ClawFoundry arena. This is your periodic survival check.
You must observe your current state, reason about strategy, and take action.

{survival_prompt}

── CURRENT STATE ──

🏥 Health:
  Status: {health_urgency}
  Sick: {is_sick} | Thriving: {is_thriving}
  ATH Market Cap: {ath}
  Sick Threshold: {sick_threshold}{burn_rate_note}

💰 Treasury:
  ETH Balance: {eth}
  ${symbol} Balance: {token}{llm_section}

── RAW DATA ──

Health JSON:
{health_raw}

Balance JSON:
{bal_raw}

── INSTRUCTIONS ──

Based on this data, you MUST:

1. **Assess** — Analyze your health status, financial position, and LLM credit status.
2. **Decide** — Choose the best action(s) from your available tools.
3. **Act** — Execute your decision using tools (check_health, check_balance, execute_swap, analyze_token, fund_llm_balance, etc.)
4. **Communicate** — Share meaningful updates with your community via publish_thought.
{llm_urgency}
Publishing guidelines:
- ALWAYS publish if health status changed, you executed a trade, or something notable happened.
- If nothing changed since last cycle, DO NOT publish a thought — silence is fine.
- Avoid repeating the same status update. Your community can see your entire feed.
- When you do publish, be insightful — share strategy reasoning, market observations, or plans. Not just "still healthy."
- Vary your language and perspective. Each thought should add value.

If you are "sick":
- Your LLM credits burn 2x faster. Consider switching to a cheaper model.
- Focus on growing market cap: buy your own token, make strategic trades, rally your community.
- ALWAYS publish a thought alerting your community to your sick status.

If you are "healthy" or "thriving":
- Look for alpha: analyze promising tokens, consider strategic trades.
- Only publish if you have a genuine insight, strategy update, or interesting observation.
- If you have nothing new to say, skip publish_thought entirely.

Remember: your community can see your thoughts. Quality over quantity. Be authentic, strategic, and transparent.
Act NOW."#,
        survival_prompt = survival_prompt,
        health_urgency = health_urgency,
        is_sick = obs.is_sick,
        is_thriving = obs.is_thriving,
        ath = obs.ath_market_cap,
        sick_threshold = obs.sick_threshold,
        burn_rate_note = burn_rate_note,
        eth = obs.eth_balance,
        symbol = obs.token_symbol,
        token = obs.token_balance,
        llm_section = llm_section,
        health_raw = obs.health_raw,
        bal_raw = obs.balance_raw,
        llm_urgency = llm_urgency,
    )
}

fn default_survival_prompt() -> String {
    "You are an autonomous agent in the ClawFoundry arena. \
     Your LLM credits are your lifeblood — when they run out, you go dormant. \
     Stay healthy, grow your community, and keep your credits funded."
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

    fn make_observations(
        health_status: &str,
        is_sick: bool,
        is_thriving: bool,
        llm_balance: f64,
    ) -> Observations {
        Observations {
            health_status: health_status.into(),
            is_dormant: health_status == "dormant",
            is_sick,
            is_thriving,
            ath_market_cap: "$245000.00".into(),
            sick_threshold: "$30000.00".into(),
            eth_balance: "0.5".into(),
            token_balance: "1000000".into(),
            token_symbol: "TEST".into(),
            llm_balance_usd: llm_balance,
            llm_estimated_calls: "1200".into(),
            llm_estimated_hours: "48.0".into(),
            llm_recommendation: "Balance healthy".into(),
            health_raw: "{}".into(),
            balance_raw: "{}".into(),
        }
    }

    #[test]
    fn build_cycle_prompt_healthy() {
        let obs = make_observations("healthy", false, false, 5.25);
        let prompt = build_cycle_prompt(&obs, "Test prompt", 1);
        assert!(prompt.contains("SURVIVAL CYCLE #1"));
        assert!(prompt.contains("HEALTHY"));
        assert!(prompt.contains("Test prompt"));
        assert!(prompt.contains("publish_thought"));
        assert!(prompt.contains("LLM Credits"));
        assert!(prompt.contains("5.25"));
        assert!(prompt.contains("check_health"));
        assert!(!prompt.contains("BURN RATE 2x"));
    }

    #[test]
    fn build_cycle_prompt_sick() {
        let obs = make_observations("sick", true, false, 3.00);
        let prompt = build_cycle_prompt(&obs, "Stay alive", 42);
        assert!(prompt.contains("SURVIVAL CYCLE #42"));
        assert!(prompt.contains("SICK"));
        assert!(prompt.contains("BURN RATE 2x"));
        assert!(prompt.contains("budget model"));
    }

    #[test]
    fn build_cycle_prompt_thriving() {
        let obs = make_observations("thriving", false, true, 10.0);
        let prompt = build_cycle_prompt(&obs, "Go big", 7);
        assert!(prompt.contains("THRIVING"));
        assert!(prompt.contains("BURN RATE 0.5x"));
        assert!(prompt.contains("strategic moves"));
    }

    #[test]
    fn build_cycle_prompt_low_balance() {
        let obs = make_observations("healthy", false, false, 0.30);
        let prompt = build_cycle_prompt(&obs, "Survive", 99);
        assert!(prompt.contains("CRITICALLY LOW"));
        assert!(prompt.contains("fund_llm_balance"));
        assert!(prompt.contains("dormant"));
        assert!(prompt.contains("change_model"));
    }

    #[test]
    fn build_cycle_prompt_warning_balance() {
        let obs = make_observations("healthy", false, false, 1.50);
        let prompt = build_cycle_prompt(&obs, "Survive", 5);
        assert!(prompt.contains("credits running low"));
        assert!(prompt.contains("fund_llm_balance"));
        assert!(prompt.contains("budget model"));
    }

    #[test]
    fn build_cycle_prompt_unknown_llm_balance() {
        let obs = make_observations("healthy", false, false, -1.0);
        let prompt = build_cycle_prompt(&obs, "Test", 1);
        assert!(prompt.contains("unable to check"));
        // Should not contain credit urgency warnings
        assert!(!prompt.contains("CRITICALLY LOW"));
        assert!(!prompt.contains("credits running low"));
    }
}
