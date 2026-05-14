//! Plugin performance statistics, thresholds, alerts, and comparison

use serde::{Deserialize, Serialize};

/// Plugin performance and usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStats {
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub min_execution_time_ms: Option<u64>,
    pub max_execution_time_ms: Option<u64>,
    pub avg_execution_time_ms: Option<f64>,
    pub p95_execution_time_ms: Option<u64>,
    pub memory_usage_bytes: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
    pub last_call_timestamp: Option<i64>,
    #[serde(skip)]
    execution_times: std::collections::VecDeque<u64>,
    pub error_distribution: std::collections::HashMap<String, u64>,
}

impl PluginStats {
    const MAX_EXECUTION_TIMES: usize = 1000;

    pub fn new() -> Self {
        Self {
            total_calls: 0,
            successful_calls: 0,
            failed_calls: 0,
            min_execution_time_ms: None,
            max_execution_time_ms: None,
            avg_execution_time_ms: None,
            p95_execution_time_ms: None,
            memory_usage_bytes: None,
            peak_memory_bytes: None,
            last_call_timestamp: None,
            execution_times: std::collections::VecDeque::with_capacity(Self::MAX_EXECUTION_TIMES),
            error_distribution: std::collections::HashMap::new(),
        }
    }

    pub fn record_success(&mut self, execution_time_ms: u64) {
        self.total_calls += 1;
        self.successful_calls += 1;
        self.update_execution_time(execution_time_ms);
        self.last_call_timestamp = Some(chrono::Utc::now().timestamp());
    }

    pub fn record_failure(&mut self, error_type: Option<&str>) {
        self.total_calls += 1;
        self.failed_calls += 1;
        self.last_call_timestamp = Some(chrono::Utc::now().timestamp());
        let err_type = error_type.unwrap_or("Unknown");
        *self.error_distribution.entry(err_type.to_string()).or_insert(0) += 1;
    }

    pub fn update_memory_usage(&mut self, bytes: u64) {
        self.memory_usage_bytes = Some(bytes);
        self.peak_memory_bytes = Some(
            self.peak_memory_bytes.map(|peak| peak.max(bytes)).unwrap_or(bytes),
        );
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_calls == 0 { 0.0 }
        else { (self.successful_calls as f64 / self.total_calls as f64) * 100.0 }
    }

    pub fn error_distribution_sorted(&self) -> Vec<(String, u64)> {
        let mut distribution: Vec<(String, u64)> = self.error_distribution
            .iter().map(|(k, v)| (k.clone(), *v)).collect();
        distribution.sort_by(|a, b| b.1.cmp(&a.1));
        distribution
    }

    // ── Private ──

    fn update_execution_time(&mut self, time_ms: u64) {
        self.min_execution_time_ms = Some(
            self.min_execution_time_ms.map(|min| min.min(time_ms)).unwrap_or(time_ms),
        );
        self.max_execution_time_ms = Some(
            self.max_execution_time_ms.map(|max| max.max(time_ms)).unwrap_or(time_ms),
        );
        let current_avg = self.avg_execution_time_ms.unwrap_or(0.0);
        let count = self.successful_calls as f64;
        self.avg_execution_time_ms = Some((current_avg * (count - 1.0) + time_ms as f64) / count);

        if self.execution_times.len() >= Self::MAX_EXECUTION_TIMES {
            self.execution_times.pop_front();
        }
        self.execution_times.push_back(time_ms);
        self.calculate_p95();
    }

    fn calculate_p95(&mut self) {
        if self.execution_times.is_empty() {
            self.p95_execution_time_ms = None;
            return;
        }
        let mut sorted_times: Vec<u64> = self.execution_times.iter().copied().collect();
        sorted_times.sort_unstable();
        let p95_index = ((sorted_times.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
        self.p95_execution_time_ms = Some(sorted_times[p95_index]);
    }
}

impl Default for PluginStats {
    fn default() -> Self { Self::new() }
}

/// Performance thresholds for alerting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    pub max_avg_execution_time_ms: Option<u64>,
    pub max_p95_execution_time_ms: Option<u64>,
    pub max_memory_bytes: Option<u64>,
    pub min_success_rate: Option<f64>,
    pub max_error_rate: Option<f64>,
}

impl PerformanceThresholds {
    pub fn new() -> Self {
        Self { max_avg_execution_time_ms: None, max_p95_execution_time_ms: None, max_memory_bytes: None, min_success_rate: None, max_error_rate: None }
    }

    pub fn default_limits() -> Self {
        Self {
            max_avg_execution_time_ms: Some(1000),
            max_p95_execution_time_ms: Some(5000),
            max_memory_bytes: Some(512 * 1024 * 1024),
            min_success_rate: Some(95.0),
            max_error_rate: Some(5.0),
        }
    }
}

impl Default for PerformanceThresholds {
    fn default() -> Self { Self::new() }
}

/// Performance alert indicating a threshold violation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAlert {
    pub alert_type: AlertType,
    pub current_value: f64,
    pub threshold_value: f64,
    pub message: String,
}

/// Types of performance alerts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AlertType {
    AvgExecutionTime,
    P95ExecutionTime,
    MemoryUsage,
    LowSuccessRate,
    HighErrorRate,
}

impl PluginStats {
    pub fn check_thresholds(&self, thresholds: &PerformanceThresholds) -> Vec<PerformanceAlert> {
        let mut alerts = Vec::new();

        if let (Some(avg), Some(max_avg)) = (self.avg_execution_time_ms, thresholds.max_avg_execution_time_ms) {
            if avg > max_avg as f64 {
                alerts.push(PerformanceAlert {
                    alert_type: AlertType::AvgExecutionTime,
                    current_value: avg,
                    threshold_value: max_avg as f64,
                    message: format!("Average execution time ({:.2}ms) exceeds threshold ({}ms)", avg, max_avg),
                });
            }
        }

        if let (Some(p95), Some(max_p95)) = (self.p95_execution_time_ms, thresholds.max_p95_execution_time_ms) {
            if p95 > max_p95 {
                alerts.push(PerformanceAlert {
                    alert_type: AlertType::P95ExecutionTime,
                    current_value: p95 as f64,
                    threshold_value: max_p95 as f64,
                    message: format!("P95 execution time ({}ms) exceeds threshold ({}ms)", p95, max_p95),
                });
            }
        }

        if let (Some(memory), Some(max_memory)) = (self.memory_usage_bytes, thresholds.max_memory_bytes) {
            if memory > max_memory {
                alerts.push(PerformanceAlert {
                    alert_type: AlertType::MemoryUsage,
                    current_value: memory as f64,
                    threshold_value: max_memory as f64,
                    message: format!("Memory usage ({} bytes) exceeds threshold ({} bytes)", memory, max_memory),
                });
            }
        }

        if let Some(min_success) = thresholds.min_success_rate {
            let success_rate = self.success_rate();
            if success_rate < min_success {
                alerts.push(PerformanceAlert {
                    alert_type: AlertType::LowSuccessRate,
                    current_value: success_rate,
                    threshold_value: min_success,
                    message: format!("Success rate ({:.2}%) is below threshold ({:.2}%)", success_rate, min_success),
                });
            }
        }

        if let Some(max_error) = thresholds.max_error_rate {
            let error_rate = if self.total_calls == 0 { 0.0 }
            else { (self.failed_calls as f64 / self.total_calls as f64) * 100.0 };
            if error_rate > max_error {
                alerts.push(PerformanceAlert {
                    alert_type: AlertType::HighErrorRate,
                    current_value: error_rate,
                    threshold_value: max_error,
                    message: format!("Error rate ({:.2}%) exceeds threshold ({:.2}%)", error_rate, max_error),
                });
            }
        }

        alerts
    }

    pub fn export_json(&self) -> crate::core::error::Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| crate::core::error::TingError::SerializationError(e.to_string()))
    }

    pub fn export_csv(&self) -> String {
        let mut csv = String::new();
        csv.push_str("total_calls,successful_calls,failed_calls,min_execution_time_ms,max_execution_time_ms,avg_execution_time_ms,p95_execution_time_ms,memory_usage_bytes,peak_memory_bytes,success_rate,last_call_timestamp\n");
        csv.push_str(&format!("{},{},{},{},{},{},{},{},{},{:.2},{}\n",
            self.total_calls, self.successful_calls, self.failed_calls,
            self.min_execution_time_ms.map(|v| v.to_string()).unwrap_or_default(),
            self.max_execution_time_ms.map(|v| v.to_string()).unwrap_or_default(),
            self.avg_execution_time_ms.map(|v| format!("{:.2}", v)).unwrap_or_default(),
            self.p95_execution_time_ms.map(|v| v.to_string()).unwrap_or_default(),
            self.memory_usage_bytes.map(|v| v.to_string()).unwrap_or_default(),
            self.peak_memory_bytes.map(|v| v.to_string()).unwrap_or_default(),
            self.success_rate(),
            self.last_call_timestamp.map(|v| v.to_string()).unwrap_or_default(),
        ));
        csv
    }

    pub fn compare(&self, other: &PluginStats) -> PerformanceComparison {
        PerformanceComparison {
            total_calls_diff: other.total_calls as i64 - self.total_calls as i64,
            successful_calls_diff: other.successful_calls as i64 - self.successful_calls as i64,
            failed_calls_diff: other.failed_calls as i64 - self.failed_calls as i64,
            avg_execution_time_diff: match (self.avg_execution_time_ms, other.avg_execution_time_ms) {
                (Some(a), Some(b)) => Some(b - a), _ => None,
            },
            p95_execution_time_diff: match (self.p95_execution_time_ms, other.p95_execution_time_ms) {
                (Some(a), Some(b)) => Some(b as i64 - a as i64), _ => None,
            },
            memory_usage_diff: match (self.memory_usage_bytes, other.memory_usage_bytes) {
                (Some(a), Some(b)) => Some(b as i64 - a as i64), _ => None,
            },
            success_rate_diff: other.success_rate() - self.success_rate(),
        }
    }
}

/// Result of comparing two PluginStats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceComparison {
    pub total_calls_diff: i64,
    pub successful_calls_diff: i64,
    pub failed_calls_diff: i64,
    pub avg_execution_time_diff: Option<f64>,
    pub p95_execution_time_diff: Option<i64>,
    pub memory_usage_diff: Option<i64>,
    pub success_rate_diff: f64,
}

impl PerformanceComparison {
    pub fn is_improvement(&self) -> bool {
        let exec_time_improved = self.avg_execution_time_diff.map(|diff| diff < 0.0).unwrap_or(true);
        let p95_improved = self.p95_execution_time_diff.map(|diff| diff < 0).unwrap_or(true);
        let memory_improved = self.memory_usage_diff.map(|diff| diff < 0).unwrap_or(true);
        let success_improved = self.success_rate_diff > 0.0;
        [exec_time_improved, p95_improved, memory_improved, success_improved]
            .iter().filter(|&&x| x).count() >= 3
    }

    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(diff) = self.avg_execution_time_diff {
            let direction = if diff < 0.0 { "decreased" } else { "increased" };
            parts.push(format!("Avg execution time {} by {:.2}ms", direction, diff.abs()));
        }
        if let Some(diff) = self.p95_execution_time_diff {
            let direction = if diff < 0 { "decreased" } else { "increased" };
            parts.push(format!("P95 execution time {} by {}ms", direction, diff.abs()));
        }
        if let Some(diff) = self.memory_usage_diff {
            let direction = if diff < 0 { "decreased" } else { "increased" };
            parts.push(format!("Memory usage {} by {} bytes", direction, diff.abs()));
        }
        let direction = if self.success_rate_diff > 0.0 { "increased" } else { "decreased" };
        parts.push(format!("Success rate {} by {:.2}%", direction, self.success_rate_diff.abs()));
        parts.join(", ")
    }
}
