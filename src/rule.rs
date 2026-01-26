//! Window rules for matching and applying settings

#![allow(dead_code)]

use crate::config::WindowDecoration;
use regex::Regex;

/// A pattern for matching window properties
#[derive(Debug, Clone)]
pub struct Pattern {
    /// Literal string to match
    pub literal: Option<String>,
    /// Compiled regex pattern
    pub regex: Option<Regex>,
}

impl Pattern {
    /// Create a literal string pattern
    pub fn literal(s: impl Into<String>) -> Self {
        Self {
            literal: Some(s.into()),
            regex: None,
        }
    }

    /// Create a regex pattern
    pub fn regex(pattern: &str) -> Option<Self> {
        Regex::new(pattern).ok().map(|r| Self {
            literal: None,
            regex: Some(r),
        })
    }

    /// Check if the pattern matches a string
    pub fn matches(&self, s: &str) -> bool {
        if let Some(ref literal) = self.literal {
            s == literal
        } else if let Some(ref regex) = self.regex {
            regex.is_match(s)
        } else {
            false
        }
    }
}

/// A window rule that matches windows and applies settings
#[derive(Debug, Clone, Default)]
pub struct Rule {
    /// App ID pattern to match
    pub app_id: Option<Vec<String>>,
    /// Title pattern to match (can be regex)
    pub title: Option<Vec<String>>,
    /// Regex pattern for app_id
    pub app_id_regex: Option<Regex>,
    /// Regex pattern for title
    pub title_regex: Option<Regex>,

    /// Require the window to only support CSD
    pub require_csd_only: Option<bool>,
    /// Require the window to have no parent
    pub require_no_parent: Option<bool>,

    /// Decoration style for matching windows
    pub decoration: Option<WindowDecoration>,
    /// Pixels to swallow from the top of matching windows
    pub swallow_top: Option<i32>,
}

impl Rule {
    /// Check if this rule matches a window
    pub fn matches(
        &self,
        app_id: Option<&str>,
        title: Option<&str>,
        decoration_hint: u32,
        has_parent: bool,
    ) -> bool {
        // If both app_id and title are None/empty, match the rule for empty windows
        if self.app_id.is_none() && self.title.is_none() {
            // This is a rule for windows with no app_id or title
            let app_empty = app_id.is_none() || app_id.map(|s| s.is_empty()).unwrap_or(true);
            let title_empty = title.is_none() || title.map(|s| s.is_empty()).unwrap_or(true);
            return app_empty || title_empty;
        }

        // Check app_id match
        let app_id_matches = match (&self.app_id, &self.app_id_regex, app_id) {
            (Some(patterns), _, Some(id)) => patterns.iter().any(|p| p == id),
            (_, Some(regex), Some(id)) => regex.is_match(id),
            (None, None, _) => true, // No app_id requirement
            _ => false,
        };

        // Check title match
        let title_matches = match (&self.title, &self.title_regex, title) {
            (Some(patterns), _, Some(t)) => patterns.iter().any(|p| p == t),
            (_, Some(regex), Some(t)) => regex.is_match(t),
            (None, None, _) => true, // No title requirement
            _ => false,
        };

        let csd_only = decoration_hint == 0;
        let csd_matches = match self.require_csd_only {
            Some(true) => csd_only,
            Some(false) => !csd_only,
            None => true,
        };

        let parent_matches = match self.require_no_parent {
            Some(true) => !has_parent,
            Some(false) => has_parent,
            None => true,
        };

        app_id_matches && title_matches && csd_matches && parent_matches
    }
}

/// Apply matching rules to a window
pub fn apply_rules(
    rules: &[Rule],
    app_id: Option<&str>,
    title: Option<&str>,
    decoration_hint: u32,
    has_parent: bool,
) -> AppliedRules {
    let mut applied = AppliedRules::default();

    for rule in rules {
        if rule.matches(app_id, title, decoration_hint, has_parent) {
            if let Some(decoration) = rule.decoration {
                applied.decoration = Some(decoration);
            }
            if let Some(swallow_top) = rule.swallow_top {
                applied.swallow_top = Some(swallow_top);
            }
        }
    }

    applied
}

/// Result of applying rules to a window
#[derive(Debug, Clone, Default)]
pub struct AppliedRules {
    pub decoration: Option<WindowDecoration>,
    pub swallow_top: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_literal() {
        let p = Pattern::literal("foot");
        assert!(p.matches("foot"));
        assert!(!p.matches("foot2"));
        assert!(!p.matches("Foot"));
    }

    #[test]
    fn test_pattern_regex() {
        let p = Pattern::regex(".*[tT]ouchpad").unwrap();
        assert!(p.matches("My Touchpad"));
        assert!(p.matches("Xtouchpad"));
        assert!(!p.matches("Mouse"));
    }

    #[test]
    fn test_rule_matching() {
        let rule = Rule {
            app_id: Some(vec!["foot".to_string()]),
            ..Default::default()
        };

        assert!(rule.matches(Some("foot"), None, 3, false));
        assert!(rule.matches(Some("foot"), Some("Terminal"), 3, false));
        assert!(!rule.matches(Some("chromium"), None, 3, false));
    }

    #[test]
    fn test_apply_rules() {
        let rules = vec![Rule {
            app_id: Some(vec!["foot".to_string()]),
            ..Default::default()
        }];

        let applied = apply_rules(&rules, Some("foot"), None, 3, false);
        assert!(applied.decoration.is_none());
    }
}
