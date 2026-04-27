//! Single-shot and session-scoped llama.cpp completion using chat template (OpenAI-compat messages).

use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::pin::pin;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{
    AddBos, GrammarTrigger, GrammarTriggerType, LlamaChatTemplate, LlamaModel,
};
use llama_cpp_2::openai::OpenAIChatTemplateParams;
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;
use llama_cpp_2::{json_schema_to_grammar, send_logs_to_tracing, GrammarError, LogOptions};

use crate::schema::CommandProposal;
use crate::tty::{eprintln_labeled, Severity};

/// Default context size and thread count for local completions (shared by session and one-shots).
pub(crate) fn default_context_params() -> LlamaContextParams {
    let n_ctx = NonZeroU32::new(8192).unwrap();
    let threads = std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1) as i32)
        .unwrap_or(4);
    LlamaContextParams::default()
        .with_n_ctx(Some(n_ctx))
        .with_n_threads(threads)
        .with_n_threads_batch(threads)
}

fn grammar_sampler_for_result(
    model: &LlamaModel,
    grammar_text: &str,
    grammar_lazy: bool,
    triggers: &[GrammarTrigger],
) -> Result<LlamaSampler, String> {
    if grammar_lazy {
        let mut words: Vec<Vec<u8>> = Vec::new();
        let mut tokens: Vec<LlamaToken> = Vec::new();
        let mut patterns: Vec<String> = Vec::new();
        for t in triggers {
            match t.trigger_type {
                GrammarTriggerType::Word => words.push(t.value.clone().into_bytes()),
                GrammarTriggerType::Token => {
                    if let Some(tok) = t.token {
                        tokens.push(tok);
                    }
                }
                GrammarTriggerType::Pattern | GrammarTriggerType::PatternFull => {
                    patterns.push(t.value.clone());
                }
            }
        }
        let lazy = if !patterns.is_empty() {
            LlamaSampler::grammar_lazy_patterns(model, grammar_text, "root", &patterns, &tokens)
                .map_err(|e: GrammarError| format!("grammar_lazy_patterns: {e:?}"))?
        } else if !words.is_empty() || !tokens.is_empty() {
            LlamaSampler::grammar_lazy(
                model,
                grammar_text,
                "root",
                words.iter().map(Vec::as_slice),
                &tokens,
            )
            .map_err(|e: GrammarError| format!("grammar_lazy: {e:?}"))?
        } else {
            LlamaSampler::grammar(model, grammar_text, "root")
                .map_err(|e: GrammarError| format!("grammar (lazy fallback): {e:?}"))?
        };
        Ok(LlamaSampler::chain_simple([lazy, LlamaSampler::greedy()]))
    } else {
        let g = LlamaSampler::grammar(model, grammar_text, "root")
            .map_err(|e: GrammarError| format!("grammar: {e:?}"))?;
        Ok(LlamaSampler::chain_simple([g, LlamaSampler::greedy()]))
    }
}

#[allow(clippy::too_many_arguments)]
fn complete_with_loaded_model(
    model: &LlamaModel,
    backend: &LlamaBackend,
    system: &str,
    user: &str,
    max_new_tokens: i32,
    context_params: &LlamaContextParams,
    verbose: bool,
    mut on_token: impl FnMut(&str),
) -> Result<String, String> {
    let tmpl = model
        .chat_template(None)
        .or_else(|_| LlamaChatTemplate::new("chatml"))
        .map_err(|e| format!("chat template: {:?}", e))?;

    let schema_str = CommandProposal::schema_json();
    let messages = serde_json::json!([
        {"role": "system", "content": system},
        {"role": "user", "content": user}
    ])
    .to_string();

    let enable_thinking = std::env::var("CLAI_ENABLE_THINKING")
        .ok()
        .is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes"));
    let reasoning_format = std::env::var("CLAI_REASONING_FORMAT").ok();

    let params = OpenAIChatTemplateParams {
        messages_json: &messages,
        tools_json: None,
        tool_choice: None,
        json_schema: Some(schema_str),
        grammar: None,
        reasoning_format: reasoning_format.as_deref(),
        chat_template_kwargs: None,
        add_generation_prompt: true,
        use_jinja: true,
        parallel_tool_calls: false,
        enable_thinking,
        add_bos: false,
        add_eos: false,
        parse_tool_calls: false,
    };

    let rendered = model
        .apply_chat_template_oaicompat(&tmpl, &params)
        .map_err(|e| format!("template: {:?}", e))?;

    let use_schema_grammar = std::env::var("CLAI_JSON_SCHEMA_GRAMMAR")
        .ok()
        .is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes"));

    let grammar_text: Option<String> = if use_schema_grammar {
        json_schema_to_grammar(schema_str)
            .ok()
            .or_else(|| rendered.grammar.clone())
    } else {
        None
    };

    let lazy_ok = std::env::var("CLAI_GRAMMAR_LAZY")
        .ok()
        .is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes"))
        && rendered.grammar_lazy;

    let mut sampler = if let Some(ref gtext) = grammar_text {
        grammar_sampler_for_result(model, gtext, lazy_ok, &rendered.grammar_triggers)?
    } else {
        LlamaSampler::chain_simple([LlamaSampler::greedy()])
    };

    if verbose {
        eprintln_labeled(
            "clai",
            "local inference: initializing context (llama context + sampler)",
            Severity::Info,
        );
    }

    let mut ctx = model
        .new_context(backend, context_params.clone())
        .map_err(|e| format!("context: {:?}", e))?;

    let tokens = model
        .str_to_token(&rendered.prompt, AddBos::Never)
        .map_err(|e| format!("tokenize: {:?}", e))?;

    let n_ctx_i = ctx.n_ctx() as i32;
    if tokens.len() as i32 >= n_ctx_i - max_new_tokens {
        return Err("prompt too long for context".into());
    }

    let mut batch = LlamaBatch::new(512, 1);
    let last_index: i32 = (tokens.len() - 1) as i32;
    for (i, token) in (0_i32..).zip(tokens) {
        let is_last = i == last_index;
        batch
            .add(token, i, &[0], is_last)
            .map_err(|e| format!("batch: {:?}", e))?;
    }

    ctx.decode(&mut batch)
        .map_err(|e| format!("decode prompt: {:?}", e))?;

    if verbose {
        eprintln_labeled("clai", "local inference: generating tokens", Severity::Info);
    }

    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut out = String::new();
    let mut n_cur = batch.n_tokens();
    let target = n_cur + max_new_tokens;

    while n_cur < target {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);
        if model.is_eog_token(token) {
            break;
        }
        let piece = model
            .token_to_piece(token, &mut decoder, true, None)
            .map_err(|e| format!("piece: {:?}", e))?;
        out.push_str(&piece);
        on_token(&piece);

        batch.clear();
        batch
            .add(token, n_cur, &[0], true)
            .map_err(|e| format!("batch add: {:?}", e))?;
        n_cur += 1;
        ctx.decode(&mut batch)
            .map_err(|e| format!("decode: {:?}", e))?;
    }

    Ok(out)
}

/// One-shot completion: initializes backend, loads GGUF, runs one request, drops (same cost model as
/// historical `complete_local`).
pub fn complete_local(
    model_path: &Path,
    system: &str,
    user: &str,
    max_new_tokens: i32,
) -> Result<String, String> {
    complete_local_with(model_path, system, user, max_new_tokens, false, |_| {})
}

/// Like [`complete_local`], with a callback for each decoded piece (e.g. TTY stream).
pub fn complete_local_with<F: FnMut(&str)>(
    model_path: &Path,
    system: &str,
    user: &str,
    max_new_tokens: i32,
    verbose: bool,
    on_token: F,
) -> Result<String, String> {
    send_logs_to_tracing(LogOptions::default().with_logs_enabled(false));
    let mut s = LocalLlamaSession::open(model_path, verbose)?;
    s.complete(system, user, max_new_tokens, verbose, on_token)
}

/// Session-scoped local inference: **one** backend + model load; subsequent [`complete`](Self::complete)
/// calls reuse the loaded weights (NFR-1). Context parameters are built once and reused for each
/// completion to avoid redundant per-turn thread sizing.
pub struct LocalLlamaSession {
    backend: LlamaBackend,
    model: LlamaModel,
    model_path: PathBuf,
    context_params: LlamaContextParams,
}

impl LocalLlamaSession {
    /// Load GGUF from disk (full cold start). With `verbose`, logs a distinct **loading weights**
    /// phase (no API keys; suitable for support triage).
    pub fn open(model_path: &Path, verbose: bool) -> Result<Self, String> {
        send_logs_to_tracing(LogOptions::default().with_logs_enabled(false));
        if verbose {
            eprintln_labeled(
                "clai",
                "local inference: loading model weights (GGUF from disk)",
                Severity::Info,
            );
        }
        let backend = LlamaBackend::init().map_err(|e| format!("backend: {:?}", e))?;

        let n_gpu = std::env::var("CLAI_N_GPU_LAYERS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let model_params = pin!(LlamaModelParams::default().with_n_gpu_layers(n_gpu));
        let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
            .map_err(|e| format!("load model: {:?}", e))?;

        Ok(Self {
            backend,
            model,
            model_path: model_path.to_path_buf(),
            context_params: default_context_params(),
        })
    }

    /// Reload GGUF from the same path (in-session `reload` / model file swap).
    pub fn reload(&mut self) -> Result<(), String> {
        send_logs_to_tracing(LogOptions::default().with_logs_enabled(false));
        let n_gpu = std::env::var("CLAI_N_GPU_LAYERS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let model_params = pin!(LlamaModelParams::default().with_n_gpu_layers(n_gpu));
        self.model = LlamaModel::load_from_file(&self.backend, &self.model_path, &model_params)
            .map_err(|e| format!("reload model: {:?}", e))?;
        self.context_params = default_context_params();
        Ok(())
    }

    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    pub fn complete<F: FnMut(&str)>(
        &mut self,
        system: &str,
        user: &str,
        max_new_tokens: i32,
        verbose: bool,
        on_token: F,
    ) -> Result<String, String> {
        let ctxp = &self.context_params;
        complete_with_loaded_model(
            &self.model,
            &self.backend,
            system,
            user,
            max_new_tokens,
            ctxp,
            verbose,
            on_token,
        )
    }
}

#[cfg(all(test, feature = "llama-embed"))]
mod tests {
    use std::num::NonZeroU32;

    use super::default_context_params;

    #[test]
    fn default_context_params_uses_expected_n_ctx() {
        let p = default_context_params();
        assert_eq!(p.n_ctx(), NonZeroU32::new(8192));
    }
}
