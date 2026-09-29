//! Tests for the Claude agent backend: argv construction, stream-frame parsing, session
//! mapping and one gated real-CLI turn.

#[path = "agent_claude/argv.rs"]
mod argv;

#[path = "agent_claude/auth.rs"]
mod auth;

#[path = "agent_claude/frames.rs"]
mod frames;

#[path = "agent_claude/routing.rs"]
mod routing;

#[path = "agent_claude/sessions.rs"]
mod sessions;

#[path = "agent_claude/turn.rs"]
mod turn;

#[path = "agent_claude/real_turn.rs"]
mod real_turn;

#[path = "agent_claude/real_command_turn.rs"]
mod real_command_turn;
