//! Pre-run human presentation for command proposals (FR-4, FR-6, US-2, SC-5).

use crate::policy::{PolicyDecision, RiskTier};
use crate::schema::CommandProposal;

/// One line of the pre-run proposal block (plain or styled).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreRunLine {
    /// Top banner for an allowed proposal.
    SectionProposal,
    /// Top banner when policy blocks.
    SectionBlocked,
    /// `blocked == true` → "Command line:"; else shell vs executable + args.
    CommandLine {
        needs_shell: bool,
        line: String,
        blocked: bool,
    },
    /// Extra `needs_shell` note (only when **blocked** and `needs_shell`).
    ShellRequestNote,
    /// `cwd: …`
    WorkingDir(String),
    /// User-visible intent (from `reason`).
    Intent(String),
    /// Confidence if present in model output.
    Confidence(String),
    /// Extra confirmation for sensitive / destructive.
    PolicyConfirm,
    /// Blocked reason from policy.
    Blocked { reason: String },
    /// Footer when blocked.
    WontRun,
    /// Ephemeral script materialized under the managed temp contract (US-5).
    ManagedTempScript(String),
}

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

/// Structured pre-run lines (for plain text or TTY styling). No executor calls.
pub fn pre_run_lines(
    proposal: &CommandProposal,
    decision: &PolicyDecision,
    managed_script_path: Option<&str>,
) -> Vec<PreRunLine> {
    let mut lines: Vec<PreRunLine> = Vec::new();

    if decision.blocked {
        lines.push(PreRunLine::SectionBlocked);
        lines.push(PreRunLine::CommandLine {
            needs_shell: proposal.needs_shell,
            line: command_line_for_display(proposal),
            blocked: true,
        });
        if let Some(p) = managed_script_path.filter(|s| !s.is_empty()) {
            lines.push(PreRunLine::ManagedTempScript(p.to_string()));
        }
        if proposal.needs_shell {
            lines.push(PreRunLine::ShellRequestNote);
        }
        let why = decision
            .reason
            .as_deref()
            .unwrap_or("blocked by policy (no reason given).")
            .to_string();
        lines.push(PreRunLine::Blocked { reason: why });
        lines.push(PreRunLine::WontRun);
        return lines;
    }

    lines.push(PreRunLine::SectionProposal);
    lines.push(PreRunLine::CommandLine {
        needs_shell: proposal.needs_shell,
        line: command_line_for_display(proposal),
        blocked: false,
    });
    if let Some(p) = managed_script_path.filter(|s| !s.is_empty()) {
        lines.push(PreRunLine::ManagedTempScript(p.to_string()));
    }
    if let Some(c) = &proposal.cwd {
        lines.push(PreRunLine::WorkingDir(c.clone()));
    }
    let intent = proposal
        .reason
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "(no rationale provided in model output)".to_string());
    lines.push(PreRunLine::Intent(intent));
    if let Some(conf) = proposal
        .confidence
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        lines.push(PreRunLine::Confidence(conf.to_string()));
    }
    match decision.tier {
        RiskTier::ReadOnly | RiskTier::Standard => {}
        RiskTier::Sensitive | RiskTier::Destructive => {
            if decision.requires_confirmation {
                lines.push(PreRunLine::PolicyConfirm);
            }
        }
    }
    lines
}

/// Rich pre-run block: argv/shell, intent, rationale, policy hints. No executor calls.
pub fn format_pre_run_presentation(
    proposal: &CommandProposal,
    decision: &PolicyDecision,
    managed_script_path: Option<&str>,
) -> String {
    use std::fmt::Write as _;

    let parts = pre_run_lines(proposal, decision, managed_script_path);
    let mut out = String::new();
    for (i, line) in parts.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        match line {
            PreRunLine::SectionProposal => {
                out.push_str("── Proposal ──");
            }
            PreRunLine::SectionBlocked => {
                out.push_str("── Proposal (blocked) ──");
            }
            PreRunLine::CommandLine {
                needs_shell,
                line,
                blocked,
            } => {
                if *blocked {
                    let _ = write!(out, "Command line: {line}");
                } else if *needs_shell {
                    let _ = write!(out, "Shell line (needs_shell): {line}");
                } else {
                    let _ = write!(out, "Executable + args: {line}");
                }
            }
            PreRunLine::ShellRequestNote => {
                out.push_str(
                    "Shell: this proposal requests shell execution (`needs_shell: true`).",
                );
            }
            PreRunLine::WorkingDir(c) => {
                let _ = write!(out, "Working directory: {c}");
            }
            PreRunLine::Intent(s) => {
                let _ = write!(out, "What / intent: {s}");
            }
            PreRunLine::Confidence(c) => {
                let _ = write!(out, "Confidence: {c}");
            }
            PreRunLine::PolicyConfirm => {
                out.push_str("Policy: this command requires extra confirmation before it may run.");
            }
            PreRunLine::Blocked { reason } => {
                let _ = write!(out, "Blocked: {reason}");
            }
            PreRunLine::WontRun => {
                out.push_str("This command will not be run.");
            }
            PreRunLine::ManagedTempScript(p) => {
                let _ = write!(
                    out,
                    "Managed temp script: interpreter runs this ephemeral file (removed after exit): {p}"
                );
            }
        }
    }
    out
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
            script_body: None,
            script_extension: None,
        };
        let d = PolicyDecision {
            tier: RiskTier::Standard,
            requires_confirmation: false,
            blocked: false,
            reason: None,
        };
        let s = format_pre_run_presentation(&p, &d, None);
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
            script_body: None,
            script_extension: None,
        };
        let d = PolicyDecision {
            tier: RiskTier::Destructive,
            requires_confirmation: true,
            blocked: true,
            reason: Some("matches high-risk blocklist".into()),
        };
        let s = format_pre_run_presentation(&p, &d, None);
        assert!(s.contains("Blocked:"));
        assert!(s.contains("will not be run"));
    }
}
