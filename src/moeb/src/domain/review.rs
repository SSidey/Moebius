use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ReviewCategory {
    Error,
    SkillImprovement,
    ToolImprovement,
    NewCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ReviewSeverity {
    Critical,
    Major,
    Minor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum GatingCondition {
    NoCandidateBranch,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSignal {
    pub signal_id: String,
    pub run_id: String,
    pub timestamp: String,
    pub category: ReviewCategory,
    pub severity: ReviewSeverity,
    pub title: String,
    pub description: String,
    pub proposed_resolution: Option<String>,
    pub auto_spec_path: Option<String>,
    pub gating_condition: Option<GatingCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepMetric {
    pub step_id: String,
    pub iteration_count: u8,
    pub acceptance_rate: f32,
    pub delta_scores: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetrics {
    pub run_id: String,
    pub timestamp: String,
    pub rubric_score: f32,
    pub step_metrics: Vec<StepMetric>,
    pub end_review_error_count: u32,
    pub wall_time_ms: u64,
}
