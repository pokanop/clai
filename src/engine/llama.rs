//! Single-shot llama.cpp completion using chat template (OpenAI-compat messages).

use std::num::NonZeroU32;
use std::path::Path;
use std::pin::pin;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatTemplate, LlamaModel};
use llama_cpp_2::openai::OpenAIChatTemplateParams;
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::{send_logs_to_tracing, LogOptions};

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

    let messages = serde_json::json!([
        {"role": "system", "content": system},
        {"role": "user", "content": user}
    ])
    .to_string();

    let params = OpenAIChatTemplateParams {
        messages_json: &messages,
        tools_json: None,
        tool_choice: None,
        json_schema: None,
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

    let mut sampler = LlamaSampler::chain_simple([LlamaSampler::greedy()]);
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
