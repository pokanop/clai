#![allow(clippy::result_large_err)]

pub mod app_update;
pub mod ask_exit;
pub mod cli_output;
pub mod cloud;
pub mod config;
pub mod engine;
pub mod ephemeral_script;
pub mod error;
pub mod executor;
pub mod host_context;
pub mod interactive_mode;
pub mod migrate;
pub mod ollama;
pub mod policy;
pub mod presentation;
pub mod registry;
pub mod runtime_tooling;
pub mod schema;
pub mod session;
pub mod stream_strategy;
pub mod tty;

pub use error::{AppError, Result};
