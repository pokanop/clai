//! Structured command proposal from the model (argv-first).

use serde::{Deserialize, Serialize};

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
    "confidence": { "type": "string" }
  }
}"##
    }

    /// Parse model output as JSON (grammar-constrained output is usually a single object).
    ///
    /// Tries a full-string parse first, then falls back to the first `{...}` slice.
    pub fn parse_from_model_text(text: &str) -> crate::error::Result<Self> {
        let t = text.trim();
        if let Ok(p) = serde_json::from_str::<Self>(t) {
            return Ok(p);
        }
        let start = t
            .find('{')
            .ok_or_else(|| crate::error::AppError::Msg("no JSON object in model output".into()))?;
        let rest = &t[start..];
        let end = rest
            .rfind('}')
            .ok_or_else(|| crate::error::AppError::Msg("unclosed JSON in model output".into()))?;
        let slice = &rest[..=end];
        serde_json::from_str(slice).map_err(Into::into)
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
}
