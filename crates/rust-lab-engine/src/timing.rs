use std::time::Duration;

/// Wall-clock durations. Compilation includes Cargo, rustc and linking;
/// execution includes process startup, replay, user code, and output capture.
#[derive(Debug, Default, Clone, Copy)]
pub struct Timings {
    pub source_generation: Duration,
    pub prepare: Duration,
    pub compile_link: Duration,
    pub execute_process: Duration,
    pub total: Duration,
}

impl Timings {
    pub fn csv_header() -> &'static str {
        "sample,source_generation_ms,prepare_ms,compile_link_ms,execute_process_ms,total_ms"
    }

    pub fn csv_row(&self, sample: usize) -> String {
        format!(
            "{sample},{:.6},{:.6},{:.6},{:.6},{:.6}",
            ms(self.source_generation),
            ms(self.prepare),
            ms(self.compile_link),
            ms(self.execute_process),
            ms(self.total)
        )
    }
}

pub fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
