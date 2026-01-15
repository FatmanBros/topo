use once_cell::sync::Lazy;
use regex::Regex;
use tower_lsp::lsp_types::*;

use crate::workspace::WorkspaceManager;

// Pre-compiled regex patterns for better performance
static IMPORT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"import\s*\{\s*([^}]+)\s*\}"#).expect("Invalid import regex")
});
static COMP_DEF_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([A-Z][a-zA-Z0-9]*)\s*(\([^)]*\))?\s*->").expect("Invalid component def regex")
});
static STORE_DEF_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([A-Z][a-zA-Z0-9]*)\s*\|").expect("Invalid store def regex")
});
static API_DEF_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([A-Z][a-zA-Z0-9]*)\s*::").expect("Invalid api def regex")
});
static PASCAL_CASE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b([A-Z][a-zA-Z0-9]*)\b").expect("Invalid pascal case regex")
});
static LEADING_PASCAL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([A-Z][a-zA-Z0-9]*)").expect("Invalid leading pascal regex")
});

pub struct DiagnosticsProvider {}

impl DiagnosticsProvider {
    pub fn new() -> Self {
        Self {}
    }

    pub fn diagnose(&self, text: &str, workspace: &WorkspaceManager) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let lines: Vec<&str> = text.lines().collect();

        // Collect imports
        let imports = self.collect_imports(text);

        // Track defined components in this file
        let local_components = self.collect_local_definitions(text);

        for (line_num, line) in lines.iter().enumerate() {
            let line_num = line_num as u32;
            let trimmed = line.trim();

            // Check for undefined components
            self.check_undefined_components(
                trimmed,
                line_num,
                &imports,
                &local_components,
                workspace,
                &mut diagnostics,
            );

            // Check for syntax issues
            self.check_syntax(trimmed, line_num, &mut diagnostics);

            // Check for style best practices
            self.check_style_issues(trimmed, line_num, &mut diagnostics);

            // Check for common mistakes
            self.check_common_mistakes(trimmed, line_num, &mut diagnostics);
        }

        // Check for unused imports
        self.check_unused_imports(text, &imports, &mut diagnostics);

        diagnostics
    }

    fn collect_imports(&self, text: &str) -> Vec<(String, u32)> {
        let mut imports = Vec::new();

        for (line_num, line) in text.lines().enumerate() {
            if let Some(caps) = IMPORT_RE.captures(line) {
                for name in caps[1].split(',') {
                    imports.push((name.trim().to_string(), line_num as u32));
                }
            }
        }

        imports
    }

    fn collect_local_definitions(&self, text: &str) -> Vec<String> {
        let mut defs = Vec::new();

        for line in text.lines() {
            let trimmed = line.trim();

            if let Some(caps) = COMP_DEF_RE.captures(trimmed) {
                defs.push(caps[1].to_string());
            } else if let Some(caps) = STORE_DEF_RE.captures(trimmed) {
                defs.push(caps[1].to_string());
            } else if let Some(caps) = API_DEF_RE.captures(trimmed) {
                defs.push(caps[1].to_string());
            }
        }

        defs
    }

    fn check_undefined_components(
        &self,
        line: &str,
        line_num: u32,
        imports: &[(String, u32)],
        local_defs: &[String],
        workspace: &WorkspaceManager,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Skip if this is a definition line
        if line.contains("->") && !line.contains(": ") {
            // This might be a component definition, skip the name part
            if let Some(caps) = LEADING_PASCAL_RE.captures(line) {
                // Skip checking the component being defined
                let defining = caps[1].to_string();
                for caps in PASCAL_CASE_RE.captures_iter(line) {
                    let name = &caps[1];
                    if name == defining {
                        continue;
                    }
                    self.check_single_component(
                        name,
                        line,
                        line_num,
                        imports,
                        local_defs,
                        workspace,
                        diagnostics,
                    );
                }
                return;
            }
        }

        // Check store/api definitions
        if line.contains(" | ") || line.contains(" :: ") {
            return;
        }

        // Check for State, Actions, Reducers keywords
        if ["State", "Actions", "Reducers"].iter().any(|k| line.starts_with(*k)) {
            return;
        }

        for caps in PASCAL_CASE_RE.captures_iter(line) {
            let name = &caps[1];
            self.check_single_component(
                name,
                line,
                line_num,
                imports,
                local_defs,
                workspace,
                diagnostics,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_single_component(
        &self,
        name: &str,
        line: &str,
        line_num: u32,
        imports: &[(String, u32)],
        local_defs: &[String],
        workspace: &WorkspaceManager,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Skip built-in keywords and types
        let builtins = [
            "State", "Actions", "Reducers", "Router", "GET", "POST", "PUT", "DELETE", "PATCH",
        ];
        if builtins.contains(&name) {
            return;
        }

        // Check if imported
        let is_imported = imports.iter().any(|(n, _)| n == name);
        if is_imported {
            return;
        }

        // Check if locally defined
        if local_defs.contains(&name.to_string()) {
            return;
        }

        // Check if it exists in workspace but not imported
        if workspace.get_component(name).is_some() {
            // Component exists but not imported
            let col = line.find(name).unwrap_or(0) as u32;
            diagnostics.push(Diagnostic {
                range: Range::new(
                    Position::new(line_num, col),
                    Position::new(line_num, col + name.len() as u32),
                ),
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String("missing-import".to_string())),
                source: Some("topo".to_string()),
                message: format!("Unknown component: {} (available in workspace, add import)", name),
                related_information: None,
                tags: None,
                code_description: None,
                data: None,
            });
        }
    }

    fn check_syntax(&self, line: &str, line_num: u32, diagnostics: &mut Vec<Diagnostic>) {
        // Only check for unclosed strings on a single line
        let mut in_string = false;
        let mut string_char = ' ';
        let chars: Vec<char> = line.chars().collect();

        for (i, &c) in chars.iter().enumerate() {
            if !in_string && (c == '"' || c == '\'') {
                in_string = true;
                string_char = c;
            } else if in_string && c == string_char {
                // Check if escaped
                let escaped = i > 0 && chars[i - 1] == '\\';
                if !escaped {
                    in_string = false;
                }
            }
        }

        // Check for unclosed string (only if line doesn't end with the opening quote on purpose)
        if in_string && !line.trim().ends_with("\"") && !line.trim().ends_with("'") {
            diagnostics.push(Diagnostic {
                range: Range::new(
                    Position::new(line_num, 0),
                    Position::new(line_num, line.len() as u32),
                ),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("topo".to_string()),
                message: "Unclosed string".to_string(),
                ..Default::default()
            });
        }

        // Note: Multi-line bracket balance is checked at document level, not per-line
    }

    fn check_style_issues(&self, line: &str, line_num: u32, diagnostics: &mut Vec<Diagnostic>) {
        // Check for inline styles (discouraged)
        if line.contains("style=") {
            diagnostics.push(Diagnostic {
                range: Range::new(
                    Position::new(line_num, 0),
                    Position::new(line_num, line.len() as u32),
                ),
                severity: Some(DiagnosticSeverity::HINT),
                source: Some("topo".to_string()),
                message: "Consider using Tailwind classes in the 'style' property instead".to_string(),
                ..Default::default()
            });
        }

        // Check for very long style strings (maybe should be extracted)
        if let Some(style_start) = line.find("style:") {
            let after = &line[style_start..];
            if let Some(quote_start) = after.find('"') {
                let after_quote = &after[quote_start + 1..];
                if let Some(quote_end) = after_quote.find('"') {
                    let style_content = &after_quote[..quote_end];
                    let class_count = style_content.split_whitespace().count();
                    if class_count > 15 {
                        diagnostics.push(Diagnostic {
                            range: Range::new(
                                Position::new(line_num, style_start as u32),
                                Position::new(line_num, (style_start + quote_start + quote_end + 2) as u32),
                            ),
                            severity: Some(DiagnosticSeverity::HINT),
                            source: Some("topo".to_string()),
                            message: format!(
                                "Long style string ({} classes). Consider extracting to a reusable component.",
                                class_count
                            ),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }

    #[allow(clippy::ptr_arg)]
    fn check_common_mistakes(&self, _line: &str, _line_num: u32, _diagnostics: &mut Vec<Diagnostic>) {
        // Disabled for now to avoid false positives
        // TODO: Implement more accurate checks using the actual parser
    }

    fn check_unused_imports(
        &self,
        text: &str,
        imports: &[(String, u32)],
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (name, line_num) in imports {
            // Count occurrences (excluding the import line itself)
            let pattern = format!(r"\b{}\b", regex::escape(name));
            let Ok(re) = Regex::new(&pattern) else {
                continue; // Skip if pattern is invalid (shouldn't happen with escaped input)
            };
            let count = re.find_iter(text).count();

            // If only found once (in the import), it's unused
            if count <= 1 {
                diagnostics.push(Diagnostic {
                    range: Range::new(
                        Position::new(*line_num, 0),
                        Position::new(*line_num, 100), // Approximate
                    ),
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String("unused-import".to_string())),
                    source: Some("topo".to_string()),
                    message: format!("'{}' is imported but never used", name),
                    tags: Some(vec![DiagnosticTag::UNNECESSARY]),
                    ..Default::default()
                });
            }
        }
    }
}
