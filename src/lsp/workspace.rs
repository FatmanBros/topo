use dashmap::DashMap;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::*;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub file_path: PathBuf,
    pub line: u32,
    pub is_store: bool,
    pub is_alias: bool,
    pub exports: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub name: String,
    pub param_type: Option<String>,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ImportInfo {
    pub names: Vec<String>,
    pub path: String,
    pub line: u32,
}

pub struct WorkspaceManager {
    /// Component name -> ComponentInfo
    components: DashMap<String, ComponentInfo>,
    /// File path -> list of exports from that file
    file_exports: DashMap<PathBuf, Vec<String>>,
    /// File path -> list of imports in that file
    file_imports: DashMap<PathBuf, Vec<ImportInfo>>,
    /// Root directory
    root: Option<PathBuf>,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self {
            components: DashMap::new(),
            file_exports: DashMap::new(),
            file_imports: DashMap::new(),
            root: None,
        }
    }

    pub fn scan_directory(&self, dir: &Path) {
        for entry in WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "tp"))
        {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                self.update_file(entry.path(), &content);
            }
        }
    }

    pub fn update_file(&self, path: &Path, content: &str) {
        let path_buf = path.to_path_buf();

        // Remove old exports from this file
        if let Some((_, old_exports)) = self.file_exports.remove(&path_buf) {
            for name in old_exports {
                self.components.remove(&name);
            }
        }

        let mut exports = Vec::new();
        let mut imports = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            // Parse imports
            if trimmed.starts_with("import") {
                if let Some(import) = self.parse_import(trimmed, line_num as u32) {
                    imports.push(import);
                }
                continue;
            }

            // Parse component definitions: Name(params) -> { or Name -> {
            if let Some(comp) = self.parse_component_def(trimmed, &path_buf, line_num as u32) {
                exports.push(comp.name.clone());
                self.components.insert(comp.name.clone(), comp);
                continue;
            }

            // Parse component aliases: Alias(params) -> Base(args)
            if let Some(comp) = self.parse_component_alias(trimmed, &path_buf, line_num as u32) {
                exports.push(comp.name.clone());
                self.components.insert(comp.name.clone(), comp);
                continue;
            }

            // Parse store definitions: Name | {
            if let Some(store) = self.parse_store_def(trimmed, &path_buf, line_num as u32) {
                exports.push(store.name.clone());
                self.components.insert(store.name.clone(), store);
                continue;
            }

            // Parse API service: Name :: {
            if let Some(api) = self.parse_api_def(trimmed, &path_buf, line_num as u32) {
                exports.push(api.name.clone());
                self.components.insert(api.name.clone(), api);
            }
        }

        self.file_exports.insert(path_buf.clone(), exports);
        self.file_imports.insert(path_buf, imports);
    }

    fn parse_import(&self, line: &str, line_num: u32) -> Option<ImportInfo> {
        // import { A, B } from "path"
        let re = regex::Regex::new(r#"import\s*\{\s*([^}]+)\s*\}\s*from\s*["']([^"']+)["']"#).ok()?;
        let caps = re.captures(line)?;

        let names: Vec<String> = caps[1]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Some(ImportInfo {
            names,
            path: caps[2].to_string(),
            line: line_num,
        })
    }

    fn parse_component_def(&self, line: &str, path: &Path, line_num: u32) -> Option<ComponentInfo> {
        // Name(param1, param2) -> {  or  Name -> {
        let re = regex::Regex::new(r"^([A-Z][a-zA-Z0-9]*)\s*(\(([^)]*)\))?\s*->\s*\{").ok()?;
        let caps = re.captures(line)?;

        let name = caps[1].to_string();
        let params = caps
            .get(3)
            .map(|m| self.parse_params(m.as_str()))
            .unwrap_or_default();

        Some(ComponentInfo {
            name,
            params,
            file_path: path.to_path_buf(),
            line: line_num,
            is_store: false,
            is_alias: false,
            exports: vec![],
        })
    }

    fn parse_component_alias(
        &self,
        line: &str,
        path: &Path,
        line_num: u32,
    ) -> Option<ComponentInfo> {
        // Alias(params) -> Base(args...)
        let re =
            regex::Regex::new(r"^([A-Z][a-zA-Z0-9]*)\s*(\(([^)]*)\))?\s*->\s*([A-Z][a-zA-Z0-9]*)\s*\(")
                .ok()?;
        let caps = re.captures(line)?;

        let name = caps[1].to_string();
        let params = caps
            .get(3)
            .map(|m| self.parse_params(m.as_str()))
            .unwrap_or_default();

        Some(ComponentInfo {
            name,
            params,
            file_path: path.to_path_buf(),
            line: line_num,
            is_store: false,
            is_alias: true,
            exports: vec![],
        })
    }

    fn parse_store_def(&self, line: &str, path: &Path, line_num: u32) -> Option<ComponentInfo> {
        // Name | {
        let re = regex::Regex::new(r"^([A-Z][a-zA-Z0-9]*)\s*\|\s*\{").ok()?;
        let caps = re.captures(line)?;

        Some(ComponentInfo {
            name: caps[1].to_string(),
            params: vec![],
            file_path: path.to_path_buf(),
            line: line_num,
            is_store: true,
            is_alias: false,
            exports: vec![],
        })
    }

    fn parse_api_def(&self, line: &str, path: &Path, line_num: u32) -> Option<ComponentInfo> {
        // Name :: {
        let re = regex::Regex::new(r"^([A-Z][a-zA-Z0-9]*)\s*::\s*\{").ok()?;
        let caps = re.captures(line)?;

        Some(ComponentInfo {
            name: caps[1].to_string(),
            params: vec![],
            file_path: path.to_path_buf(),
            line: line_num,
            is_store: false,
            is_alias: false,
            exports: vec![],
        })
    }

    fn parse_params(&self, params_str: &str) -> Vec<ParamInfo> {
        params_str
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| ParamInfo {
                name: s.to_string(),
                param_type: None,
                default_value: None,
            })
            .collect()
    }

    pub fn get_component(&self, name: &str) -> Option<ComponentInfo> {
        self.components.get(name).map(|c| c.clone())
    }

    pub fn get_all_components(&self) -> Vec<ComponentInfo> {
        self.components.iter().map(|r| r.value().clone()).collect()
    }

    pub fn get_imports(&self, path: &Path) -> Vec<ImportInfo> {
        self.file_imports
            .get(&path.to_path_buf())
            .map(|r| r.clone())
            .unwrap_or_default()
    }

    pub fn is_imported(&self, name: &str, current_file: &Path) -> bool {
        if let Some(imports) = self.file_imports.get(&current_file.to_path_buf()) {
            return imports.iter().any(|i| i.names.contains(&name.to_string()));
        }
        false
    }

    pub fn find_definition(&self, text: &str, position: Position) -> Option<Location> {
        // Get the word at position
        let lines: Vec<&str> = text.lines().collect();
        let line = lines.get(position.line as usize)?;

        let word = self.get_word_at_position(line, position.character as usize)?;

        // Look up component
        let comp = self.components.get(&word)?;

        let uri = Url::from_file_path(&comp.file_path).ok()?;
        Some(Location {
            uri,
            range: Range::new(
                Position::new(comp.line, 0),
                Position::new(comp.line, word.len() as u32),
            ),
        })
    }

    pub fn find_import_path(&self, component_name: &str, current_uri: &Url) -> Option<String> {
        let comp = self.components.get(component_name)?;
        let current_path = current_uri.to_file_path().ok()?;
        let current_dir = current_path.parent()?;

        // Calculate relative path
        let rel_path = pathdiff::diff_paths(&comp.file_path, current_dir)?;
        let mut path_str = rel_path.to_string_lossy().to_string();

        // Ensure it starts with ./ or ../
        if !path_str.starts_with('.') {
            path_str = format!("./{}", path_str);
        }

        Some(path_str)
    }

    fn get_word_at_position(&self, line: &str, char_pos: usize) -> Option<String> {
        let chars: Vec<char> = line.chars().collect();
        if char_pos >= chars.len() {
            return None;
        }

        // Find word boundaries
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
}
