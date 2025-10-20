use crate::parsers::types::{ParsedMeasurement, Parser, TestMeasurement, TestStatus};
use anyhow::Result;
use quick_xml::de::from_str;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

/// Parser for JUnit XML format
pub struct JunitXmlParser;

impl Parser for JunitXmlParser {
    fn parse(&self, input: &str) -> Result<Vec<ParsedMeasurement>> {
        // Check the root element name to determine which structure to parse
        let trimmed = input.trim();
        let is_testsuites = trimmed.contains("<testsuites");

        if is_testsuites {
            // Parse as multiple test suites
            if let Ok(testsuites) = from_str::<TestSuites>(input) {
                return Ok(testsuites.into_measurements());
            }
        } else {
            // Parse as single test suite
            if let Ok(testsuite) = from_str::<TestSuite>(input) {
                return Ok(testsuite.into_measurements());
            }
        }

        anyhow::bail!("Failed to parse JUnit XML: input is neither <testsuites> nor <testsuite>")
    }
}

/// Root element for multiple test suites
#[derive(Debug, Deserialize)]
struct TestSuites {
    #[serde(rename = "$value", default)]
    testsuite: Vec<TestSuite>,
}

impl TestSuites {
    fn into_measurements(self) -> Vec<ParsedMeasurement> {
        self.testsuite
            .into_iter()
            .flat_map(|suite| suite.into_measurements())
            .collect()
    }
}

/// A test suite containing multiple test cases
#[derive(Debug, Deserialize)]
struct TestSuite {
    #[serde(rename = "@name", default)]
    name: String,
    #[serde(rename = "$value", default)]
    testcase: Vec<TestCase>,
}

impl TestSuite {
    fn into_measurements(self) -> Vec<ParsedMeasurement> {
        self.testcase
            .into_iter()
            .map(|tc| tc.into_measurement(&self.name))
            .collect()
    }
}

/// A single test case
#[derive(Debug, Deserialize)]
struct TestCase {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@classname", default)]
    classname: String,
    #[serde(rename = "@time", default)]
    time: Option<f64>,
    #[serde(default)]
    failure: Option<Failure>,
    #[serde(default)]
    error: Option<Error>,
    #[serde(default)]
    skipped: Option<Skipped>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Failure {
    #[serde(default)]
    message: String,
    #[serde(rename = "type", default)]
    failure_type: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Error {
    #[serde(default)]
    message: String,
    #[serde(rename = "type", default)]
    error_type: String,
}

#[derive(Debug, Deserialize)]
struct Skipped {}

impl TestCase {
    fn into_measurement(self, suite_name: &str) -> ParsedMeasurement {
        let status = if self.skipped.is_some() {
            TestStatus::Skipped
        } else if self.error.is_some() {
            TestStatus::Error
        } else if self.failure.is_some() {
            TestStatus::Failed
        } else {
            TestStatus::Passed
        };

        let duration = self.time.map(Duration::from_secs_f64);

        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), "test".to_string());
        if !self.classname.is_empty() {
            metadata.insert("classname".to_string(), self.classname);
        }
        if !suite_name.is_empty() {
            metadata.insert("suite".to_string(), suite_name.to_string());
        }
        metadata.insert("status".to_string(), status.as_str().to_string());

        ParsedMeasurement::Test(TestMeasurement {
            name: self.name,
            duration,
            status,
            metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_testsuite() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="my_tests" tests="2" failures="0" errors="0" skipped="0" time="3.5">
  <testcase name="test_one" classname="module::tests" time="1.5"/>
  <testcase name="test_two" classname="module::tests" time="2.0"/>
</testsuite>"#;

        let parser = JunitXmlParser;
        let result = parser.parse(xml).unwrap();

        assert_eq!(result.len(), 2);

        if let ParsedMeasurement::Test(test) = &result[0] {
            assert_eq!(test.name, "test_one");
            assert_eq!(test.duration, Some(Duration::from_secs_f64(1.5)));
            assert_eq!(test.status, TestStatus::Passed);
            assert_eq!(test.metadata.get("classname").unwrap(), "module::tests");
            assert_eq!(test.metadata.get("status").unwrap(), "passed");
        } else {
            panic!("Expected Test measurement");
        }
    }

    #[test]
    fn test_parse_testsuites() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites tests="3" failures="1" errors="0" skipped="1" time="5.2">
  <testsuite name="suite_one" tests="2" failures="0" time="3.5">
    <testcase name="test_one" classname="module::tests" time="1.5"/>
    <testcase name="test_two" classname="module::tests" time="2.0"/>
  </testsuite>
  <testsuite name="suite_two" tests="1" failures="1" time="1.7">
    <testcase name="test_three" classname="other::tests" time="1.7">
      <failure message="assertion failed" type="AssertionError"/>
    </testcase>
  </testsuite>
</testsuites>"#;

        let parser = JunitXmlParser;
        let result = parser.parse(xml).unwrap();

        assert_eq!(result.len(), 3);

        if let ParsedMeasurement::Test(test) = &result[2] {
            assert_eq!(test.name, "test_three");
            assert_eq!(test.status, TestStatus::Failed);
            assert_eq!(test.metadata.get("suite").unwrap(), "suite_two");
        } else {
            panic!("Expected Test measurement");
        }
    }

    #[test]
    fn test_parse_skipped_test() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="my_tests" tests="1" skipped="1">
  <testcase name="test_skip" classname="module::tests" time="0.0">
    <skipped/>
  </testcase>
</testsuite>"#;

        let parser = JunitXmlParser;
        let result = parser.parse(xml).unwrap();

        if let ParsedMeasurement::Test(test) = &result[0] {
            assert_eq!(test.status, TestStatus::Skipped);
        } else {
            panic!("Expected Test measurement");
        }
    }

    #[test]
    fn test_parse_error_test() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="my_tests" tests="1" errors="1">
  <testcase name="test_error" classname="module::tests" time="0.5">
    <error message="runtime error" type="RuntimeError"/>
  </testcase>
</testsuite>"#;

        let parser = JunitXmlParser;
        let result = parser.parse(xml).unwrap();

        if let ParsedMeasurement::Test(test) = &result[0] {
            assert_eq!(test.status, TestStatus::Error);
        } else {
            panic!("Expected Test measurement");
        }
    }

    #[test]
    fn test_parse_missing_time() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuite name="my_tests" tests="1">
  <testcase name="test_no_time" classname="module::tests"/>
</testsuite>"#;

        let parser = JunitXmlParser;
        let result = parser.parse(xml).unwrap();

        if let ParsedMeasurement::Test(test) = &result[0] {
            assert_eq!(test.duration, None);
        } else {
            panic!("Expected Test measurement");
        }
    }

    #[test]
    fn test_parse_invalid_xml() {
        let xml = "not valid xml";
        let parser = JunitXmlParser;
        assert!(parser.parse(xml).is_err());
    }
}
