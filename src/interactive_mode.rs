//! Interactive session execution mode (dry-run / confirm / auto) and resolution (FR-13–FR-15).

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// How the default interactive session handles execution after policy allows a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[clap(rename_all = "kebab-case")]
pub enum InteractiveExecutionMode {
    /// Show the proposal, then prompt whether to execute (default **no**); policy may still apply before run.
    DryRun,
    /// Show the proposal, then prompt before run (unless blocked or dry-run).
    #[default]
    Confirm,
    /// Run without the interactive “run it?” step; still honors FR-16 policy confirmation.
    Auto,
}

impl InteractiveExecutionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "dry-run",
            Self::Confirm => "confirm",
            Self::Auto => "auto",
        }
    }

    /// Parse kebab-case values used in TOML and env (`dry-run`, `confirm`, `auto`).
    pub fn parse_loose(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "dry-run" | "dry_run" | "dryrun" => Some(Self::DryRun),
            "confirm" => Some(Self::Confirm),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// FR-16 helper: interactive “run it?” step applies only in **confirm** when the user did not pass `--yes`.
#[must_use]
pub fn needs_interactive_run_prompt(mode: InteractiveExecutionMode, policy_auto_yes: bool) -> bool {
    mode == InteractiveExecutionMode::Confirm && !policy_auto_yes
}

/// True when **dry-run** should ask whether to execute (before policy and the confirm-mode run prompt).
#[must_use]
pub fn needs_dry_run_execute_prompt(mode: InteractiveExecutionMode, policy_auto_yes: bool) -> bool {
    mode == InteractiveExecutionMode::DryRun && !policy_auto_yes
}

/// Resolve effective mode: **CLI `--yes` > CLI `--interactive-mode` > config (file+env via figment) > legacy `dry_run_default`**.
#[must_use]
pub fn resolve_effective_interactive_execution_mode(
    config_field: Option<InteractiveExecutionMode>,
    cli_flag: Option<InteractiveExecutionMode>,
    cli_yes: bool,
    dry_run_default: bool,
) -> InteractiveExecutionMode {
    if cli_yes {
        return InteractiveExecutionMode::Auto;
    }
    if let Some(m) = cli_flag {
        return m;
    }
    if let Some(m) = config_field {
        return m;
    }
    if dry_run_default {
        InteractiveExecutionMode::DryRun
    } else {
        InteractiveExecutionMode::Confirm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_cli_yes_wins() {
        assert_eq!(
            resolve_effective_interactive_execution_mode(
                Some(InteractiveExecutionMode::DryRun),
                Some(InteractiveExecutionMode::DryRun),
                true,
                true,
            ),
            InteractiveExecutionMode::Auto
        );
    }

    #[test]
    fn precedence_cli_flag_over_config() {
        assert_eq!(
            resolve_effective_interactive_execution_mode(
                Some(InteractiveExecutionMode::DryRun),
                Some(InteractiveExecutionMode::Confirm),
                false,
                true,
            ),
            InteractiveExecutionMode::Confirm
        );
    }

    #[test]
    fn config_field_over_legacy() {
        assert_eq!(
            resolve_effective_interactive_execution_mode(
                Some(InteractiveExecutionMode::Auto),
                None,
                false,
                true,
            ),
            InteractiveExecutionMode::Auto
        );
    }

    #[test]
    fn legacy_dry_run_default_true_maps_dry_run() {
        assert_eq!(
            resolve_effective_interactive_execution_mode(None, None, false, true),
            InteractiveExecutionMode::DryRun
        );
    }

    #[test]
    fn legacy_dry_run_default_false_maps_confirm_not_auto() {
        assert_eq!(
            resolve_effective_interactive_execution_mode(None, None, false, false),
            InteractiveExecutionMode::Confirm
        );
    }

    #[test]
    fn parse_loose_accepts_aliases() {
        assert_eq!(
            InteractiveExecutionMode::parse_loose("DRY-RUN"),
            Some(InteractiveExecutionMode::DryRun)
        );
    }

    #[test]
    fn fr16_run_prompt_only_in_confirm_without_yes() {
        assert!(needs_interactive_run_prompt(
            InteractiveExecutionMode::Confirm,
            false
        ));
        assert!(!needs_interactive_run_prompt(
            InteractiveExecutionMode::Confirm,
            true
        ));
        assert!(!needs_interactive_run_prompt(
            InteractiveExecutionMode::Auto,
            false
        ));
        assert!(!needs_interactive_run_prompt(
            InteractiveExecutionMode::DryRun,
            false
        ));
    }

    #[test]
    fn dry_run_asks_before_execute_unless_yes() {
        assert!(needs_dry_run_execute_prompt(
            InteractiveExecutionMode::DryRun,
            false
        ));
        assert!(!needs_dry_run_execute_prompt(
            InteractiveExecutionMode::DryRun,
            true
        ));
        assert!(!needs_dry_run_execute_prompt(
            InteractiveExecutionMode::Confirm,
            false
        ));
    }
}
