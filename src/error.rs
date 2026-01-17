//! Error formatting utilities for better error messages

use crate::parser::ParseError;
use std::path::Path;

/// Format a parse error with source context for better debugging
///
/// Produces output like:
/// ```text
/// error: Unexpected token: expected expression, found =>
///   --> pages/home/index.tp:149:54
///     |
/// 149 | items.for(item, index, => { ... })
///     |                        ^^ expected expression
/// ```
pub fn format_parse_error(error: &ParseError, source: &str, file_path: Option<&Path>) -> String {
    let mut output = String::new();

    match error {
        ParseError::UnexpectedToken { expected, found, line, column } => {
            // Error header
            output.push_str(&format!("\x1b[1;31merror\x1b[0m: Unexpected token: expected \x1b[1;33m{}\x1b[0m, found \x1b[1;33m{}\x1b[0m\n", expected, found));

            // File location
            if let Some(path) = file_path {
                output.push_str(&format!("  \x1b[1;34m-->\x1b[0m {}:{}:{}\n", path.display(), line, column));
            } else {
                output.push_str(&format!("  \x1b[1;34m-->\x1b[0m line {}:{}\n", line, column));
            }

            // Source context
            output.push_str(&format_source_context(source, *line, *column, found.len()));

            // Add hint based on error pattern
            if let Some(hint) = generate_hint(source, *line, expected, found) {
                output.push_str(&hint);
            }
        }

        ParseError::InvalidDefinitionOperator { line, column } => {
            output.push_str("\x1b[1;31merror\x1b[0m: Invalid definition operator\n");
            output.push_str("       Expected one of: \x1b[1;33m->\x1b[0m (component), \x1b[1;33m|\x1b[0m (store), \x1b[1;33m::\x1b[0m (api)\n");

            if let Some(path) = file_path {
                output.push_str(&format!("  \x1b[1;34m-->\x1b[0m {}:{}:{}\n", path.display(), line, column));
            } else {
                output.push_str(&format!("  \x1b[1;34m-->\x1b[0m line {}:{}\n", line, column));
            }

            output.push_str(&format_source_context(source, *line, *column, 1));
        }

        ParseError::MaxRecursionDepthExceeded { line, column } => {
            output.push_str("\x1b[1;31merror\x1b[0m: Maximum recursion depth exceeded\n");
            output.push_str("       The nesting is too deep (limit: 64 levels)\n");

            if let Some(path) = file_path {
                output.push_str(&format!("  \x1b[1;34m-->\x1b[0m {}:{}:{}\n", path.display(), line, column));
            } else {
                output.push_str(&format!("  \x1b[1;34m-->\x1b[0m line {}:{}\n", line, column));
            }

            output.push_str(&format_source_context(source, *line, *column, 1));
        }

        ParseError::UnexpectedEof => {
            output.push_str("\x1b[1;31merror\x1b[0m: Unexpected end of file\n");
            if let Some(path) = file_path {
                output.push_str(&format!("  \x1b[1;34m-->\x1b[0m {}\n", path.display()));
            }
        }
    }

    output
}

/// Generate a hint message based on error patterns
fn generate_hint(source: &str, line: usize, expected: &str, found: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let line_idx = if line > 0 { line - 1 } else { 0 };
    let source_line = lines.get(line_idx)?;

    // Pattern: Arrow function () => or (param) => in event handler
    if (found == ")" || found == "=>") && source_line.contains("() =>") {
        // Extract the property name (e.g., onClick)
        if let Some(colon_pos) = source_line.find(':') {
            let prop_name = source_line[..colon_pos].trim();
            // Extract the action call after =>
            if let Some(arrow_pos) = source_line.find("=>") {
                let action = source_line[arrow_pos + 2..].trim();
                return Some(format!(
                    "\n  \x1b[1;36mhint\x1b[0m: Arrow functions are not supported in topo\n\
                       \x1b[1;32mhelp\x1b[0m: Try this instead:\n\
                       \n\
                           {}: {}\n",
                    prop_name, action
                ));
            }
        }
        return Some(
            "\n  \x1b[1;36mhint\x1b[0m: Arrow functions `() =>` are not supported in topo\n\
               \x1b[1;32mhelp\x1b[0m: Use direct action call instead:\n\
               \n\
                   onClick: SomeAction(param)\n".to_string()
        );
    }

    // Pattern: Arrow function with parameters (item) => or item =>
    if found == "=>" && expected == "expression" {
        return Some(
            "\n  \x1b[1;36mhint\x1b[0m: Arrow function syntax is not supported here\n\
               \x1b[1;32mhelp\x1b[0m: For iterations, use `.for()` method:\n\
               \n\
                   items.for(item => { ... })\n".to_string()
        );
    }

    // Pattern: Using => instead of -> for component definition
    if found == "=>" && (expected == "->" || expected == "|" || expected == "::") {
        return Some(
            "\n  \x1b[1;36mhint\x1b[0m: Use `->` for component definitions, not `=>`\n\
               \x1b[1;32mhelp\x1b[0m: Try this instead:\n\
               \n\
                   MyComponent -> { ... }\n".to_string()
        );
    }

    // Pattern: Computed property key [expr]
    if found == "[" || (expected == ":" && source_line.contains('[')) {
        return Some(
            "\n  \x1b[1;36mhint\x1b[0m: Computed property keys `[key]` are not supported in topo\n\
               \x1b[1;32mhelp\x1b[0m: Use a different approach or restructure your data\n".to_string()
        );
    }

    // Pattern: Spread operator ...
    if source_line.contains("...") && (expected == ":" || expected == "expression") {
        return Some(
            "\n  \x1b[1;36mhint\x1b[0m: Spread operator `...` usage may not be supported here\n\
               \x1b[1;32mhelp\x1b[0m: Check if spread is valid in this context\n".to_string()
        );
    }

    None
}

/// Format source context with line numbers and error marker
fn format_source_context(source: &str, line: usize, column: usize, marker_len: usize) -> String {
    let mut output = String::new();
    let lines: Vec<&str> = source.lines().collect();

    // Convert to 0-indexed
    let line_idx = if line > 0 { line - 1 } else { 0 };

    // Calculate line number width for padding
    let line_width = format!("{}", line).len().max(3);

    // Empty line with pipe
    output.push_str(&format!("{:width$} \x1b[1;34m|\x1b[0m\n", "", width = line_width));

    // Show the problematic line
    if line_idx < lines.len() {
        let source_line = lines[line_idx];
        output.push_str(&format!("\x1b[1;34m{:>width$}\x1b[0m \x1b[1;34m|\x1b[0m {}\n",
            line, source_line, width = line_width));

        // Error marker line
        let col_idx = if column > 0 { column - 1 } else { 0 };
        let marker_len = marker_len.max(1);

        // Calculate spaces before the marker, accounting for tab expansion
        let spaces: String = source_line.chars()
            .take(col_idx)
            .map(|c| if c == '\t' { '\t' } else { ' ' })
            .collect();

        let markers = "^".repeat(marker_len);
        output.push_str(&format!("{:width$} \x1b[1;34m|\x1b[0m {}\x1b[1;31m{}\x1b[0m\n",
            "", spaces, markers, width = line_width));
    }

    output
}

/// Format a lexer error with context
pub fn format_lexer_error(error: &str, source: &str, file_path: Option<&Path>) -> String {
    let mut output = String::new();

    output.push_str(&format!("\x1b[1;31merror\x1b[0m: {}\n", error));

    if let Some(path) = file_path {
        output.push_str(&format!("  \x1b[1;34m-->\x1b[0m {}\n", path.display()));
    }

    // Try to extract line/column from error message
    if let Some((line, col)) = extract_position_from_error(error) {
        output.push_str(&format_source_context(source, line, col, 1));
    }

    output
}

/// Extract line and column from error message if present
fn extract_position_from_error(error: &str) -> Option<(usize, usize)> {
    // Pattern: "at line X, column Y" or "line X:Y"
    if let Some(line_start) = error.find("line ") {
        let after_line = &error[line_start + 5..];
        let line_end = after_line.find(|c: char| !c.is_ascii_digit()).unwrap_or(after_line.len());
        if let Ok(line) = after_line[..line_end].parse::<usize>() {
            if let Some(col_start) = after_line.find("column ") {
                let after_col = &after_line[col_start + 7..];
                let col_end = after_col.find(|c: char| !c.is_ascii_digit()).unwrap_or(after_col.len());
                if let Ok(col) = after_col[..col_end].parse::<usize>() {
                    return Some((line, col));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_unexpected_token() {
        let error = ParseError::UnexpectedToken {
            expected: "expression".to_string(),
            found: "=>".to_string(),
            line: 5,
            column: 20,
        };

        let source = r#"Component -> {
    items.for(item => {
        Text(item)
    })
    items.for(item, => { Text(item) })
}"#;

        let result = format_parse_error(&error, source, Some(Path::new("test.tp")));

        // Check that output contains key elements
        assert!(result.contains("error"));
        assert!(result.contains("expected"));
        assert!(result.contains("expression"));
        assert!(result.contains("=>"));
        assert!(result.contains("test.tp"));
        assert!(result.contains("5:20"));
        assert!(result.contains("^"));
    }

    #[test]
    fn test_format_invalid_operator() {
        let error = ParseError::InvalidDefinitionOperator {
            line: 1,
            column: 10,
        };

        let source = "Component => { }";
        let result = format_parse_error(&error, source, None);

        assert!(result.contains("Invalid definition operator"));
        assert!(result.contains("->"));
        assert!(result.contains("|"));
    }
}
