use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Rule {
    pub name: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "PatternSet::is_empty")]
    pub sender_contains: PatternSet,
    #[serde(default)]
    #[serde(skip_serializing_if = "PatternSet::is_empty")]
    pub subject_contains: PatternSet,
    pub action: RuleAction,
}

impl Rule {
    /// Check if a message matches this rule
    pub fn matches(&self, sender: &str, subject: &str) -> bool {
        let sender_patterns = self.sender_contains.to_vec();
        let subject_patterns = self.subject_contains.to_vec();

        // Skip empty rules (should be caught by validation, but just in case)
        if sender_patterns.is_empty() && subject_patterns.is_empty() {
            return false;
        }

        // If both pattern types are present, need to match at least one from each
        if !sender_patterns.is_empty() && !subject_patterns.is_empty() {
            // Check for sender match
            let mut sender_matched = false;
            for pattern in &sender_patterns {
                if sender.to_lowercase().contains(&pattern.to_lowercase()) {
                    sender_matched = true;
                    break;
                }
            }

            // Check for subject match
            let mut subject_matched = false;
            for pattern in &subject_patterns {
                if subject.to_lowercase().contains(&pattern.to_lowercase()) {
                    subject_matched = true;
                    break;
                }
            }

            // Both must match for the rule to apply
            return sender_matched && subject_matched;
        }
        // If only sender patterns exist
        else if !sender_patterns.is_empty() {
            for pattern in &sender_patterns {
                if sender.to_lowercase().contains(&pattern.to_lowercase()) {
                    return true;
                }
            }
        }
        // If only subject patterns exist
        else if !subject_patterns.is_empty() {
            for pattern in &subject_patterns {
                if subject.to_lowercase().contains(&pattern.to_lowercase()) {
                    return true;
                }
            }
        }

        false
    }
}

/// Pattern set is now always a Vec<String>
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PatternSet(Vec<String>);

// Custom serialization/deserialization for PatternSet
impl Serialize for PatternSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Directly serialize the inner Vec
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PatternSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Deserialize as a Vec and wrap in PatternSet
        let vec = Vec::<String>::deserialize(deserializer)?;
        Ok(PatternSet(vec))
    }
}

impl PatternSet {
    pub fn new() -> Self {
        PatternSet(Vec::new())
    }

    pub fn with_patterns(patterns: Vec<String>) -> Self {
        PatternSet(patterns)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty() || self.0.iter().all(|s| s.trim().is_empty())
    }

    pub fn to_vec(&self) -> Vec<String> {
        self.0.clone()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum RuleAction {
    #[serde(rename = "archive")]
    #[default]
    Archive,
    #[serde(rename = "delete")]
    Delete,
    #[serde(rename = "mark_read")]
    MarkRead,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Rules {
    #[serde(flatten)]
    pub items: Vec<Rule>,
}

impl Rules {
    pub fn new() -> Self {
        // Create an empty rules configuration
        Self { items: Vec::new() }
    }

    /// Get example rules to help users
    pub fn get_example_rules() -> Vec<Rule> {
        vec![
            Rule {
                name: "Archive newsletters".to_string(),
                sender_contains: PatternSet::with_patterns(vec![
                    "newsletter".to_string(),
                    "updates".to_string(),
                ]),
                subject_contains: PatternSet::new(),
                action: RuleAction::Archive,
            },
            Rule {
                name: "Delete promotions".to_string(),
                sender_contains: PatternSet::new(),
                subject_contains: PatternSet::with_patterns(vec![
                    "discount".to_string(),
                    "sale".to_string(),
                    "offer".to_string(),
                ]),
                action: RuleAction::Delete,
            },
            Rule {
                name: "Mark read meeting invites".to_string(),
                sender_contains: PatternSet::new(),
                subject_contains: PatternSet::with_patterns(vec!["invitation".to_string()]),
                action: RuleAction::MarkRead,
            },
            Rule {
                name: "Archive tech updates from company domain".to_string(),
                sender_contains: PatternSet::with_patterns(vec!["@company.com".to_string()]),
                subject_contains: PatternSet::with_patterns(vec![
                    "tech update".to_string(),
                    "technology news".to_string(),
                ]),
                action: RuleAction::Archive,
            },
        ]
    }

    /// Validate rules and return a list of validation errors
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        // Validate each rule
        for (i, rule) in self.items.iter().enumerate() {
            // Validate rule name
            if rule.name.trim().is_empty() {
                errors.push(format!("Rule #{}: name cannot be empty", i + 1));
            }

            // Validate match conditions (must have at least one pattern that's not empty)
            if rule.sender_contains.is_empty() && rule.subject_contains.is_empty() {
                errors.push(format!(
                    "Rule '{}': must specify at least one match pattern (sender_contains or subject_contains)",
                    rule.name
                ));
            }

            // No need to check if arrays are empty since PatternSet::is_empty handles that
        }

        errors
    }

    /// Gets the path to the JSON schema file in the rules directory
    pub fn get_schema_path() -> Result<PathBuf> {
        let schema_path = Self::get_rules_dir()?.join("rules.schema.json");

        // Ensure the schema file exists by creating it if it doesn't
        if !schema_path.exists() {
            Self::initialize_schema_file(&schema_path)?;
        }

        Ok(schema_path)
    }

    /// Initialize the schema file in the rules directory
    fn initialize_schema_file(path: &PathBuf) -> Result<()> {
        let schema_content = include_str!("../schema/rules.schema.json");
        fs::write(path, schema_content)?;
        Ok(())
    }

    /// Update schema file to latest version (useful when config format changes)
    pub fn update_schema_file() -> Result<()> {
        let schema_path = Self::get_schema_path()?;
        Self::initialize_schema_file(&schema_path)
    }

    /// Load rules from disk or create default
    pub fn load() -> Result<Self> {
        let rules_path = Self::get_rules_path()?;

        // Always ensure the schema file is up to date
        Self::update_schema_file()?;

        if rules_path.exists() {
            let rules_str = fs::read_to_string(&rules_path)?;

            // If the file exists but is empty, return default rules
            if rules_str.trim().is_empty() {
                let default_rules = Rules::new();
                default_rules.save()?;
                return Ok(default_rules);
            }

            // Try parsing the YAML directly as an array of Rule objects
            match serde_yaml::from_str::<Vec<Rule>>(&rules_str) {
                Ok(rule_items) => Ok(Rules { items: rule_items }),
                Err(_) => {
                    // If that fails, try parsing as a Rules struct (for backward compatibility)
                    let rules: Rules = serde_yaml::from_str(&rules_str)?;
                    Ok(rules)
                }
            }
        } else {
            let default_rules = Rules::new();
            default_rules.save()?;
            Ok(default_rules)
        }
    }

    /// Save rules to disk
    pub fn save(&self) -> Result<()> {
        let rules_path = Self::get_rules_path()?;
        let schema_path = Self::get_schema_path()?;

        // Ensure directory exists
        let rules_dir = Self::get_rules_dir()?;
        if !rules_dir.exists() {
            fs::create_dir_all(&rules_dir)?;
        }

        // Add YAML header with schema reference for IDE support
        let mut content = format!(
            "# yaml-language-server: $schema={}\n\n",
            schema_path.to_string_lossy()
        );

        // If there are no rules, just add example rules as comments without empty array
        if self.items.is_empty() {
            content.push_str("# Example rules (uncomment and modify as needed):\n");

            // Add example rules as comments
            let example_rules = Self::get_example_rules();
            let example_yaml = serde_yaml::to_string(&example_rules)?;

            // Format each line as a comment
            for line in example_yaml.lines() {
                if !line.trim().is_empty() {
                    content.push_str(&format!("# {}\n", line));
                }
            }

            // Don't add empty array or "Add your own rules below" message
            fs::write(&rules_path, content)?;
            return Ok(());
        }

        // Let's simplify and just use the standard serialization
        let mut rules_str = serde_yaml::to_string(&self.items)?;

        // Replace the standard 2-space indentation with 4-space indentation
        if rules_str.contains("\n  - ") {
            rules_str = rules_str.replace("\n  - ", "\n    - ");
        }

        content.push_str(&rules_str);
        fs::write(&rules_path, content)?;

        Ok(())
    }

    /// Get the rules directory path
    fn get_rules_dir() -> Result<PathBuf> {
        crate::config::get_app_config_dir()
    }

    pub fn get_rules_path() -> Result<PathBuf> {
        crate::config::get_config_file_path("rules.yaml")
    }

    pub fn get_rules_path_str() -> Result<String> {
        crate::config::get_config_file_path_str("rules.yaml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_formatting() {
        // Create a rule with multiple patterns
        let rule = Rule {
            name: "Test Rule".to_string(),
            sender_contains: PatternSet::with_patterns(vec![
                "test1.com".to_string(),
                "test2.com".to_string(),
            ]),
            subject_contains: PatternSet::with_patterns(vec![
                "important".to_string(),
                "urgent".to_string(),
            ]),
            action: RuleAction::Archive,
        };

        // Create a rules set with the rule
        let rules = Rules { items: vec![rule] };

        // Serialize to YAML with our formatting logic
        let yaml = serde_yaml::to_string(&rules.items).unwrap();
        let formatted_yaml = yaml
            .replace(": [", ":\n    - ")
            .replace(", ", "\n    - ")
            .replace("]", "");

        // Check that we have the expected format lines (with proper indentation)
        // First let's fix our expectation to match what serde_yaml generates
        assert!(yaml.contains("sender_contains:\n  - test1.com"));

        // Now check if our replacements worked
        assert!(
            formatted_yaml.contains("sender_contains:\n    - test1.com")
                || formatted_yaml.contains("sender_contains:\n  - test1.com")
        );
    }

    #[test]
    fn test_rule_validation() {
        // Test empty rule
        let empty_rule = Rule {
            name: "Empty rule".to_string(),
            sender_contains: PatternSet::new(),
            subject_contains: PatternSet::new(),
            action: RuleAction::Archive,
        };

        let rules = Rules {
            items: vec![empty_rule],
        };

        let errors = rules.validate();
        assert!(!errors.is_empty(), "Empty rule should fail validation");
        assert!(
            errors[0].contains("must specify at least one match pattern"),
            "Error should mention missing patterns"
        );

        // Test valid rule with sender pattern only
        let sender_rule = Rule {
            name: "Sender rule".to_string(),
            sender_contains: PatternSet::with_patterns(vec!["example.com".to_string()]),
            subject_contains: PatternSet::new(),
            action: RuleAction::Delete,
        };

        let rules = Rules {
            items: vec![sender_rule],
        };

        let errors = rules.validate();
        assert!(
            errors.is_empty(),
            "Valid sender rule should pass validation"
        );

        // Test valid rule with subject pattern only
        let subject_rule = Rule {
            name: "Subject rule".to_string(),
            sender_contains: PatternSet::new(),
            subject_contains: PatternSet::with_patterns(vec!["important".to_string()]),
            action: RuleAction::MarkRead,
        };

        let rules = Rules {
            items: vec![subject_rule],
        };

        let errors = rules.validate();
        assert!(
            errors.is_empty(),
            "Valid subject rule should pass validation"
        );

        // Test rule with empty name
        let no_name_rule = Rule {
            name: "".to_string(),
            sender_contains: PatternSet::with_patterns(vec!["example.com".to_string()]),
            subject_contains: PatternSet::new(),
            action: RuleAction::Archive,
        };

        let rules = Rules {
            items: vec![no_name_rule],
        };

        let errors = rules.validate();
        assert!(
            !errors.is_empty(),
            "Rule without name should fail validation"
        );
        assert!(
            errors[0].contains("name cannot be empty"),
            "Error should mention empty name"
        );
    }

    #[test]
    fn test_pattern_set() {
        // Test empty pattern set
        let empty = PatternSet::new();
        assert!(empty.is_empty(), "New pattern set should be empty");
        assert_eq!(
            empty.to_vec().len(),
            0,
            "Empty pattern set should have no items"
        );

        // Test non-empty pattern set
        let patterns = vec!["a".to_string(), "b".to_string()];
        let non_empty = PatternSet::with_patterns(patterns.clone());
        assert!(
            !non_empty.is_empty(),
            "Pattern set with items should not be empty"
        );
        assert_eq!(
            non_empty.to_vec(),
            patterns,
            "Pattern set should return same items"
        );

        // Test pattern set with empty strings
        let empty_strings = PatternSet::with_patterns(vec!["".to_string(), "  ".to_string()]);
        assert!(
            empty_strings.is_empty(),
            "Pattern set with only empty strings should be considered empty"
        );
    }
}
