use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct MeasurementSummary {
    pub epoch: u32,
    pub val: f64,
}

#[derive(Debug)]
pub struct CommitSummary {
    pub commit: String,
    pub measurement: Option<MeasurementSummary>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct MeasurementData {
    pub epoch: u32,
    pub name: String,
    pub timestamp: f64,
    pub val: f64,
    pub key_values: HashMap<String, String>,
}

// TODO double check CLONE addition
#[derive(Debug, PartialEq, Clone)]
pub struct Commit {
    pub commit: String,
    pub title: String,
    pub author: String,
    pub measurements: Vec<MeasurementData>,
}

impl MeasurementData {
    /// Checks if this measurement matches all the specified key-value criteria.
    /// Returns true if all key-value pairs in `criteria` exist in this measurement
    /// with matching values.
    #[must_use]
    pub fn matches_key_values(&self, criteria: &[(String, String)]) -> bool {
        self.key_values_is_superset_of(criteria)
    }

    /// Checks if this measurement's key-values form a superset of the given criteria.
    /// In other words, verifies that criteria ⊆ measurement.key_values.
    /// Returns true if all key-value pairs in `criteria` exist in this measurement's
    /// key_values with matching values.
    #[must_use]
    pub fn key_values_is_superset_of(&self, criteria: &[(String, String)]) -> bool {
        criteria
            .iter()
            .all(|(k, v)| self.key_values.get(k).map(|mv| v == mv).unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_key_values() {
        let mut key_values = HashMap::new();
        key_values.insert("env".to_string(), "production".to_string());
        key_values.insert("branch".to_string(), "main".to_string());
        key_values.insert("cpu".to_string(), "x64".to_string());

        let measurement = MeasurementData {
            epoch: 1,
            name: "test_measurement".to_string(),
            timestamp: 1234567890.0,
            val: 42.0,
            key_values,
        };

        // Test empty criteria (should always match)
        assert!(measurement.matches_key_values(&[]));

        // Test single matching criterion
        assert!(measurement.matches_key_values(&[("env".to_string(), "production".to_string())]));

        // Test multiple matching criteria
        assert!(measurement.matches_key_values(&[
            ("env".to_string(), "production".to_string()),
            ("branch".to_string(), "main".to_string()),
        ]));

        // Test all matching criteria
        assert!(measurement.matches_key_values(&[
            ("env".to_string(), "production".to_string()),
            ("branch".to_string(), "main".to_string()),
            ("cpu".to_string(), "x64".to_string()),
        ]));

        // Test non-matching value
        assert!(!measurement.matches_key_values(&[("env".to_string(), "staging".to_string())]));

        // Test non-existing key
        assert!(!measurement.matches_key_values(&[("os".to_string(), "linux".to_string())]));

        // Test mixed (some match, some don't) - should fail
        assert!(!measurement.matches_key_values(&[
            ("env".to_string(), "production".to_string()), // matches
            ("branch".to_string(), "develop".to_string()), // doesn't match
        ]));
    }
}
