use serde::{Deserialize, Serialize};

/// Persisted result of one clip loudness analysis. The measured values make the
/// operation reproducible; playback/export consume only `gain_db` and never
/// need to re-read the source.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoudnessNormalization {
    pub target_lufs: f64,
    pub true_peak_ceiling_dbtp: f64,
    pub input_integrated_lufs: f64,
    pub input_true_peak_dbtp: f64,
    pub gain_db: f64,
    pub output_integrated_lufs: f64,
    pub output_true_peak_dbtp: f64,
}

impl LoudnessNormalization {
    pub fn validate(&self) -> Result<(), &'static str> {
        let values = [
            self.target_lufs,
            self.true_peak_ceiling_dbtp,
            self.input_integrated_lufs,
            self.input_true_peak_dbtp,
            self.gain_db,
            self.output_integrated_lufs,
            self.output_true_peak_dbtp,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return Err("loudness values must be finite");
        }
        if self.true_peak_ceiling_dbtp > 0.0 {
            return Err("true-peak ceiling must be at most 0 dBTP");
        }
        if !(-70.0..=0.0).contains(&self.target_lufs)
            || !(-20.0..=0.0).contains(&self.true_peak_ceiling_dbtp)
            || !(-120.0..=60.0).contains(&self.gain_db)
        {
            return Err("loudness target, ceiling, or gain is outside the supported range");
        }
        if (self.output_integrated_lufs - self.target_lufs).abs() > 1.0 {
            return Err("normalized loudness does not reach its target");
        }
        if self.output_true_peak_dbtp > self.true_peak_ceiling_dbtp + 0.05 {
            return Err("normalized true peak exceeds its ceiling");
        }
        Ok(())
    }

    pub fn linear_gain(self) -> f64 {
        if !self.gain_db.is_finite() {
            return 1.0;
        }
        10.0_f64.powf(self.gain_db.clamp(-120.0, 60.0) / 20.0)
    }
}
