use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::schema::CommandProposal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskTier {
    ReadOnly,
    Standard,
    Sensitive,
    Destructive,
}

#[derive(Debug, Clone, Serialize)]
pub struct PolicyDecision {
    pub tier: RiskTier,
    pub requires_confirmation: bool,
    pub blocked: bool,
    pub reason: Option<String>,
}

pub struct PolicyEngine {
    pub jail_root: PathBuf,
    pub strict_allowlist: bool,
    pub allowlist_bins: Vec<String>,
}

impl PolicyEngine {
    pub fn new(jail_root: PathBuf, strict_allowlist: bool, allowlist_bins: Vec<String>) -> Self {
        Self {
            jail_root,
            strict_allowlist,
            allowlist_bins,
        }
    }

    pub fn evaluate(&self, proposal: &CommandProposal) -> PolicyDecision {
        let program_lower = proposal.program.to_lowercase();
        let joined = format!("{} {}", proposal.program, proposal.args.join(" ")).to_lowercase();

        if self.strict_allowlist && !self.allowlist_bins.is_empty() {
            let ok = self
                .allowlist_bins
                .iter()
                .any(|b| b.eq_ignore_ascii_case(&proposal.program));
            if !ok {
                return PolicyDecision {
                    tier: RiskTier::Destructive,
                    requires_confirmation: true,
                    blocked: true,
                    reason: Some("program not in allowlist".into()),
                };
            }
        }

        if is_blocked_pattern(&joined) {
            return PolicyDecision {
                tier: RiskTier::Destructive,
                requires_confirmation: true,
                blocked: true,
                reason: Some("matches high-risk blocklist".into()),
            };
        }

        if let Err(reason) = self.check_cwd_jail(proposal) {
            return PolicyDecision {
                tier: RiskTier::Sensitive,
                requires_confirmation: true,
                blocked: true,
                reason: Some(reason),
            };
        }

        if let Some(tier) = sensitive_tier(&program_lower, &joined) {
            let destructive = tier == RiskTier::Destructive;
            return PolicyDecision {
                tier,
                requires_confirmation: true,
                blocked: destructive,
                reason: None,
            };
        }

        if proposal.needs_shell {
            return PolicyDecision {
                tier: RiskTier::Sensitive,
                requires_confirmation: true,
                blocked: false,
                reason: Some("shell execution requested".into()),
            };
        }

        PolicyDecision {
            tier: RiskTier::Standard,
            requires_confirmation: false,
            blocked: false,
            reason: None,
        }
    }

    fn check_cwd_jail(&self, proposal: &CommandProposal) -> Result<(), String> {
        let Some(cwd) = &proposal.cwd else {
            return Ok(());
        };
        let path = Path::new(cwd);
        let normalized = normalize_path(path);
        if !normalized.starts_with(&self.jail_root) {
            return Err(format!(
                "cwd {} escapes jail root {}",
                cwd,
                self.jail_root.display()
            ));
        }
        Ok(())
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Prefix(p) => out.push(p.as_os_str()),
            Component::RootDir => out.push(std::path::MAIN_SEPARATOR_STR),
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = out.pop();
            }
            Component::Normal(p) => out.push(p),
        }
    }
    out
}

fn is_blocked_pattern(s: &str) -> bool {
    let patterns = [
        "rm -rf /",
        "rm -rf /*",
        "mkfs",
        "dd if=",
        ":(){",
        "chmod -r 777",
        "chmod 777 -r",
        "> /dev/sd",
        "format ",
        "diskpart",
    ];
    patterns.iter().any(|p| s.contains(p))
}

fn sensitive_tier(program: &str, full: &str) -> Option<RiskTier> {
    let destructive_progs = ["rm", "shred", "mkfs", "dd"];
    if destructive_progs.contains(&program) {
        return Some(RiskTier::Destructive);
    }
    if full.contains("rm -rf") || full.contains("curl") && full.contains("|") {
        return Some(RiskTier::Destructive);
    }
    let sensitive = ["sudo", "su", "chmod", "chown", "diskutil", "fdisk", "mount"];
    if sensitive.contains(&program) {
        return Some(RiskTier::Sensitive);
    }
    let readonly = [
        "ls", "cat", "head", "tail", "less", "find", "grep", "which", "pwd",
    ];
    if readonly.contains(&program) {
        return Some(RiskTier::ReadOnly);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_rm_rf_root() {
        let eng = PolicyEngine::new(PathBuf::from("/tmp"), false, vec![]);
        let p = CommandProposal {
            program: "rm".into(),
            args: vec!["-rf".into(), "/".into()],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
            script_body: None,
            script_extension: None,
        };
        let d = eng.evaluate(&p);
        assert!(d.blocked);
    }

    #[test]
    fn cwd_jail() {
        let jail = std::env::temp_dir().join("clai_policy_jail");
        let eng = PolicyEngine::new(jail.clone(), false, vec![]);
        let outside = std::env::temp_dir().join("clai_policy_outside");
        let p = CommandProposal {
            program: "ls".into(),
            args: vec![],
            cwd: Some(outside.display().to_string()),
            reason: None,
            needs_shell: false,
            confidence: None,
            script_body: None,
            script_extension: None,
        };
        let d = eng.evaluate(&p);
        assert!(d.blocked);
    }

    #[test]
    fn strict_allowlist_sees_final_interpreter_with_temp_path() {
        let eng = PolicyEngine::new(PathBuf::from("/tmp"), true, vec!["python3".into()]);
        let p = CommandProposal {
            program: "python3".into(),
            args: vec!["/var/tmp/clai-script-abc/script.py".into()],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
            script_body: None,
            script_extension: None,
        };
        let d = eng.evaluate(&p);
        assert!(!d.blocked);
    }
}
