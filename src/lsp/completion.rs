use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::json;
use tower_lsp::lsp_types::*;

use crate::tailwind::TAILWIND_CLASSES;
use crate::workspace::{ComponentInfo, WorkspaceManager};

static IMPORT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"import\s*\{\s*([^}]+)\s*\}"#).expect("Invalid import regex")
});

pub struct CompletionProvider {}

impl CompletionProvider {
    pub fn new() -> Self {
        Self {}
    }

    pub fn provide(
        &self,
        text: &str,
        position: Position,
        workspace: &WorkspaceManager,
    ) -> Vec<CompletionItem> {
        let lines: Vec<&str> = text.lines().collect();
        let line = match lines.get(position.line as usize) {
            Some(l) => *l,
            None => return vec![],
        };

        let prefix = &line[..position.character as usize];
        let trimmed = prefix.trim();

        // Context detection
        if self.is_in_string(prefix) {
            // Check if in style property
            if self.is_in_style_value(prefix) {
                return self.tailwind_completions(prefix);
            }
            // Check if in import path
            if trimmed.contains("from") && (prefix.contains('"') || prefix.contains('\'')) {
                return self.import_path_completions(text, workspace);
            }
            return vec![];
        }

        // Store access: $
        if trimmed.ends_with('$') || prefix.ends_with('$') {
            return self.store_access_completions(workspace);
        }

        // Store dispatch: StoreName.
        if let Some(store_name) = self.get_store_before_dot(prefix) {
            return self.store_action_completions(&store_name, workspace);
        }

        // Annotation completions: @
        if trimmed.ends_with('@') || prefix.ends_with('@') {
            return self.annotation_completions();
        }

        // Import completions
        if trimmed.starts_with("import") && !trimmed.contains("from") {
            return self.import_name_completions(workspace);
        }

        // Property key completions (inside component body)
        if self.is_in_component_body(text, position) {
            if !trimmed.contains(':') {
                return self.property_completions();
            }
            // Property value completions
            if let Some(prop_name) = self.get_property_name(trimmed) {
                return self.property_value_completions(&prop_name);
            }
        }

        // Component completions (for children, general usage)
        if self.should_suggest_components(prefix, text, position) {
            return self.component_completions(text, workspace);
        }

        // Default: show all components
        self.component_completions(text, workspace)
    }

    pub fn hover(
        &self,
        text: &str,
        position: Position,
        workspace: &WorkspaceManager,
    ) -> Option<Hover> {
        let lines: Vec<&str> = text.lines().collect();
        let line = lines.get(position.line as usize)?;

        let word = self.get_word_at_position(line, position.character as usize)?;

        // Check if it's a component
        if let Some(comp) = workspace.get_component(&word) {
            let signature = self.get_component_signature(&comp);
            let mut content = format!("```topo\n{}\n```", signature);

            if !comp.params.is_empty() {
                content.push_str("\n\n**Parameters:**\n");
                for p in &comp.params {
                    content.push_str(&format!(
                        "- `{}`: {}\n",
                        p.name,
                        p.param_type.as_deref().unwrap_or("any")
                    ));
                }
            }

            if comp.is_store {
                content.push_str("\n\n*Store*");
            } else if comp.is_alias {
                content.push_str("\n\n*Component Alias*");
            }

            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: content,
                }),
                range: None,
            });
        }

        // Check for built-in properties
        if let Some(prop_doc) = self.get_property_documentation(&word) {
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: prop_doc,
                }),
                range: None,
            });
        }

        None
    }

    fn component_completions(&self, text: &str, workspace: &WorkspaceManager) -> Vec<CompletionItem> {
        let mut items = Vec::new();
        let current_imports = self.get_current_imports(text);

        for comp in workspace.get_all_components() {
            let is_imported = current_imports.contains(&comp.name);

            // With params: show signature
            let signature = self.get_component_signature(&comp);
            let detail = if comp.is_store {
                "Store".to_string()
            } else if comp.is_alias {
                "Component Alias".to_string()
            } else {
                "Component".to_string()
            };

            let mut item = CompletionItem {
                label: comp.name.clone(),
                kind: Some(if comp.is_store {
                    CompletionItemKind::CLASS
                } else {
                    CompletionItemKind::FUNCTION
                }),
                detail: Some(detail),
                documentation: Some(Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```topo\n{}\n```", signature),
                })),
                ..Default::default()
            };

            // If has params, provide snippet with placeholders
            if !comp.params.is_empty() {
                let placeholders: Vec<String> = comp
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, p)| format!("${{{}:{}}}", i + 1, p.name))
                    .collect();

                // Snippet for positional args
                item.insert_text = Some(format!("{}({})", comp.name, placeholders.join(", ")));
                item.insert_text_format = Some(InsertTextFormat::SNIPPET);

                // Also suggest object style
                let obj_placeholders: Vec<String> = comp
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, p)| format!("{}: ${{{}:{}}}", p.name, i + 1, p.name))
                    .collect();

                let mut obj_item = item.clone();
                obj_item.label = format!("{}({{...}})", comp.name);
                obj_item.insert_text = Some(format!(
                    "{}({{\n    {}\n}})",
                    comp.name,
                    obj_placeholders.join(",\n    ")
                ));
                obj_item.detail = Some("Component (object style)".to_string());
                obj_item.sort_text = Some(format!("1{}", comp.name));

                item.sort_text = Some(format!("0{}", comp.name));
                items.push(obj_item);
            }

            // Add auto-import info if not imported
            if !is_imported {
                item.additional_text_edits = Some(vec![]);
                item.data = Some(json!({
                    "import": true,
                    "component": comp.name,
                    "file": comp.file_path.to_string_lossy()
                }));
                item.label_details = Some(CompletionItemLabelDetails {
                    detail: None,
                    description: Some("(auto import)".to_string()),
                });
            }

            items.push(item);
        }

        items
    }

    fn store_access_completions(&self, workspace: &WorkspaceManager) -> Vec<CompletionItem> {
        workspace
            .get_all_components()
            .into_iter()
            .filter(|c| c.is_store)
            .map(|c| CompletionItem {
                label: format!("${}", c.name),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some("Store state".to_string()),
                insert_text: Some(format!("${}.${{{}}}", c.name, "1:field")),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            })
            .collect()
    }

    fn store_action_completions(
        &self,
        _store_name: &str,
        _workspace: &WorkspaceManager,
    ) -> Vec<CompletionItem> {
        // For now, return common action patterns
        vec![
            CompletionItem {
                label: "Set${1:Field}".to_string(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some("Action".to_string()),
                insert_text: Some("Set${1:Field}".to_string()),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            },
            CompletionItem {
                label: "Submit".to_string(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some("Action".to_string()),
                ..Default::default()
            },
            CompletionItem {
                label: "Reset".to_string(),
                kind: Some(CompletionItemKind::METHOD),
                detail: Some("Action".to_string()),
                ..Default::default()
            },
        ]
    }

    fn annotation_completions(&self) -> Vec<CompletionItem> {
        vec![
            ("required", "Mark field as required"),
            ("email", "Validate as email address"),
            ("minLength(n)", "Minimum string length"),
            ("maxLength(n)", "Maximum string length"),
            ("min(n)", "Minimum numeric value"),
            ("max(n)", "Maximum numeric value"),
            ("pattern(regex)", "Match regex pattern"),
            ("label(text)", "Display label for field"),
            ("range(min, max)", "Numeric range validation"),
            ("url", "Validate as URL"),
            ("alphanumeric", "Only letters and numbers"),
        ]
        .into_iter()
        .map(|(name, desc)| CompletionItem {
            label: format!("@{}", name),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(desc.to_string()),
            insert_text: Some(name.to_string()),
            ..Default::default()
        })
        .collect()
    }

    fn property_completions(&self) -> Vec<CompletionItem> {
        vec![
            ("type", "Element type (text, button, input, link, etc.)", "type: ${1|text,button,input,link,select,textarea,container,form|}"),
            ("content", "Text content", "content: \"$1\""),
            ("value", "Input value binding", "value: $1"),
            ("style", "Tailwind CSS classes", "style: \"$1\""),
            ("children", "Child components", "children: [$1]"),
            ("align", "Layout alignment", "align: ${1|vertical,horizontal|}"),
            ("click", "Click handler", "click: $1"),
            ("onInput", "Input handler", "onInput: $1"),
            ("onChange", "Change handler", "onChange: $1"),
            ("href", "Link URL", "href: \"$1\""),
            ("placeholder", "Input placeholder", "placeholder: \"$1\""),
            ("inputType", "Input type", "inputType: \"${1|text,email,password,number|}\""),
            ("dataBind", "Two-way data binding", "dataBind: $1"),
            ("dataError", "Error message binding", "dataError: $1"),
            ("dataField", "Field name for forms", "dataField: \"$1\""),
            ("options", "Select options", "options: [$1]"),
            ("rows", "Textarea rows", "rows: ${1:3}"),
        ]
        .into_iter()
        .map(|(name, desc, snippet)| CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::PROPERTY),
            detail: Some(desc.to_string()),
            insert_text: Some(snippet.to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        })
        .collect()
    }

    fn property_value_completions(&self, prop_name: &str) -> Vec<CompletionItem> {
        match prop_name {
            "type" => vec!["text", "button", "input", "link", "select", "textarea", "container", "form", "submit"]
                .into_iter()
                .map(|v| CompletionItem {
                    label: v.to_string(),
                    kind: Some(CompletionItemKind::ENUM_MEMBER),
                    ..Default::default()
                })
                .collect(),
            "align" => vec!["vertical", "horizontal"]
                .into_iter()
                .map(|v| CompletionItem {
                    label: v.to_string(),
                    kind: Some(CompletionItemKind::ENUM_MEMBER),
                    ..Default::default()
                })
                .collect(),
            "inputType" => vec!["text", "email", "password", "number", "tel", "url", "search", "date"]
                .into_iter()
                .map(|v| CompletionItem {
                    label: v.to_string(),
                    kind: Some(CompletionItemKind::ENUM_MEMBER),
                    ..Default::default()
                })
                .collect(),
            _ => vec![],
        }
    }

    fn tailwind_completions(&self, prefix: &str) -> Vec<CompletionItem> {
        // Get the last partial class being typed
        let last_space = prefix.rfind(' ').unwrap_or(0);
        let partial = &prefix[last_space..].trim();

        TAILWIND_CLASSES
            .iter()
            .filter(|(class, _)| partial.is_empty() || class.starts_with(partial))
            .take(50)
            .map(|(class, desc)| CompletionItem {
                label: class.to_string(),
                kind: Some(CompletionItemKind::VALUE),
                detail: Some(desc.to_string()),
                ..Default::default()
            })
            .collect()
    }

    fn import_name_completions(&self, workspace: &WorkspaceManager) -> Vec<CompletionItem> {
        workspace
            .get_all_components()
            .into_iter()
            .map(|c| CompletionItem {
                label: c.name.clone(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(c.file_path.to_string_lossy().to_string()),
                ..Default::default()
            })
            .collect()
    }

    fn import_path_completions(
        &self,
        _text: &str,
        workspace: &WorkspaceManager,
    ) -> Vec<CompletionItem> {
        // Return available .tp files
        workspace
            .get_all_components()
            .into_iter()
            .map(|c| CompletionItem {
                label: c.file_path.to_string_lossy().to_string(),
                kind: Some(CompletionItemKind::FILE),
                ..Default::default()
            })
            .collect()
    }

    fn get_component_signature(&self, comp: &ComponentInfo) -> String {
        if comp.params.is_empty() {
            if comp.is_store {
                format!("{} | {{ ... }}", comp.name)
            } else {
                format!("{} -> {{ ... }}", comp.name)
            }
        } else {
            let params: Vec<&str> = comp.params.iter().map(|p| p.name.as_str()).collect();
            format!("{}({}) -> {{ ... }}", comp.name, params.join(", "))
        }
    }

    fn is_in_string(&self, prefix: &str) -> bool {
        let mut in_string = false;
        let mut string_char = ' ';

        for (i, c) in prefix.chars().enumerate() {
            if !in_string && (c == '"' || c == '\'') {
                in_string = true;
                string_char = c;
            } else if in_string && c == string_char {
                // Check for escape
                let prev_backslashes = prefix[..i]
                    .chars()
                    .rev()
                    .take_while(|&c| c == '\\')
                    .count();
                if prev_backslashes % 2 == 0 {
                    in_string = false;
                }
            }
        }

        in_string
    }

    fn is_in_style_value(&self, prefix: &str) -> bool {
        // Check if we're in a style: "..." context
        let style_patterns = ["style:", "style :", "class:", "class :"];
        for pattern in style_patterns {
            if let Some(pos) = prefix.rfind(pattern) {
                let after = &prefix[pos + pattern.len()..];
                // Check if we opened a string after style:
                if after.contains('"') || after.contains('\'') {
                    return true;
                }
            }
        }
        false
    }

    fn is_in_component_body(&self, text: &str, position: Position) -> bool {
        let lines: Vec<&str> = text.lines().collect();
        let mut brace_depth = 0;

        for (i, line) in lines.iter().enumerate() {
            if i > position.line as usize {
                break;
            }

            let end = if i == position.line as usize {
                position.character as usize
            } else {
                line.len()
            };

            for c in line[..end.min(line.len())].chars() {
                if c == '{' {
                    brace_depth += 1;
                } else if c == '}' {
                    brace_depth -= 1;
                }
            }
        }

        brace_depth > 0
    }

    fn get_property_name(&self, trimmed: &str) -> Option<String> {
        if let Some(colon_pos) = trimmed.rfind(':') {
            let before = trimmed[..colon_pos].trim();
            let words: Vec<&str> = before.split_whitespace().collect();
            return words.last().map(|s| s.to_string());
        }
        None
    }

    fn get_store_before_dot(&self, prefix: &str) -> Option<String> {
        // Check for pattern: StoreName.
        let trimmed = prefix.trim();
        if let Some(before_dot) = trimmed.strip_suffix('.') {
            let words: Vec<&str> = before_dot.split_whitespace().collect();
            if let Some(last) = words.last() {
                if last.chars().next().is_some_and(|c| c.is_uppercase()) {
                    return Some(last.to_string());
                }
            }
        }
        None
    }

    fn should_suggest_components(&self, prefix: &str, text: &str, position: Position) -> bool {
        let trimmed = prefix.trim();

        // After children: [
        if trimmed.ends_with('[') || trimmed.ends_with(',') {
            return true;
        }

        // In children array
        if self.is_in_children_array(text, position) {
            return true;
        }

        // Start of line (new component reference)
        if trimmed.is_empty() {
            return true;
        }

        // After colon in certain properties
        if trimmed.ends_with(':') {
            return true;
        }

        false
    }

    fn is_in_children_array(&self, text: &str, position: Position) -> bool {
        let lines: Vec<&str> = text.lines().collect();
        let mut bracket_depth = 0;
        let mut in_children = false;

        for (i, line) in lines.iter().enumerate() {
            if i > position.line as usize {
                break;
            }

            let end = if i == position.line as usize {
                position.character as usize
            } else {
                line.len()
            };

            let slice = &line[..end.min(line.len())];

            if slice.contains("children:") || slice.contains("children :") {
                in_children = true;
            }

            for c in slice.chars() {
                if c == '[' {
                    bracket_depth += 1;
                } else if c == ']' {
                    bracket_depth -= 1;
                    if bracket_depth == 0 {
                        in_children = false;
                    }
                }
            }
        }

        in_children && bracket_depth > 0
    }

    fn get_current_imports(&self, text: &str) -> Vec<String> {
        let mut imports = Vec::new();

        for caps in IMPORT_RE.captures_iter(text) {
            for name in caps[1].split(',') {
                imports.push(name.trim().to_string());
            }
        }

        imports
    }

    fn get_word_at_position(&self, line: &str, char_pos: usize) -> Option<String> {
        let chars: Vec<char> = line.chars().collect();
        if char_pos >= chars.len() {
            return None;
        }

        let mut start = char_pos;
        let mut end = char_pos;

        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }

        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }

        if start == end {
            return None;
        }

        Some(chars[start..end].iter().collect())
    }

    fn get_property_documentation(&self, name: &str) -> Option<String> {
        match name {
            "type" => Some("**type**\n\nElement type: `text`, `button`, `input`, `link`, `select`, `textarea`, `container`, `form`".to_string()),
            "content" => Some("**content**\n\nText content to display".to_string()),
            "style" => Some("**style**\n\nTailwind CSS classes for styling".to_string()),
            "children" => Some("**children**\n\nArray of child components".to_string()),
            "align" => Some("**align**\n\nLayout direction: `vertical` or `horizontal`".to_string()),
            "click" => Some("**click**\n\nClick event handler (Store action or function)".to_string()),
            "onInput" => Some("**onInput**\n\nInput event handler for real-time updates".to_string()),
            "href" => Some("**href**\n\nURL for link components".to_string()),
            _ => None,
        }
    }
}
