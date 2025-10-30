pub mod audit;
pub mod basic_measure;
pub mod cli;
pub mod config;
pub mod converters;
pub mod data;
pub mod filter;
pub mod git;
pub mod import;
pub mod measurement_retrieval;
pub mod measurement_storage;
pub mod parsers;
pub mod reporting;
pub mod serialization;
pub mod size;
pub mod stats;
pub mod units;

// Test helpers module - made public for use in unit tests, integration tests, and benchmarks
// This is conditionally compiled to avoid including test code in release builds
#[doc(hidden)]
#[cfg(any(test, doctest, feature = "test-helpers"))]
pub mod test_helpers;
