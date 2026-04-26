//! NFR-2 guardrail: wall-clock for **direct** `run_proposal` with **piped capture** (baseline) vs
//! **inherited** stdio for a no-op child (`true` / `cmd exit 0`); median overhead must stay within
//! the PRD’s ~500 ms order of magnitude (reference hardware; CI runners may be noisy).
//!
//! Run: `cargo test --no-default-features --locked --test trivial_child_overhead`

use std::time::{Duration, Instant};

use clai::config::{ExecutionConfig, ExecutionMode};
use clai::executor::run_proposal;
use clai::schema::CommandProposal;
use clai::stream_strategy::StreamStrategy;

const TIMEOUT: Duration = Duration::from_secs(15);
const CAP: usize = 32 * 1024;
const ITER: usize = 20;
const MAX_OVERHEAD: Duration = Duration::from_millis(500);

fn execution_direct() -> ExecutionConfig {
    ExecutionConfig {
        mode: ExecutionMode::Direct,
        ..Default::default()
    }
}

fn noop_proposal() -> CommandProposal {
    #[cfg(unix)]
    {
        CommandProposal {
            program: "true".to_string(),
            args: vec![],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
        }
    }
    #[cfg(windows)]
    {
        CommandProposal {
            program: "cmd".to_string(),
            args: vec!["/C".to_string(), "exit".to_string(), "0".to_string()],
            cwd: None,
            reason: None,
            needs_shell: false,
            confidence: None,
        }
    }
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    let mid = samples.len() / 2;
    if samples.is_empty() {
        return Duration::ZERO;
    }
    if samples.len().is_multiple_of(2) {
        (samples[mid - 1] + samples[mid]) / 2
    } else {
        samples[mid]
    }
}

fn time_run(strategy: StreamStrategy) -> Duration {
    let t0 = Instant::now();
    run_proposal(
        &noop_proposal(),
        TIMEOUT,
        CAP,
        &execution_direct(),
        strategy,
    )
    .expect("trivial no-op");
    t0.elapsed()
}

#[test]
fn inherit_overhead_versus_capture_within_nfr2_guardrail() {
    for _ in 0..2 {
        let _ = time_run(StreamStrategy::Capture);
        let _ = time_run(StreamStrategy::Inherit);
    }
    let mut cap = Vec::with_capacity(ITER);
    let mut inh = Vec::with_capacity(ITER);
    for _ in 0..ITER {
        cap.push(time_run(StreamStrategy::Capture));
        inh.push(time_run(StreamStrategy::Inherit));
    }
    let m_cap = median(cap);
    let m_inh = median(inh);
    let overhead = m_inh.saturating_sub(m_cap);
    assert!(
        overhead <= MAX_OVERHEAD,
        "NFR-2: median inherit ({m_inh:?}) vs capture ({m_cap:?}) overhead {overhead:?} exceeds {MAX_OVERHEAD:?} — investigate or document hardware noise"
    );
}
