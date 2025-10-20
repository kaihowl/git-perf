use git_perf::parsers::{CriterionJsonParser, JunitXmlParser, Parser};
use std::fs;

fn main() {
    println!("=== Testing JUnit XML Parser ===\n");

    let junit_xml = fs::read_to_string("/tmp/real_junit.xml").expect("Failed to read JUnit XML");
    let junit_parser = JunitXmlParser;

    match junit_parser.parse(&junit_xml) {
        Ok(measurements) => {
            println!(
                "Successfully parsed {} measurements from JUnit XML:",
                measurements.len()
            );
            for (i, m) in measurements.iter().enumerate() {
                println!("  {}. {:?}", i + 1, m);
            }
        }
        Err(e) => {
            eprintln!("ERROR parsing JUnit XML: {}", e);
        }
    }

    println!("\n=== Testing Criterion JSON Parser (v2 with group field) ===\n");

    let criterion_json = fs::read_to_string("/tmp/real_criterion_v2.json")
        .expect("Failed to read Criterion JSON v2");
    let criterion_parser = CriterionJsonParser;

    match criterion_parser.parse(&criterion_json) {
        Ok(measurements) => {
            println!(
                "Successfully parsed {} measurements from Criterion JSON:",
                measurements.len()
            );
            for (i, m) in measurements.iter().enumerate() {
                println!("  {}. {:?}", i + 1, m);
            }
        }
        Err(e) => {
            eprintln!("ERROR parsing Criterion JSON: {}", e);
        }
    }
}
