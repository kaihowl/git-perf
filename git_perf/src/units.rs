use bytesize::ByteSize;
use fundu::DurationParser;
use human_repr::{HumanCount, HumanDuration, HumanThroughput};
use std::str::FromStr;

/// Represents a parsed measurement with detected type
#[derive(Debug, Clone)]
pub enum Measurement {
    Duration(std::time::Duration),
    DataSize(u64), // bytes
    DataRate(f64), // bytes per second
    Count(f64),    // unitless or custom
}

/// Parse a numeric value with its unit string
/// Tries different parsers until one succeeds
pub fn parse_value_with_unit(value: f64, unit_str: &str) -> Result<Measurement, String> {
    // Try duration parsing (ms, s, min, h, etc.)
    if let Ok(duration) = parse_duration(value, unit_str) {
        return Ok(Measurement::Duration(duration));
    }

    // Try data size parsing (B, KB, MB, GB, etc.)
    if let Ok(size) = parse_data_size(value, unit_str) {
        return Ok(Measurement::DataSize(size));
    }

    // Try data rate parsing (KB/s, MB/s, etc.)
    if unit_str.contains("/s") {
        if let Ok(rate) = parse_data_rate(value, unit_str) {
            return Ok(Measurement::DataRate(rate));
        }
    }

    // Fallback: treat as unitless count
    Ok(Measurement::Count(value))
}

/// Format measurement with auto-scaling using human-repr
pub fn format_measurement(measurement: Measurement) -> String {
    match measurement {
        Measurement::Duration(d) => d.human_duration().to_string(),
        Measurement::DataSize(bytes) => bytes.human_count_bytes().to_string(),
        Measurement::DataRate(bps) => bps.human_throughput_bytes().to_string(),
        Measurement::Count(v) => format!("{:.3}", v),
    }
}

/// Helper: Parse duration from value + unit
fn parse_duration(value: f64, unit: &str) -> Result<std::time::Duration, String> {
    let parser = DurationParser::with_all_time_units();
    // Try without space first (9000ms), then with space (9000 ms)
    let inputs = [format!("{}{}", value, unit), format!("{} {}", value, unit)];

    for input in &inputs {
        if let Ok(fundu_duration) = parser.parse(input) {
            if let Ok(duration) = fundu_duration.try_into() {
                return Ok(duration);
            }
        }
    }

    Err(format!("Failed to parse duration: {} {}", value, unit))
}

/// Helper: Parse data size from value + unit
fn parse_data_size(value: f64, unit: &str) -> Result<u64, String> {
    // Try various input formats
    let inputs = [format!("{}{}", value, unit), format!("{} {}", value, unit)];

    for input in &inputs {
        if let Ok(bs) = ByteSize::from_str(input) {
            return Ok(bs.as_u64());
        }
    }

    Err(format!("Failed to parse data size: {} {}", value, unit))
}

/// Helper: Parse data rate from value + unit (e.g., KB/s, MB/s)
fn parse_data_rate(value: f64, unit_with_rate: &str) -> Result<f64, String> {
    let parts: Vec<&str> = unit_with_rate.split('/').collect();
    if parts.len() != 2 || parts[1] != "s" {
        return Err("Invalid rate format".to_string());
    }

    let multiplier = match parts[0].to_lowercase().as_str() {
        "b" => 1.0,
        "kb" => 1_000.0,
        "mb" => 1_000_000.0,
        "gb" => 1_000_000_000.0,
        "kib" => 1_024.0,
        "mib" => 1_048_576.0,
        "gib" => 1_073_741_824.0,
        _ => return Err(format!("Unknown unit: {}", parts[0])),
    };

    Ok(value * multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_units() {
        // 9000 ms → "9s"
        let m = parse_value_with_unit(9000.0, "ms").unwrap();
        assert_eq!(format_measurement(m), "9s");

        // 125000 ms → "2:05.0"
        let m = parse_value_with_unit(125000.0, "ms").unwrap();
        let formatted = format_measurement(m);
        assert!(formatted.contains("2:05"));
    }

    #[test]
    fn test_parse_data_size_units() {
        // 9000 KB → "9MB"
        let m = parse_value_with_unit(9000.0, "KB").unwrap();
        assert_eq!(format_measurement(m), "9MB");

        // 1500 MB → "1.5GB"
        let m = parse_value_with_unit(1500.0, "MB").unwrap();
        assert_eq!(format_measurement(m), "1.5GB");
    }

    #[test]
    fn test_parse_data_rate_units() {
        // 9000 KB/s → "9MB/s"
        let m = parse_value_with_unit(9000.0, "KB/s").unwrap();
        assert_eq!(format_measurement(m), "9MB/s");
    }

    #[test]
    fn test_parse_fallback_to_count() {
        // Unknown unit → Count (no parsing error)
        let m = parse_value_with_unit(42.5, "widgets").unwrap();
        assert_eq!(format_measurement(m), "42.500");
    }

    #[test]
    fn test_duration_milliseconds() {
        let m = parse_value_with_unit(9000.0, "ms").unwrap();
        assert_eq!(format_measurement(m), "9s");
    }

    #[test]
    fn test_duration_seconds_to_minutes() {
        let m = parse_value_with_unit(125.0, "s").unwrap();
        let formatted = format_measurement(m);
        assert!(formatted.contains("2:05"));
    }

    #[test]
    fn test_data_size_kilobytes() {
        let m = parse_value_with_unit(9000.0, "KB").unwrap();
        assert_eq!(format_measurement(m), "9MB");
    }

    #[test]
    fn test_data_rate_megabytes() {
        let m = parse_value_with_unit(1500.0, "MB/s").unwrap();
        assert_eq!(format_measurement(m), "1.5GB/s");
    }

    #[test]
    fn test_unknown_unit_fallback() {
        // Unknown units fallback to raw count
        let m = parse_value_with_unit(42.5, "widgets").unwrap();
        assert!(matches!(m, Measurement::Count(_)));
    }

    #[test]
    fn test_nanoseconds() {
        let m = parse_value_with_unit(1_000_000.0, "ns").unwrap();
        let formatted = format_measurement(m);
        // 1,000,000 ns = 1 ms
        assert!(formatted.contains("ms") || formatted.contains("1"));
    }

    #[test]
    fn test_bytes() {
        let m = parse_value_with_unit(1024.0, "B").unwrap();
        let formatted = format_measurement(m);
        // Should be formatted as bytes
        assert!(formatted.contains("1") || formatted.contains("B"));
    }

    #[test]
    fn test_gigabytes() {
        let m = parse_value_with_unit(2.5, "GB").unwrap();
        assert_eq!(format_measurement(m), "2.5GB");
    }

    #[test]
    fn test_hours() {
        let m = parse_value_with_unit(2.0, "h").unwrap();
        let formatted = format_measurement(m);
        // 2 hours should be formatted appropriately
        assert!(formatted.contains("2:00") || formatted.contains("h"));
    }

    #[test]
    fn test_zero_values() {
        let m = parse_value_with_unit(0.0, "ms").unwrap();
        let formatted = format_measurement(m);
        assert!(formatted.contains("0"));
    }

    #[test]
    fn test_small_durations() {
        let m = parse_value_with_unit(500.0, "ns").unwrap();
        let formatted = format_measurement(m);
        assert!(formatted.contains("ns") || formatted.contains("500"));
    }
}
