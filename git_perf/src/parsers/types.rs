use std::collections::HashMap;

/// Represents a parsed measurement from various input formats
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedMeasurement {
    Test(TestMeasurement),
    Benchmark(BenchmarkMeasurement),
}

/// Represents a test measurement from test runners (e.g., JUnit XML)
#[derive(Debug, Clone, PartialEq)]
pub struct TestMeasurement {
    pub name: String,
    pub duration: Option<std::time::Duration>,
    pub status: TestStatus,
    pub metadata: HashMap<String, String>,
}

/// Test execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    Passed,
    Failed,
    Error,
    Skipped,
}

impl TestStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TestStatus::Passed => "passed",
            TestStatus::Failed => "failed",
            TestStatus::Error => "error",
            TestStatus::Skipped => "skipped",
        }
    }
}

/// Represents a benchmark measurement from benchmark tools (e.g., criterion)
#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkMeasurement {
    pub id: String,
    pub statistics: BenchStatistics,
    pub metadata: HashMap<String, String>,
}

/// Benchmark statistics from criterion output
#[derive(Debug, Clone, PartialEq)]
pub struct BenchStatistics {
    pub mean_ns: Option<f64>,
    pub median_ns: Option<f64>,
    pub slope_ns: Option<f64>,
    pub mad_ns: Option<f64>,
    pub unit: String,
}

/// Trait for parsers that convert external formats to ParsedMeasurement
pub trait Parser {
    fn parse(&self, input: &str) -> anyhow::Result<Vec<ParsedMeasurement>>;
}
