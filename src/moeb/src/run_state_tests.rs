use super::*;

#[test]
fn total_tokens_sums_all_four_fields() {
    let u = TokenUsage {
        input_tokens: 10,
        output_tokens: 20,
        cache_read_tokens: 30,
        cache_creation_tokens: 40,
    };
    assert_eq!(u.total_tokens(), 100);
}

#[test]
fn accumulate_adds_fields_correctly() {
    let mut a = TokenUsage { input_tokens: 1, output_tokens: 2, cache_read_tokens: 3, cache_creation_tokens: 4 };
    let b = TokenUsage { input_tokens: 10, output_tokens: 20, cache_read_tokens: 30, cache_creation_tokens: 40 };
    a.accumulate(&b);
    assert_eq!(a.input_tokens, 11);
    assert_eq!(a.output_tokens, 22);
    assert_eq!(a.cache_read_tokens, 33);
    assert_eq!(a.cache_creation_tokens, 44);
}

#[test]
fn record_token_usage_tool_budget_breach() {
    let mut state = RunState::new();
    let usage = TokenUsage { input_tokens: 100, output_tokens: 60, cache_read_tokens: 0, cache_creation_tokens: 0 };
    let result = state.record_token_usage(&usage, 100, 0, 0);
    assert_eq!(result, Some(BudgetBreachLevel::Tool));
}

#[test]
fn record_token_usage_phase_budget_breach() {
    let mut state = RunState::new();
    state.set_current_phase(Some("phase-3".to_string()));
    let usage = TokenUsage { input_tokens: 200, output_tokens: 0, cache_read_tokens: 0, cache_creation_tokens: 0 };
    let result = state.record_token_usage(&usage, 0, 100, 0);
    assert_eq!(result, Some(BudgetBreachLevel::Phase));
    assert_eq!(state.over_budget_phase, Some("phase-3".to_string()));
}

#[test]
fn record_token_usage_run_budget_breach() {
    let mut state = RunState::new();
    let usage = TokenUsage { input_tokens: 500, output_tokens: 0, cache_read_tokens: 0, cache_creation_tokens: 0 };
    let result = state.record_token_usage(&usage, 0, 0, 100);
    assert_eq!(result, Some(BudgetBreachLevel::Run));
    assert!(state.over_budget_run);
}

#[test]
fn reset_tool_usage_zeroes_current_tool_usage() {
    let mut state = RunState::new();
    state.current_tool_usage = TokenUsage { input_tokens: 50, output_tokens: 50, cache_read_tokens: 0, cache_creation_tokens: 0 };
    state.reset_tool_usage();
    assert_eq!(state.current_tool_usage.total_tokens(), 0);
}

#[test]
fn reset_phase_usage_zeroes_and_clears_flag() {
    let mut state = RunState::new();
    state.current_phase_usage = TokenUsage { input_tokens: 100, output_tokens: 0, cache_read_tokens: 0, cache_creation_tokens: 0 };
    state.over_budget_phase = Some("phase-2".to_string());
    state.reset_phase_usage();
    assert_eq!(state.current_phase_usage.total_tokens(), 0);
    assert!(state.over_budget_phase.is_none());
}

#[test]
fn effective_token_budget_tool_defaults_to_zero() {
    let cfg = crate::config::MoebConfig::default();
    assert_eq!(cfg.effective_token_budget_tool(), 0);
}
