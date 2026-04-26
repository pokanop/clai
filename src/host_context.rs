use std::env;
use std::io::IsTerminal;
use std::path::MAIN_SEPARATOR;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShellFamily {
    PosixSh,
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Pwsh,
    Cmd,
    Nu,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostContext {
    pub os: String,
    pub arch: String,
    pub os_description: String,
    pub shell_family: ShellFamily,
    pub shell_executable_hint: Option<String>,
    pub is_tty: bool,
    pub path_separator: char,
    pub cwd: String,
}

impl HostContext {
    pub fn gather(preferred_shell: Option<&str>, execution_profile: Option<&str>) -> Self {
        let os = env::consts::OS.to_string();
        let arch = env::consts::ARCH.to_string();
        let info = os_info::get();
        let os_description = format!("{} {}", info.os_type(), info.version());

        let shell_family = if let Some(p) = execution_profile {
            match p {
                "windows_powershell" => ShellFamily::PowerShell,
                "windows_cmd" => ShellFamily::Cmd,
                "posix" => ShellFamily::PosixSh,
                _ => detect_shell_family(preferred_shell),
            }
        } else {
            detect_shell_family(preferred_shell)
        };

        let shell_executable_hint = preferred_shell.map(|s| s.to_string()).or_else(|| {
            if cfg!(windows) {
                env::var("COMSPEC").ok()
            } else {
                env::var("SHELL").ok()
            }
        });

        let is_tty = std::io::stdout().is_terminal();

        HostContext {
            os,
            arch,
            os_description,
            shell_family,
            shell_executable_hint,
            is_tty,
            path_separator: MAIN_SEPARATOR,
            cwd: env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| ".".into()),
        }
    }

    pub fn to_prompt_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

fn detect_shell_family(preferred: Option<&str>) -> ShellFamily {
    if let Some(s) = preferred {
        return classify_shell_path(s);
    }
    if cfg!(windows) {
        if env::var("PSModulePath").is_ok() {
            return ShellFamily::PowerShell;
        }
        return ShellFamily::Cmd;
    }
    if let Ok(shell) = env::var("SHELL") {
        return classify_shell_path(&shell);
    }
    if let Some(sv) = which_shell::which_shell() {
        use which_shell::Shell as W;
        return match sv.shell {
            W::Bash => ShellFamily::Bash,
            W::Zsh => ShellFamily::Zsh,
            W::Fish => ShellFamily::Fish,
            W::Pwsh => ShellFamily::Pwsh,
            W::PowerShell => ShellFamily::PowerShell,
            W::Cmd => ShellFamily::Cmd,
            W::Nu => ShellFamily::Nu,
            W::Dash | W::Sh | W::Ksh | W::Tcsh | W::Csh => ShellFamily::PosixSh,
            W::Unknown => ShellFamily::Unknown,
        };
    }
    ShellFamily::Unknown
}

fn classify_shell_path(path: &str) -> ShellFamily {
    let lower = path.to_lowercase();
    if lower.contains("pwsh") {
        return ShellFamily::Pwsh;
    }
    if lower.contains("powershell") {
        return ShellFamily::PowerShell;
    }
    if lower.contains("cmd") {
        return ShellFamily::Cmd;
    }
    if lower.contains("fish") {
        return ShellFamily::Fish;
    }
    if lower.contains("zsh") {
        return ShellFamily::Zsh;
    }
    if lower.contains("bash") {
        return ShellFamily::Bash;
    }
    if lower.contains("nu") {
        return ShellFamily::Nu;
    }
    if lower.contains("sh") {
        return ShellFamily::PosixSh;
    }
    ShellFamily::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_bash_path() {
        assert_eq!(classify_shell_path("/bin/bash"), ShellFamily::Bash);
    }
}
