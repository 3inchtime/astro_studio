pub mod runner;
pub mod skills;
pub mod tools;
pub mod types;

pub const PROMPT_AGENT_MAX_TURNS: usize = 6;
pub const PROMPT_AGENT_STATUS_ACTIVE: &str = "active";
pub const PROMPT_AGENT_STATUS_ACCEPTED: &str = "accepted";
pub const PROMPT_AGENT_STATUS_CANCELLED: &str = "cancelled";
