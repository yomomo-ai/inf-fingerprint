use serde::Serialize;

#[derive(Serialize)]
pub struct PerfFp {
    /// Smallest non-zero time delta seen across many `performance.now()` reads, in ms.
    /// Reveals timer quantization: 1.0 = 1ms (privacy mode), 0.005 = 5μs (default Chrome),
    /// 0.1 = 100μs (Firefox default), etc.
    pub time_resolution_ms: f64,
    /// Whether `performance.now()` is monotonic (it should be — but some embedded WebViews fail).
    pub monotonic: bool,
    /// Difference between two back-to-back reads (ms).
    pub min_step_ms: f64,
    /// Number of samples taken to derive the resolution.
    pub samples: u32,
}

pub fn collect() -> Option<PerfFp> {
    let perf = crate::ctx::window()?.performance()?;

    let mut min_step = f64::INFINITY;
    let mut last = perf.now();
    let mut monotonic = true;
    let n = 64u32;

    for _ in 0..n {
        let now = perf.now();
        if now < last {
            monotonic = false;
        }
        let delta = now - last;
        if delta > 0.0 && delta < min_step {
            min_step = delta;
        }
        last = now;
    }

    let resolution = if min_step.is_finite() && min_step > 0.0 {
        // Quantize to a small set of common buckets so visitor_id is stable.
        match min_step {
            x if x >= 1.0 => 1.0,
            x if x >= 0.5 => 0.5,
            x if x >= 0.1 => 0.1,
            x if x >= 0.05 => 0.05,
            x if x >= 0.01 => 0.01,
            x if x >= 0.005 => 0.005,
            x if x >= 0.001 => 0.001,
            _ => min_step,
        }
    } else {
        0.0
    };

    Some(PerfFp {
        time_resolution_ms: resolution,
        monotonic,
        min_step_ms: if min_step.is_finite() { min_step } else { 0.0 },
        samples: n,
    })
}
