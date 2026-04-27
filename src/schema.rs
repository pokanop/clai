//! Structured command proposal from the model (argv-first).

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

/// Replace raw U+0000..=U+001F inside JSON string literals with `\\u00XX` escapes so `serde_json`
/// accepts model output that occasionally includes unescaped newlines, ESC, or other controls.
fn escape_unescaped_control_chars_in_json_strings(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 32);
    let mut in_string = false;
    let mut it = input.chars().peekable();
    while let Some(c) = it.next() {
        if !in_string {
            if c == '"' {
                in_string = true;
            }
            out.push(c);
            continue;
        }
        if c == '\\' {
            if it.peek() == Some(&'u') {
                out.push(c);
                out.push(it.next().unwrap());
                for _ in 0..4 {
                    if let Some(h) = it.next() {
                        out.push(h);
                    }
                }
                continue;
            }
            out.push(c);
            if let Some(n) = it.next() {
                out.push(n);
            }
            continue;
        }
        if c == '"' {
            in_string = false;
            out.push(c);
            continue;
        }
        if (c as u32) < 0x20 {
            let _ = write!(&mut out, "\\u{:04x}", c as u32);
        } else {
            out.push(c);
        }
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandProposal {
    /// Executable name or path (resolved later).
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    /// When true, executor may use a shell wrapper (policy-gated).
    #[serde(default)]
    pub needs_shell: bool,
    #[serde(default)]
    pub confidence: Option<String>,
    /// When `[tooling].ephemeral_scripts` is true, written to a private temp file and path appended to `args`.
    #[serde(default)]
    pub script_body: Option<String>,
    /// Optional suffix for the temp file (e.g. `py`, `js`); refined default is inferred from `program`.
    #[serde(default)]
    pub script_extension: Option<String>,
}

impl CommandProposal {
    pub fn schema_json() -> &'static str {
        r##"{
  "type": "object",
  "required": ["program"],
    "properties": {
    "program": { "type": "string" },
    "args": { "type": "array", "items": { "type": "string" } },
    "cwd": { "type": "string" },
    "reason": { "type": "string" },
    "needs_shell": { "type": "boolean" },
    "confidence": { "type": "string" },
    "script_body": { "type": "string" },
    "script_extension": { "type": "string" }
  }
}"##
    }

    /// Parse model output as JSON (grammar-constrained output is usually a single object).
    ///
    /// Tries a full-string parse first, then falls back to the first `{...}` slice.
    ///
    /// Unescaped ASCII control characters inside JSON string values are escaped (models sometimes
    /// emit raw newlines or terminal escape bytes in `reason` / `args`).
    pub fn parse_from_model_text(text: &str) -> crate::error::Result<Self> {
        let t = text.trim();
        if let Ok(p) = serde_json::from_str::<Self>(t) {
            return Ok(p);
        }
        let t_escaped = escape_unescaped_control_chars_in_json_strings(t);
        if t_escaped != t {
            if let Ok(p) = serde_json::from_str::<Self>(&t_escaped) {
                return Ok(p);
            }
        }
        let start = t
            .find('{')
            .ok_or_else(|| crate::error::AppError::Msg("no JSON object in model output".into()))?;
        let rest = &t[start..];
        let end = rest.rfind('}').ok_or_else(|| {
            crate::error::AppError::Msg(
                "unclosed JSON in model output (likely truncated; raise CLAI_MAX_NEW_TOKENS or shorten the command)"
                    .into(),
            )
        })?;
        let slice = &rest[..=end];
        let fixed = escape_unescaped_control_chars_in_json_strings(slice);
        serde_json::from_str(&fixed).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_embedded_json() {
        let t = r#"Here is the command:
{"program": "ls", "args": ["-la"], "reason": "list"}
tail"#;
        let p = CommandProposal::parse_from_model_text(t).unwrap();
        assert_eq!(p.program, "ls");
        assert_eq!(p.args, vec!["-la"]);
    }

    #[test]
    fn parses_raw_json_object() {
        let t = r#"  {"program": "echo", "args": ["hi"]}  "#;
        let p = CommandProposal::parse_from_model_text(t).unwrap();
        assert_eq!(p.program, "echo");
        assert_eq!(p.args, vec!["hi"]);
    }

    #[test]
    fn parses_optional_script_body_and_extension() {
        let t = r#"{"program":"python3","args":["-u"],"script_body":"print(42)","script_extension":"py","reason":"demo"}"#;
        let p = CommandProposal::parse_from_model_text(t).unwrap();
        assert_eq!(p.script_body.as_deref(), Some("print(42)"));
        assert_eq!(p.script_extension.as_deref(), Some("py"));
    }

    #[test]
    fn parses_json_with_unescaped_newline_in_string() {
        let t = concat!(
            r#"{"program": "echo", "reason": "line1"#,
            "\n",
            r#"line2"}"#,
        );
        let p = CommandProposal::parse_from_model_text(t).unwrap();
        assert_eq!(p.program, "echo");
        assert_eq!(p.reason.as_deref(), Some("line1\nline2"));
    }

    #[test]
    fn parses_json_with_raw_esc_in_reason() {
        // Raw U+001B in a value (invalid JSON) is what serde rejects; we escape it.
        let t = format!(r#"{{"program": "echo", "reason": "a{}b"}}"#, '\u{001b}');
        let p = CommandProposal::parse_from_model_text(&t).unwrap();
        assert_eq!(p.reason.as_deref(), Some("a\u{001b}b"));
    }
}
