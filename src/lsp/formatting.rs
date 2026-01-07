use tower_lsp::lsp_types::FormattingOptions;

pub struct FormattingProvider {
    max_line_length: usize,
}

impl FormattingProvider {
    pub fn new() -> Self {
        Self {
            max_line_length: 100,
        }
    }

    pub fn format(&self, text: &str, options: &FormattingOptions) -> String {
        let indent_size = options.tab_size as usize;
        let use_tabs = !options.insert_spaces;

        let indent_char = if use_tabs {
            "\t".to_string()
        } else {
            " ".repeat(indent_size)
        };

        let lines: Vec<&str> = text.lines().collect();
        let mut result = Vec::new();
        let mut indent_level = 0;

        for line in lines {
            let trimmed = line.trim();

            // Skip empty lines (preserve them)
            if trimmed.is_empty() {
                result.push(String::new());
                continue;
            }

            // Handle comments
            if trimmed.starts_with("//") {
                result.push(format!("{}{}", indent_char.repeat(indent_level), trimmed));
                continue;
            }

            // Decrease indent for closing braces/brackets at start
            if trimmed.starts_with('}') || trimmed.starts_with(']') {
                indent_level = indent_level.saturating_sub(1);
            }

            // Special handling for lines that might need breaking
            if self.should_break_line(trimmed) {
                let broken = self.break_line(trimmed, indent_level, &indent_char);
                result.extend(broken);
            } else {
                // Format the line
                let formatted = self.format_line(trimmed);
                result.push(format!("{}{}", indent_char.repeat(indent_level), formatted));
            }

            // Increase indent for opening braces/brackets at end
            let opens = self.count_openers(trimmed);
            let closes = self.count_closers_at_end(trimmed);
            indent_level += opens;
            indent_level = indent_level.saturating_sub(closes);
        }

        // Clean up multiple consecutive empty lines
        self.clean_empty_lines(result.join("\n"))
    }

    fn format_line(&self, line: &str) -> String {
        // Format property lines: ensure consistent spacing around colon
        if let Some((key, value)) = self.parse_property(line) {
            return format!("{}: {}", key, value);
        }

        // Format component definition
        if line.contains(" -> ") {
            return self.format_component_def(line);
        }

        // Format store definition
        if line.contains(" | ") {
            let parts: Vec<&str> = line.splitn(2, " | ").collect();
            if parts.len() == 2 {
                return format!("{} | {}", parts[0].trim(), parts[1].trim());
            }
        }

        // Format API definition
        if line.contains(" :: ") {
            let parts: Vec<&str> = line.splitn(2, " :: ").collect();
            if parts.len() == 2 {
                return format!("{} :: {}", parts[0].trim(), parts[1].trim());
            }
        }

        line.to_string()
    }

    fn format_component_def(&self, line: &str) -> String {
        let parts: Vec<&str> = line.splitn(2, " -> ").collect();
        if parts.len() != 2 {
            return line.to_string();
        }

        let name_part = parts[0].trim();
        let body_part = parts[1].trim();

        // Format name and params
        let formatted_name = if name_part.contains('(') {
            let paren_start = name_part.find('(').unwrap();
            let name = &name_part[..paren_start];
            let params = &name_part[paren_start..];
            // Format params with consistent spacing
            let params_formatted = self.format_params(params);
            format!("{}{}", name, params_formatted)
        } else {
            name_part.to_string()
        };

        format!("{} -> {}", formatted_name, body_part)
    }

    fn format_params(&self, params: &str) -> String {
        // Remove outer parens, format inner, add back
        let inner = params.trim_start_matches('(').trim_end_matches(')');
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        format!("({})", parts.join(", "))
    }

    fn parse_property(&self, line: &str) -> Option<(String, String)> {
        // Skip if this is a definition line
        if line.contains(" -> ") || line.contains(" | ") || line.contains(" :: ") {
            return None;
        }

        // Look for key: value pattern
        let colon_pos = line.find(':')?;

        // Make sure this isn't inside a string
        let before_colon = &line[..colon_pos];
        if before_colon.matches('"').count() % 2 != 0 {
            return None;
        }

        let key = before_colon.trim();
        let value = line[colon_pos + 1..].trim();

        // Skip if key contains spaces (not a valid property)
        if key.contains(' ') {
            return None;
        }

        Some((key.to_string(), value.to_string()))
    }

    fn should_break_line(&self, line: &str) -> bool {
        // Check if line is too long
        if line.len() <= self.max_line_length {
            return false;
        }

        // Check if it's a component call with object props
        if line.contains("({") && line.contains("})") {
            return true;
        }

        // Check if it's a children array
        if line.starts_with("children:") && line.contains('[') {
            return true;
        }

        false
    }

    fn break_line(&self, line: &str, indent_level: usize, indent_char: &str) -> Vec<String> {
        let base_indent = indent_char.repeat(indent_level);
        let inner_indent = indent_char.repeat(indent_level + 1);

        // Handle component call with object props: Component({ key: value, key2: value2 })
        if let Some(result) = self.break_component_call(line, &base_indent, &inner_indent) {
            return result;
        }

        // Handle children array
        if let Some(result) = self.break_children_array(line, &base_indent, &inner_indent) {
            return result;
        }

        // Fallback: just return the line as-is
        vec![format!("{}{}", base_indent, line)]
    }

    fn break_component_call(
        &self,
        line: &str,
        base_indent: &str,
        inner_indent: &str,
    ) -> Option<Vec<String>> {
        // Match: ComponentName({ prop: value, prop2: value2 })
        let re = regex::Regex::new(r"^(\w+)\(\{(.+)\}\)(,?)$").ok()?;
        let caps = re.captures(line)?;

        let name = &caps[1];
        let props_str = &caps[2];
        let trailing = caps.get(3).map(|m| m.as_str()).unwrap_or("");

        let props = self.parse_props(props_str);
        if props.is_empty() {
            return None;
        }

        let mut result = Vec::new();
        result.push(format!("{}{}({{", base_indent, name));

        for (i, (key, value)) in props.iter().enumerate() {
            let comma = if i < props.len() - 1 { "" } else { "" };
            result.push(format!("{}{}: {}{}", inner_indent, key, value, comma));
        }

        result.push(format!("{}}}){}",base_indent, trailing));

        Some(result)
    }

    fn break_children_array(
        &self,
        line: &str,
        base_indent: &str,
        inner_indent: &str,
    ) -> Option<Vec<String>> {
        // Match: children: [Child1, Child2, Child3]
        let re = regex::Regex::new(r"^children\s*:\s*\[(.+)\]$").ok()?;
        let caps = re.captures(line)?;

        let children_str = &caps[1];
        let children = self.split_children(children_str);

        if children.len() <= 2 {
            return None;
        }

        let mut result = Vec::new();
        result.push(format!("{}children: [", base_indent));

        for (i, child) in children.iter().enumerate() {
            let comma = if i < children.len() - 1 { "," } else { "" };
            result.push(format!("{}{}{}", inner_indent, child.trim(), comma));
        }

        result.push(format!("{}]", base_indent));

        Some(result)
    }

    fn parse_props(&self, props_str: &str) -> Vec<(String, String)> {
        let mut props = Vec::new();
        let mut current = String::new();
        let mut depth = 0;
        let mut in_string = false;
        let mut string_char = ' ';

        for c in props_str.chars() {
            if !in_string && (c == '"' || c == '\'') {
                in_string = true;
                string_char = c;
            } else if in_string && c == string_char {
                in_string = false;
            }

            if !in_string {
                if c == '(' || c == '{' || c == '[' {
                    depth += 1;
                }
                if c == ')' || c == '}' || c == ']' {
                    depth -= 1;
                }
            }

            if depth == 0 && !in_string && c == ',' {
                if let Some((k, v)) = self.parse_single_prop(&current) {
                    props.push((k, v));
                }
                current.clear();
            } else {
                current.push(c);
            }
        }

        if !current.trim().is_empty() {
            if let Some((k, v)) = self.parse_single_prop(&current) {
                props.push((k, v));
            }
        }

        props
    }

    fn parse_single_prop(&self, s: &str) -> Option<(String, String)> {
        let colon_pos = s.find(':')?;
        let key = s[..colon_pos].trim().to_string();
        let value = s[colon_pos + 1..].trim().to_string();
        Some((key, value))
    }

    fn split_children(&self, children_str: &str) -> Vec<String> {
        let mut children = Vec::new();
        let mut current = String::new();
        let mut depth = 0;

        for c in children_str.chars() {
            if c == '(' || c == '{' || c == '[' {
                depth += 1;
            }
            if c == ')' || c == '}' || c == ']' {
                depth -= 1;
            }

            if depth == 0 && c == ',' {
                children.push(current.trim().to_string());
                current.clear();
            } else {
                current.push(c);
            }
        }

        if !current.trim().is_empty() {
            children.push(current.trim().to_string());
        }

        children
    }

    fn count_openers(&self, line: &str) -> usize {
        let mut count = 0;
        let mut in_string = false;
        let mut string_char = ' ';

        for c in line.chars() {
            if !in_string && (c == '"' || c == '\'') {
                in_string = true;
                string_char = c;
            } else if in_string && c == string_char {
                in_string = false;
            }

            if !in_string && (c == '{' || c == '[') {
                count += 1;
            }
        }

        count
    }

    fn count_closers_at_end(&self, line: &str) -> usize {
        let mut count = 0;
        let mut in_string = false;
        let mut string_char = ' ';

        // Check closers that are at the end and not balanced on this line
        let mut opens = 0;
        let mut closes = 0;

        for c in line.chars() {
            if !in_string && (c == '"' || c == '\'') {
                in_string = true;
                string_char = c;
            } else if in_string && c == string_char {
                in_string = false;
            }

            if !in_string {
                if c == '{' || c == '[' {
                    opens += 1;
                }
                if c == '}' || c == ']' {
                    closes += 1;
                }
            }
        }

        // Only count closers that aren't matched by openers on this line
        if closes > opens {
            count = closes - opens;
        }

        count
    }

    fn clean_empty_lines(&self, text: String) -> String {
        // Replace 3+ consecutive newlines with 2
        let re = regex::Regex::new(r"\n{3,}").unwrap();
        re.replace_all(&text, "\n\n").to_string()
    }
}
