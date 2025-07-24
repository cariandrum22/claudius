use std::time::{Duration, Instant};
use tracing::{debug, info};

/// A simple timer for measuring execution time of code blocks
#[derive(Debug)]
pub struct Timer {
    label: String,
    start: Instant,
}

impl Timer {
    /// Create a new timer with a label
    pub fn new(label: &str) -> Self {
        debug!("Starting timer: {}", label);
        Self { label: label.to_string(), start: Instant::now() }
    }

    /// Stop the timer and log the elapsed time
    pub fn stop(self) -> Duration {
        let elapsed = self.start.elapsed();
        debug!("Timer '{}' completed in {:?}", self.label, elapsed);
        elapsed
    }
}

/// Macro for timing a block of code
#[macro_export]
macro_rules! time_block {
    ($label:expr, $block:block) => {{
        let _timer = $crate::profiling::Timer::new($label);
        let result = $block;
        result
    }};
}

/// Profile a function with flamegraph support
///
/// # Errors
///
/// Returns an error if:
/// - Failed to build the profiler
/// - Failed to create the output file
/// - Failed to generate the flamegraph report
#[cfg(feature = "profiling")]
pub fn profile_flamegraph<F, R>(label: &str, f: F) -> Result<R, anyhow::Error>
where
    F: FnOnce() -> R,
{
    use pprof::ProfilerGuardBuilder;
    use std::fs::File;

    info!("Starting flamegraph profiling for: {}", label);

    let guard = ProfilerGuardBuilder::default()
        .frequency(1000)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create profiler: {}", e))?;

    let result = f();

    if let Ok(report) = guard.report().build() {
        let file_name = format!("flamegraph-{}-{}.svg", label, chrono::Utc::now().timestamp());
        let file = File::create(&file_name)?;
        report.flamegraph(file)?;
        info!("Flamegraph saved to: {}", file_name);
    }

    Ok(result)
}

/// Profile a function without flamegraph (no-op when profiling feature is disabled)
#[cfg(not(feature = "profiling"))]
pub fn profile_flamegraph<F, R>(_label: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    f()
}

/// Detailed timing information for secret resolution
#[derive(Debug, Clone, Default)]
pub struct SecretResolutionMetrics {
    pub total_secrets: usize,
    pub successful_resolutions: usize,
    pub failed_resolutions: usize,
    pub op_calls: Vec<OpCallMetric>,
    pub total_duration: Duration,
}

/// Metrics for individual 1Password CLI calls
#[derive(Debug, Clone)]
pub struct OpCallMetric {
    pub secret_ref: String,
    pub duration: Duration,
    pub success: bool,
}

impl SecretResolutionMetrics {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_op_call(&mut self, secret_ref: String, duration: Duration, success: bool) {
        self.op_calls.push(OpCallMetric { secret_ref, duration, success });
        if success {
            self.successful_resolutions = self.successful_resolutions.saturating_add(1);
        } else {
            self.failed_resolutions = self.failed_resolutions.saturating_add(1);
        }
    }

    pub fn log_summary(&self) {
        info!("=== Secret Resolution Performance Summary ===");
        info!("Total secrets processed: {}", self.total_secrets);
        info!("Successful resolutions: {}", self.successful_resolutions);
        info!("Failed resolutions: {}", self.failed_resolutions);
        info!("Total time: {:?}", self.total_duration);

        if !self.op_calls.is_empty() {
            let op_count = u32::try_from(self.op_calls.len()).unwrap_or(1);
            let avg_duration = self.total_duration.checked_div(op_count).unwrap_or_default();
            info!("Average time per op call: {:?}", avg_duration);

            // Find slowest calls
            let mut sorted_calls = self.op_calls.iter().collect::<Vec<_>>();
            sorted_calls.sort_by_key(|c| std::cmp::Reverse(c.duration));

            info!("Slowest op calls:");
            for (i, call) in sorted_calls.iter().take(5).enumerate() {
                info!(
                    "  {}. {} - {:?} ({})",
                    i.saturating_add(1),
                    call.secret_ref,
                    call.duration,
                    if call.success { "success" } else { "failed" }
                );
            }
        }
    }
}
