//! Pre-run human presentation for command proposals (FR-4, FR-6, US-2, SC-5).

use crate::policy::{PolicyDecision, RiskTier};
use crate::schema::CommandProposal;

/// Display-only quoting for a single argv token (matches `main` preview behavior).
pub fn shell_escape_for_display(t: &str) -> String {
    if t.is_empty() {
        return "''".to_string();
    }
    if t.chars()
        .any(|c| c.is_whitespace() || matches!(c, '\\' | '\'' | '"'))
    {
        format!("'{}'", t.replace('\'', "'\"'\"'"))
    } else {
        t.to_string()
    }
}

pub fn command_line_for_display(p: &CommandProposal) -> String {
    let mut s = shell_escape_for_display(&p.program);
    for a in &p.args {
        s.push(' ');
        s.push_str(&shell_escape_for_display(a));
    }
    s
}

/// Rich pre-run block: argv/shell, intent, rationale, policy hints. No executor calls.
pub fn format_pre_run_presentation(
    proposal: &CommandProposal,
    decision: &PolicyDecision,
) -> String {
    let mut lines: Vec<String> = Vec::new();

    if decision.blocked {
        lines.push("── Proposal (blocked) ──".to_string());
        lines.push(format!(
            "Command line: {}",
            command_line_for_display(proposal)
        ));
        if proposal.needs_shell {
            lines.push(
                "Shell: this proposal requests shell execution (`needs_shell: true`).".to_string(),
            );
        }
        let why = decision
            .reason
            .as_deref()
            .unwrap_or("blocked by policy (no reason given).");
        lines.push(format!("Blocked: {why}"));
        lines.push("This command will not be run.".to_string());
        return lines.join("\n");
    }

    lines.push("── Proposal ──".to_string());
    if proposal.needs_shell {
        lines.push(format!(
            "Shell line (needs_shell): {}",
            command_line_for_display(proposal)
        ));
    } else {
        lines.push(format!(
            "Executable + args: {}",
            command_line_for_display(proposal)
        ));
    }

    if let Some(c) = &proposal.cwd {
        lines.push(format!("Working directory: {c}"));
    }

    let intent = proposal
        .reason
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|r| format!("What / intent: {r}"))
        .unwrap_or_else(|| "What / intent: (no rationale provided in model output)".to_string());
    lines.push(intent);

    if let Some(conf) = proposal
        .confidence
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        lines.push(format!("Confidence: {conf}"));
    }

    match decision.tier {
        RiskTier::ReadOnly | RiskTier::Standard => {}
        RiskTier::Sensitive | RiskTier::Destructive => {
            if decision.requires_confirmation {
                lines.push(
                    "Policy: this command requires extra confirmation before it may run."
                        .to_string(),
                );
            }
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::PolicyDecision;

    #[test]
    fn empty_reason_shows_explicit_copy() {
        let p = CommandProposal {
            program: "true".into(),
            args: vec![],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
        };
        let d = PolicyDecision {
            tier: RiskTier::Standard,
            requires_confirmation: false,
            blocked: false,
            reason: None,
        };
        let s = format_pre_run_presentation(&p, &d);
        assert!(s.contains("no rationale provided"));
    }

    #[test]
    fn blocked_branch_includes_explanation() {
        let p = CommandProposal {
            program: "rm".into(),
            args: vec!["-rf".into(), "/".into()],
            cwd: None,
            reason: Some("bad".into()),
            needs_shell: false,
            confidence: None,
        };
        let d = PolicyDecision {
            tier: RiskTier::Destructive,
            requires_confirmation: true,
            blocked: true,
            reason: Some("matches high-risk blocklist".into()),
        };
        let s = format_pre_run_presentation(&p, &d);
        assert!(s.contains("Blocked:"));
        assert!(s.contains("will not be run"));
    }
}
