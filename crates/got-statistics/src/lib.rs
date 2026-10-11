use got_core::ExperimentConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectAnalysis {
    pub cause_name: String,
    pub mean_low: f64,
    pub mean_high: f64,
    pub effect_size: f64,
    pub std_dev: Option<f64>,
    pub confidence_interval: Option<(f64, f64)>,
    pub p_value: Option<f64>,
    pub is_significant: Option<bool>,
    /// Paired experimental units, not intermediate cycles.
    pub n_pairs: usize,
    /// Statistical classification; declaring a source does not authenticate it.
    pub inference_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnovaResult {
    pub f_statistic: Option<f64>,
    pub p_value: Option<f64>,
    pub eta_squared: Option<f64>,
    pub is_significant: Option<bool>,
    pub inference_available: bool,
    pub inference_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedObservation {
    /// A distinct, independent experiment execution (not a cycle).
    pub run_id: String,
    pub low: f64,
    pub high: f64,
}

/// Explicitly declared empirical provenance and raw paired observations.
/// Caller must audit the source; this type is NOT proof of a real-world agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmpiricalPairedBatch {
    pub source_id: String,
    pub observations: Vec<PairedObservation>,
}

impl EmpiricalPairedBatch {
    pub fn validate(&self) -> Result<(), String> {
        if self.source_id.trim().is_empty() {
            return Err("empirical source_id is required".into());
        }
        if self.observations.len() < 2 {
            return Err("at least two independent paired runs are required".into());
        }
        let mut ids = HashSet::new();
        for obs in &self.observations {
            if obs.run_id.trim().is_empty() || !ids.insert(obs.run_id.as_str()) {
                return Err("independent paired run IDs must be nonempty and unique".into());
            }
            if !obs.low.is_finite() || !obs.high.is_finite() {
                return Err("non-finite empirical observation".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmpiricalAnovaGroup {
    pub cause_name: String,
    pub batch: EmpiricalPairedBatch,
}

pub struct StatisticalEngine {
    pub alpha: f64,
}

impl Default for StatisticalEngine {
    fn default() -> Self {
        Self { alpha: 0.05 }
    }
}

impl StatisticalEngine {
    fn validate_alpha(&self) -> Result<(), String> {
        if !self.alpha.is_finite() || self.alpha <= 0.0 || self.alpha >= 1.0 {
            Err("alpha must be finite and between 0 and 1".into())
        } else {
            Ok(())
        }
    }

    /// Synthetic observations can only yield descriptive effects. A config
    /// string cannot turn reference-agent data into empirical evidence.
    pub fn compute_effect(
        &self,
        cause_name: &str,
        low_obs: &[f64],
        high_obs: &[f64],
        config: &ExperimentConfig,
    ) -> Result<EffectAnalysis, String> {
        self.validate_alpha()?;
        if config.evidence_kind != "synthetic" {
            return Err(
                "configuration labels cannot certify empirical evidence; use compute_empirical_effect with raw paired runs and provenance"
                    .into(),
            );
        }
        if low_obs.len() != high_obs.len() || low_obs.is_empty() {
            return Err("paired observations must be nonempty and have equal lengths".into());
        }
        if low_obs.iter().chain(high_obs.iter()).any(|x| !x.is_finite()) {
            return Err("non-finite observation".into());
        }
        let n = low_obs.len() as f64;
        let mean_low = low_obs.iter().sum::<f64>() / n;
        let mean_high = high_obs.iter().sum::<f64>() / n;
        Ok(EffectAnalysis {
            cause_name: cause_name.to_string(),
            mean_low,
            mean_high,
            effect_size: mean_high - mean_low,
            std_dev: None,
            confidence_interval: None,
            p_value: None,
            is_significant: None,
            n_pairs: low_obs.len(),
            inference_note: "synthetic: descriptive effect only".into(),
        })
    }

    /// Paired Student t test on independent, provenance-labelled repetitions.
    /// The source claim must still be independently audited by the experimenter.
    pub fn compute_empirical_effect(
        &self,
        cause_name: &str,
        batch: &EmpiricalPairedBatch,
    ) -> Result<EffectAnalysis, String> {
        self.validate_alpha()?;
        batch.validate()?;
        let n = batch.observations.len();
        let nf = n as f64;
        let mean_low = batch.observations.iter().map(|x| x.low).sum::<f64>() / nf;
        let mean_high = batch.observations.iter().map(|x| x.high).sum::<f64>() / nf;
        let diffs: Vec<f64> = batch.observations.iter().map(|x| x.high - x.low).collect();
        if diffs.iter().any(|x| !x.is_finite()) {
            return Err("non-finite paired difference".into());
        }
        let effect = diffs.iter().sum::<f64>() / nf;
        let variance = diffs.iter().map(|x| (x - effect).powi(2)).sum::<f64>() / (nf - 1.0);
        let std_dev = variance.sqrt();
        if !std_dev.is_finite() {
            return Err("non-finite standard deviation".into());
        }
        let (confidence_interval, p_value, is_significant, inference_note) = if std_dev == 0.0 {
            // No valid variance estimate for Student inference. Do not fabricate p=0/1.
            (
                None,
                None,
                None,
                "degenerate zero sample variance: inference withheld".to_string(),
            )
        } else {
            let se = std_dev / nf.sqrt();
            let t = effect / se;
            let df = nf - 1.0;
            let p = student_two_sided_p(t, df)?;
            let critical = student_t_critical(df, self.alpha)?;
            (
                Some((effect - critical * se, effect + critical * se)),
                Some(p),
                Some(p < self.alpha),
                "paired Student t test; independence and provenance asserted by source, not cryptographically verified".to_string(),
            )
        };
        Ok(EffectAnalysis {
            cause_name: cause_name.into(),
            mean_low,
            mean_high,
            effect_size: effect,
            std_dev: Some(std_dev),
            confidence_interval,
            p_value,
            is_significant,
            n_pairs: n,
            inference_note,
        })
    }

    /// Aggregated effect summaries do not contain within-group observations.
    /// They cannot support an ANOVA, irrespective of the evidence label.
    pub fn compute_anova(&self, _analyses: &[EffectAnalysis], _evidence_kind: &str) -> AnovaResult {
        AnovaResult {
            f_statistic: None,
            p_value: None,
            eta_squared: None,
            is_significant: None,
            inference_available: false,
            inference_reason: "aggregate summaries are insufficient for ANOVA; supply raw independent paired groups".into(),
        }
    }

    /// One-way between-groups ANOVA on independent paired-effect observations.
    /// Reused run IDs across groups are refused (use repeated-measures methods).
    pub fn compute_empirical_anova(
        &self,
        groups: &[EmpiricalAnovaGroup],
    ) -> Result<AnovaResult, String> {
        self.validate_alpha()?;
        if groups.len() < 2 {
            return Err("ANOVA requires at least two independent groups".into());
        }
        let mut all_ids = HashSet::new();
        let mut group_names = HashSet::new();
        let mut data = Vec::with_capacity(groups.len());
        for group in groups {
            if group.cause_name.trim().is_empty() || !group_names.insert(group.cause_name.as_str()) {
                return Err("ANOVA group names must be nonempty and unique".into());
            }
            group.batch.validate()?;
            for obs in &group.batch.observations {
                if !all_ids.insert((group.batch.source_id.as_str(), obs.run_id.as_str())) {
                    return Err("run ID is shared between groups; independence not established".into());
                }
            }
            let effects: Vec<f64> = group.batch.observations.iter().map(|o| o.high - o.low).collect();
            if effects.iter().any(|x| !x.is_finite()) {
                return Err("non-finite effect in ANOVA".into());
            }
            data.push(effects);
        }
        let n = data.iter().map(Vec::len).sum::<usize>() as f64;
        let k = data.len() as f64;
        let grand_mean = data.iter().flatten().sum::<f64>() / n;
        let mut ss_between = 0.0;
        let mut ss_within = 0.0;
        for values in &data {
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            ss_between += values.len() as f64 * (mean - grand_mean).powi(2);
            ss_within += values.iter().map(|v| (v - mean).powi(2)).sum::<f64>();
        }
        let total = ss_between + ss_within;
        let eta_squared = if total == 0.0 { 0.0 } else { ss_between / total };
        let (f_stat, p) = if ss_within == 0.0 {
            if ss_between == 0.0 { (0.0, 1.0) } else { (f64::INFINITY, 0.0) }
        } else {
            let f = (ss_between / (k - 1.0)) / (ss_within / (n - k));
            let p = f_survival(f, k - 1.0, n - k)?;
            (f, p)
        };
        Ok(AnovaResult {
            f_statistic: Some(f_stat),
            p_value: Some(p),
            eta_squared: Some(eta_squared),
            is_significant: Some(p < self.alpha),
            inference_available: true,
            inference_reason: "one-way between-groups ANOVA on raw paired effects; independent source provenance must be audited".into(),
        })
    }
}

// Lanczos log-gamma for arguments >= 0.5; needed by incomplete beta.
fn log_gamma(z: f64) -> f64 {
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    let x0 = z - 1.0;
    let mut acc = C[0];
    for (i, c) in C.iter().enumerate().skip(1) {
        acc += c / (x0 + i as f64);
    }
    let t = x0 + 7.5;
    0.918_938_533_204_672_7 + (x0 + 0.5) * t.ln() - t + acc.ln()
}

// Continued fraction for regularized incomplete beta, Numerical Recipes form.
fn beta_fraction(a: f64, b: f64, x: f64) -> Result<f64, String> {
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let floor = 1e-300_f64;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < floor { d = floor; }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=1000 {
        let mf = m as f64;
        let m2 = 2.0 * mf;
        let mut aa = mf * (b - mf) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < floor { d = floor; }
        c = 1.0 + aa / c;
        if c.abs() < floor { c = floor; }
        d = 1.0 / d;
        h *= d * c;
        aa = -(a + mf) * (qab + mf) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < floor { d = floor; }
        c = 1.0 + aa / c;
        if c.abs() < floor { c = floor; }
        d = 1.0 / d;
        let delta = d * c;
        h *= delta;
        if (delta - 1.0).abs() < 3e-14 {
            return Ok(h);
        }
    }
    Err("incomplete beta continued fraction did not converge".into())
}

fn regularized_beta(x: f64, a: f64, b: f64) -> Result<f64, String> {
    if !(0.0..=1.0).contains(&x) || a <= 0.0 || b <= 0.0 || !x.is_finite() {
        return Err("invalid incomplete beta arguments".into());
    }
    if x == 0.0 { return Ok(0.0); }
    if x == 1.0 { return Ok(1.0); }
    let front = (log_gamma(a + b) - log_gamma(a) - log_gamma(b)
        + a * x.ln() + b * (-x).ln_1p()).exp();
    let result = if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_fraction(a, b, x)? / a
    } else {
        1.0 - front * beta_fraction(b, a, 1.0 - x)? / b
    };
    if result.is_finite() { Ok(result.clamp(0.0, 1.0)) }
    else { Err("non-finite incomplete beta result".into()) }
}

fn student_two_sided_p(t: f64, df: f64) -> Result<f64, String> {
    if df <= 0.0 { return Err("invalid Student degrees of freedom".into()); }
    if !t.is_finite() { return Ok(0.0); }
    regularized_beta(df / (df + t * t), df / 2.0, 0.5)
}

fn student_t_critical(df: f64, alpha: f64) -> Result<f64, String> {
    let mut lo = 0.0;
    let mut hi = 1.0;
    while student_two_sided_p(hi, df)? > alpha {
        hi *= 2.0;
        if hi > 1e12 { return Err("Student critical quantile out of bounds".into()); }
    }
    for _ in 0..96 {
        let mid = (lo + hi) / 2.0;
        if student_two_sided_p(mid, df)? > alpha { lo = mid; }
        else { hi = mid; }
    }
    Ok((lo + hi) / 2.0)
}

fn f_survival(f: f64, df1: f64, df2: f64) -> Result<f64, String> {
    if f < 0.0 || !f.is_finite() || df1 <= 0.0 || df2 <= 0.0 {
        return Err("invalid F distribution arguments".into());
    }
    regularized_beta(df2 / (df2 + df1 * f), df2 / 2.0, df1 / 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(kind: &str) -> ExperimentConfig {
        ExperimentConfig {
            experiment_id: "test".into(),
            seed: 42,
            iterations: 5,
            repetitions: 5,
            evidence_kind: kind.into(),
        }
    }
    fn batch(id: &str, vals: &[f64]) -> EmpiricalPairedBatch {
        EmpiricalPairedBatch {
            source_id: id.into(),
            observations: vals.iter().enumerate().map(|(i, x)| PairedObservation {
                run_id: format!("run-{i}"),
                low: 0.0,
                high: *x,
            }).collect(),
        }
    }

    #[test]
    fn descriptive_is_never_inferential() {
        let stat = StatisticalEngine::default();
        let effect = stat.compute_effect("scenario", &[10.0, 10.0], &[20.0, 21.0], &config("synthetic")).unwrap();
        assert_eq!(effect.n_pairs, 2);
        assert!(effect.p_value.is_none());
        assert!(effect.confidence_interval.is_none());
        assert!(stat.compute_effect("scenario", &[1.0], &[2.0], &config("empirical")).is_err());
    }

    #[test]
    fn paired_t_matches_scipy_reference() {
        // scipy.stats.ttest_1samp([1.,2.,3.,4.], 0).pvalue == 0.03046629166217
        let stat = StatisticalEngine::default();
        let res = stat.compute_empirical_effect("A", &batch("observation-system", &[1.0, 2.0, 3.0, 4.0])).unwrap();
        assert!((res.p_value.unwrap() - 0.030466291662170977).abs() < 1e-8);
        assert!(res.is_significant.unwrap());
        let ci = res.confidence_interval.unwrap();
        assert!(ci.0 > 0.0 && ci.1 > ci.0);
    }

    #[test]
    fn opposing_effects_are_not_significant() {
        let res = StatisticalEngine::default().compute_empirical_effect("A", &batch("meter", &[1.0, -1.0])).unwrap();
        assert!((res.p_value.unwrap() - 1.0).abs() < 1e-10);
        assert_eq!(res.is_significant, Some(false));
    }

    #[test]
    fn degenerate_and_duplicate_runs_withhold_inference() {
        let stat = StatisticalEngine::default();
        let const_sample = stat.compute_empirical_effect("A", &batch("meter", &[1.0, 1.0])).unwrap();
        assert!(const_sample.p_value.is_none());
        assert!(const_sample.confidence_interval.is_none());
        let mut dup = batch("meter", &[1.0, 2.0]);
        dup.observations[1].run_id = "run-0".into();
        assert!(stat.compute_empirical_effect("A", &dup).is_err());
    }

    #[test]
    fn anova_matches_scipy_and_depends_on_data() {
        // scipy.stats.f_oneway([0.,1.,2.,3.],[5.,6.,7.,8.]) = (30,0.00154742121454)
        let stat = StatisticalEngine::default();
        let groups = vec![
            EmpiricalAnovaGroup { cause_name: "A".into(), batch: batch("system-a", &[0.0, 1.0, 2.0, 3.0]) },
            EmpiricalAnovaGroup { cause_name: "B".into(), batch: batch("system-b", &[5.0, 6.0, 7.0, 8.0]) },
        ];
        let result = stat.compute_empirical_anova(&groups).unwrap();
        assert!((result.f_statistic.unwrap() - 30.0).abs() < 1e-10);
        assert!((result.p_value.unwrap() - 0.0015474212145409388).abs() < 1e-8);
        assert!((result.eta_squared.unwrap() - 50.0 / 60.0).abs() < 1e-10);
        let flattened = stat.compute_anova(&[], "empirical");
        assert!(!flattened.inference_available);
        assert!(flattened.f_statistic.is_none());
    }

    #[test]
    fn anova_refuses_repeated_run_ids() {
        let stat = StatisticalEngine::default();
        let groups = vec![
            EmpiricalAnovaGroup { cause_name: "A".into(), batch: batch("same-source", &[0.0, 1.0]) },
            EmpiricalAnovaGroup { cause_name: "B".into(), batch: batch("same-source", &[2.0, 3.0]) },
        ];
        assert!(stat.compute_empirical_anova(&groups).is_err());
    }
}
