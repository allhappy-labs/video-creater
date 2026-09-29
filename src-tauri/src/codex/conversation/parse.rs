//! Extracts a conversation proposal from app-server turn output.

use super::CodexConversationEditProposal;
use serde_json::Value;

pub fn conversation_proposal_from_value(value: &Value) -> Option<CodexConversationEditProposal> {
    serde_json::from_value::<CodexConversationEditProposal>(value.clone())
        .ok()
        .or_else(|| {
            value
                .get("proposal")
                .and_then(|proposal| serde_json::from_value(proposal.clone()).ok())
        })
        .or_else(|| {
            value
                .get("structuredOutput")
                .and_then(conversation_proposal_from_value)
        })
        .or_else(|| match value.get("output") {
            Some(Value::String(text)) => conversation_proposal_from_text(text),
            Some(output) => conversation_proposal_from_value(output),
            None => None,
        })
}

pub fn conversation_proposal_from_text(text: &str) -> Option<CodexConversationEditProposal> {
    let trimmed = text.trim();
    let trimmed = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|body| body.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(trimmed);
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str::<Value>(trimmed)
        .ok()
        .and_then(|value| conversation_proposal_from_value(&value))
}
