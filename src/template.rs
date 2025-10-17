use std::collections::HashMap;

/// A simple template engine for string substitution
#[derive(Debug, Clone)]
pub struct TemplateEngine {
    variables: HashMap<String, String>,
}

impl TemplateEngine {
    /// Create a new template engine
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// Set a variable for template substitution
    pub fn set<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.variables.insert(key.into(), value.into());
        self
    }

    /// Render a template string by substituting variables
    pub fn render(&self, template: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = template.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '{' && i + 1 < chars.len() {
                if chars[i + 1] == '{' {
                    // Double brace - skip substitution
                    result.push(chars[i]);
                    result.push(chars[i + 1]);
                    i += 2;
                } else {
                    // Single brace - look for variable substitution
                    let start = i;
                    i += 1;

                    // Find the closing brace
                    let mut var_name = String::new();
                    let mut found_closing = false;

                    while i < chars.len() {
                        if chars[i] == '}' {
                            found_closing = true;
                            break;
                        }
                        var_name.push(chars[i]);
                        i += 1;
                    }

                    if found_closing && self.variables.contains_key(&var_name) {
                        // Substitute the variable
                        result.push_str(self.variables.get(&var_name).unwrap());
                        i += 1; // Skip the closing brace
                    } else {
                        // No substitution - copy the original text
                        for j in start..=i {
                            if j < chars.len() {
                                result.push(chars[j]);
                            }
                        }
                        if found_closing {
                            i += 1;
                        }
                    }
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }

        result
    }

    /// Render template arguments by substituting variables in each argument
    pub fn render_args(&self, args: &[String]) -> Vec<String> {
        args.iter().map(|arg| self.render(arg)).collect()
    }

    /// Clear all variables
    pub fn clear(&mut self) {
        self.variables.clear();
    }

    /// Check if a variable is set
    pub fn has_variable(&self, key: &str) -> bool {
        self.variables.contains_key(key)
    }

    /// Get the value of a variable
    pub fn get_variable(&self, key: &str) -> Option<&str> {
        self.variables.get(key).map(std::string::String::as_str)
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_template_engine() {
        let engine = TemplateEngine::new();
        assert!(engine.variables.is_empty());
    }

    #[test]
    fn test_default_template_engine() {
        let engine = TemplateEngine::default();
        assert!(engine.variables.is_empty());
    }

    #[test]
    fn test_set_variable() {
        let mut engine = TemplateEngine::new();
        engine.set("name", "test");

        assert_eq!(engine.get_variable("name"), Some("test"));
        assert!(engine.has_variable("name"));
    }

    #[test]
    fn test_set_variable_chaining() {
        let mut engine = TemplateEngine::new();
        engine.set("name", "test").set("value", "123");

        assert_eq!(engine.get_variable("name"), Some("test"));
        assert_eq!(engine.get_variable("value"), Some("123"));
    }

    #[test]
    fn test_render_basic_substitution() {
        let mut engine = TemplateEngine::new();
        engine.set("name", "world");

        let result = engine.render("Hello {name}!");
        assert_eq!(result, "Hello world!");
    }

    #[test]
    fn test_render_multiple_variables() {
        let mut engine = TemplateEngine::new();
        engine
            .set("greeting", "Hello")
            .set("name", "world")
            .set("punctuation", "!");

        let result = engine.render("{greeting} {name}{punctuation}");
        assert_eq!(result, "Hello world!");
    }

    #[test]
    fn test_render_same_variable_multiple_times() {
        let mut engine = TemplateEngine::new();
        engine.set("name", "test");

        let result = engine.render("{name} and {name} again");
        assert_eq!(result, "test and test again");
    }

    #[test]
    fn test_render_no_variables() {
        let engine = TemplateEngine::new();
        let result = engine.render("No variables here");
        assert_eq!(result, "No variables here");
    }

    #[test]
    fn test_render_missing_variable() {
        let engine = TemplateEngine::new();
        let result = engine.render("Hello {missing}!");
        assert_eq!(result, "Hello {missing}!");
    }

    #[test]
    fn test_render_empty_template() {
        let engine = TemplateEngine::new();
        let result = engine.render("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_render_args() {
        let mut engine = TemplateEngine::new();
        engine
            .set("prompt", "Choose file")
            .set("header", "Available options");

        let args = vec![
            "--prompt".to_string(),
            "{prompt}".to_string(),
            "--header={header}".to_string(),
            "--static".to_string(),
        ];

        let result = engine.render_args(&args);
        assert_eq!(
            result,
            vec![
                "--prompt",
                "Choose file",
                "--header=Available options",
                "--static"
            ]
        );
    }

    #[test]
    fn test_render_args_empty() {
        let engine = TemplateEngine::new();
        let result = engine.render_args(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_clear_variables() {
        let mut engine = TemplateEngine::new();
        engine.set("name", "test");
        assert!(engine.has_variable("name"));

        engine.clear();
        assert!(!engine.has_variable("name"));
        assert!(engine.variables.is_empty());
    }

    #[test]
    fn test_has_variable() {
        let mut engine = TemplateEngine::new();
        assert!(!engine.has_variable("name"));

        engine.set("name", "test");
        assert!(engine.has_variable("name"));
        assert!(!engine.has_variable("other"));
    }

    #[test]
    fn test_get_variable_none() {
        let engine = TemplateEngine::new();
        assert_eq!(engine.get_variable("missing"), None);
    }

    #[test]
    fn test_variable_overwrite() {
        let mut engine = TemplateEngine::new();
        engine.set("name", "first");
        assert_eq!(engine.get_variable("name"), Some("first"));

        engine.set("name", "second");
        assert_eq!(engine.get_variable("name"), Some("second"));
    }

    #[test]
    fn test_render_with_braces_in_content() {
        let mut engine = TemplateEngine::new();
        engine.set("code", "if (x > 0) { return true; }");

        let result = engine.render("Code: {code}");
        assert_eq!(result, "Code: if (x > 0) { return true; }");
    }

    #[test]
    fn test_render_with_special_characters() {
        let mut engine = TemplateEngine::new();
        engine.set("special", "!@#$%^&*()");

        let result = engine.render("Special: {special}");
        assert_eq!(result, "Special: !@#$%^&*()");
    }

    #[test]
    fn test_render_with_unicode() {
        let mut engine = TemplateEngine::new();
        engine.set("unicode", "🦀 Rust 🚀");

        let result = engine.render("Message: {unicode}");
        assert_eq!(result, "Message: 🦀 Rust 🚀");
    }

    #[test]
    fn test_render_with_empty_variable() {
        let mut engine = TemplateEngine::new();
        engine.set("empty", "");

        let result = engine.render("Before{empty}After");
        assert_eq!(result, "BeforeAfter");
    }

    #[test]
    fn test_render_with_whitespace_variable() {
        let mut engine = TemplateEngine::new();
        engine.set("space", "   ");

        let result = engine.render("A{space}B");
        assert_eq!(result, "A   B");
    }

    #[test]
    fn test_large_template_performance() {
        let mut engine = TemplateEngine::new();
        engine.set("var", "replacement");

        // Create a large template with many substitutions
        let template = "{var} ".repeat(1000);
        let result = engine.render(&template);

        assert_eq!(result, "replacement ".repeat(1000));
    }

    #[test]
    fn test_many_variables() {
        let mut engine = TemplateEngine::new();

        // Set many variables
        for i in 0..100 {
            engine.set(format!("var{i}"), format!("value{i}"));
        }

        // Use some of them
        let result = engine.render("{var0} {var50} {var99}");
        assert_eq!(result, "value0 value50 value99");
    }

    #[test]
    fn test_nested_braces_no_substitution() {
        let mut engine = TemplateEngine::new();
        engine.set("inner", "test");

        // Nested braces should not be substituted
        let result = engine.render("{{inner}}");
        assert_eq!(result, "{{inner}}");
    }

    #[test]
    fn test_partial_variable_names() {
        let mut engine = TemplateEngine::new();
        engine.set("var", "value");
        engine.set("variable", "other");

        let result = engine.render("{var} {variable}");
        assert_eq!(result, "value other");
    }

    #[test]
    fn test_variable_with_numbers() {
        let mut engine = TemplateEngine::new();
        engine.set("var123", "numbered");

        let result = engine.render("{var123}");
        assert_eq!(result, "numbered");
    }

    #[test]
    fn test_variable_with_underscores() {
        let mut engine = TemplateEngine::new();
        engine.set("var_name", "underscore");

        let result = engine.render("{var_name}");
        assert_eq!(result, "underscore");
    }

    #[test]
    fn test_clone_template_engine() {
        let mut engine = TemplateEngine::new();
        engine.set("name", "test");

        let cloned = engine.clone();
        assert_eq!(cloned.get_variable("name"), Some("test"));

        // Verify they're independent
        engine.set("name", "changed");
        assert_eq!(cloned.get_variable("name"), Some("test"));
    }

    #[test]
    fn test_debug_format() {
        let mut engine = TemplateEngine::new();
        engine.set("test", "value");

        let debug_str = format!("{engine:?}");
        assert!(debug_str.contains("TemplateEngine"));
        assert!(debug_str.contains("variables"));
    }
}
