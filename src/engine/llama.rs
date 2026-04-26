//! Single-shot llama.cpp completion using chat template (OpenAI-compat messages).

use std::num::NonZeroU32;
use std::path::Path;
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

pub fn complete_local(
    model_path: &Path,
    system: &str,
    user: &str,
    max_new_tokens: i32,
) -> Result<String, String> {
    send_logs_to_tracing(LogOptions::default().with_logs_enabled(false));
    let backend = LlamaBackend::init().map_err(|e| format!("backend: {:?}", e))?;

    let n_gpu = std::env::var("CLAI_N_GPU_LAYERS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    let model_params = pin!(LlamaModelParams::default().with_n_gpu_layers(n_gpu));
    let model = LlamaModel::load_from_file(&backend, model_path, &model_params)
        .map_err(|e| format!("load model: {:?}", e))?;

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

    let params = OpenAIChatTemplateParams {
        messages_json: &messages,
        tools_json: None,
        tool_choice: None,
        json_schema: Some(schema_str),
        grammar: None,
        reasoning_format: None,
        chat_template_kwargs: None,
        add_generation_prompt: true,
        use_jinja: true,
        parallel_tool_calls: false,
        enable_thinking: false,
        add_bos: false,
        add_eos: false,
        parse_tool_calls: false,
    };

    let rendered = model
        .apply_chat_template_oaicompat(&tmpl, &params)
        .map_err(|e| format!("template: {:?}", e))?;

    // GBNF + llama.cpp's grammar sampler can abort (GGML_ASSERT in llama-grammar.cpp) on some
    // models/builds. Default is off; output is still steered by json_schema in the chat template
    // and validated after generation. Opt in: CLAI_JSON_SCHEMA_GRAMMAR=1
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

    // Lazy grammar + triggers (only if CLAI_JSON_SCHEMA_GRAMMAR=1): enable with CLAI_GRAMMAR_LAZY=1.
    let lazy_ok = std::env::var("CLAI_GRAMMAR_LAZY")
        .ok()
        .is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes"))
        && rendered.grammar_lazy;

    let mut sampler = if let Some(ref gtext) = grammar_text {
        grammar_sampler_for_result(&model, gtext, lazy_ok, &rendered.grammar_triggers)?
    } else {
        LlamaSampler::chain_simple([LlamaSampler::greedy()])
    };

    let n_ctx = NonZeroU32::new(8192).unwrap();
    let threads = std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1) as i32)
        .unwrap_or(4);

    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(n_ctx))
        .with_n_threads(threads)
        .with_n_threads_batch(threads);

    let mut ctx = model
        .new_context(&backend, ctx_params)
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
