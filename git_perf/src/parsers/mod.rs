//! Parsers for external measurement formats
//!
//! This module provides parsers for various test and benchmark output formats,
//! converting them into the common `ParsedMeasurement` type.

pub mod criterion_json;
pub mod junit_xml;
pub mod types;

// Re-export commonly used types
pub use criterion_json::CriterionJsonParser;
pub use junit_xml::JunitXmlParser;
pub use types::{
    BenchStatistics, BenchmarkMeasurement, ParsedMeasurement, Parser, TestMeasurement, TestStatus,
};
