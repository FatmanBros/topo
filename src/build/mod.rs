//! Build module - handles project compilation and code generation

mod builder;
mod html;
mod resolver;

pub use builder::{build_project, build_project_dev};
#[allow(unused_imports)]
pub use html::{generate_html, generate_html_dev, generate_html_ssg};
#[allow(unused_imports)]
pub use resolver::{resolve_imports, resolve_import_path};

use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

// Pre-compiled regex patterns for function deduplication
static FUNC_DECL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"function\s+([A-Z][a-zA-Z0-9_]*)\s*\(").expect("Invalid function declaration regex")
});
static CONST_DECL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"const\s+([A-Z][a-zA-Z0-9_]*)\s*=").expect("Invalid const declaration regex")
});

/// Copy all files from source directory to destination
pub fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            fs::create_dir_all(&dst_path)?;
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Deduplicate function definitions in generated JS code by renaming duplicates
pub fn deduplicate_functions(js: &str, defined_names: &mut HashSet<String>) -> String {
    use std::collections::HashMap;

    // First pass: find all names defined in this chunk
    let mut local_functions: Vec<String> = Vec::new();
    for cap in FUNC_DECL_RE.captures_iter(js) {
        if let Some(name_match) = cap.get(1) {
            let name = name_match.as_str().to_string();
            if !local_functions.contains(&name) {
                local_functions.push(name);
            }
        }
    }
    for cap in CONST_DECL_RE.captures_iter(js) {
        if let Some(name_match) = cap.get(1) {
            let name = name_match.as_str().to_string();
            if !local_functions.contains(&name) {
                local_functions.push(name);
            }
        }
    }

    // Build rename map for duplicates
    let mut rename_map: HashMap<String, String> = HashMap::new();
    for name in &local_functions {
        if defined_names.contains(name) {
            // Find a unique suffix
            let mut suffix = 1;
            loop {
                let new_name = format!("{}_{}", name, suffix);
                if !defined_names.contains(&new_name) {
                    rename_map.insert(name.clone(), new_name.clone());
                    defined_names.insert(new_name);
                    break;
                }
                suffix += 1;
            }
        } else {
            defined_names.insert(name.clone());
        }
    }

    // If no renames needed, return as-is
    if rename_map.is_empty() {
        return js.to_string();
    }

    // Apply renames - replace function/const declarations and references
    let mut result = js.to_string();
    for (old_name, new_name) in &rename_map {
        // Replace function declaration: "function OldName(" -> "function NewName("
        let decl_pattern = format!(r"function\s+{}\s*\(", regex::escape(old_name));
        if let Ok(decl_regex) = Regex::new(&decl_pattern) {
            result = decl_regex.replace_all(&result, format!("function {}(", new_name)).to_string();
        }

        // Replace const declaration: "const OldName =" -> "const NewName ="
        let const_pattern = format!(r"const\s+{}\s*=", regex::escape(old_name));
        if let Ok(const_regex) = Regex::new(&const_pattern) {
            result = const_regex.replace_all(&result, format!("const {} =", new_name)).to_string();
        }

        // Replace references using word boundaries
        let ref_pattern = format!(r"\b{}\b", regex::escape(old_name));
        if let Ok(ref_regex) = Regex::new(&ref_pattern) {
            result = ref_regex.replace_all(&result, new_name.as_str()).to_string();
        }
    }

    result
}

/// Minify JavaScript code for production builds
pub fn minify_js(js: &str) -> String {
    let mut result = String::new();

    for line in js.lines() {
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Skip lines that are only single-line comments
        if trimmed.starts_with("//") {
            continue;
        }

        // For other lines, just collapse leading/trailing whitespace
        // but preserve internal whitespace and content
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(trimmed);
    }

    result
}

/// Find project root by looking for topo.config.json
pub fn find_project_root(input: &Path) -> Result<PathBuf> {
    let start_dir = if input.is_file() {
        input.parent().unwrap_or(input).to_path_buf()
    } else {
        input.to_path_buf()
    };

    let mut current = start_dir.canonicalize().unwrap_or_else(|_| start_dir.clone());
    loop {
        if current.join("topo.config.json").exists() {
            return Ok(current);
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    // Fall back to input directory or its parent
    if input.is_file() {
        Ok(input.parent().unwrap_or(input).to_path_buf())
    } else {
        Ok(input.to_path_buf())
    }
}

/// Find all .tp files in a directory
pub fn find_tp_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if dir.is_file() {
        if dir.extension().is_some_and(|ext| ext == "tp") {
            files.push(dir.to_path_buf());
        }
        return Ok(files);
    }

    if dir.extension().is_some_and(|ext| ext == "tp") {
        files.push(dir.to_path_buf());
        return Ok(files);
    }

    // Look for pages directory for file-based routing
    let pages_dir = dir.join("pages");
    if pages_dir.exists() {
        collect_tp_files_recursive(&pages_dir, &mut files)?;
    }

    // Also check services directory for API definitions
    let services_dir = dir.join("services");
    if services_dir.exists() {
        collect_tp_files_recursive(&services_dir, &mut files)?;
    }

    // Check components directory
    let components_dir = dir.join("components");
    if components_dir.exists() {
        collect_tp_files_recursive(&components_dir, &mut files)?;
    }

    // If no standard directories found, scan the whole directory
    if files.is_empty() {
        collect_tp_files_recursive(dir, &mut files)?;
    }

    Ok(files)
}

fn collect_tp_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Skip non-source directories
            if dir_name == "node_modules" || dir_name == "target" || dir_name.starts_with('.') {
                continue;
            }
            collect_tp_files_recursive(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "tp") {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Skip http setup and routes definition files (handled separately)
            if file_name != "http.setup.tp" && file_name != "routes.tp" {
                files.push(path);
            }
        }
    }

    Ok(())
}

/// Generate i18n runtime code
pub fn generate_i18n_runtime(config: &topo::config::I18nConfig) -> String {
    let mut output = String::new();
    output.push_str("\n// i18n Internationalization\n");

    // Generate translations object
    output.push_str("const __i18n = {\n");
    output.push_str(&format!("  locale: '{}',\n", config.default_locale));
    output.push_str(&format!("  locales: {:?},\n", config.locales));
    output.push_str("  translations: {\n");

    if let Some(translations) = &config.translations {
        for (key, locales) in translations {
            output.push_str(&format!("    '{}': {{\n", key));
            for (locale, value) in locales {
                // Escape single quotes in value
                let escaped_value = value.replace('\'', "\\'");
                output.push_str(&format!("      '{}': '{}',\n", locale, escaped_value));
            }
            output.push_str("    },\n");
        }
    }

    output.push_str("  },\n");
    output.push_str("  subscribers: [],\n");
    output.push_str("};\n\n");

    // Generate t() function for translations
    output.push_str("function t(key, params = {}) {\n");
    output.push_str("  const translation = __i18n.translations[key];\n");
    output.push_str("  if (!translation) return key;\n");
    output.push_str("  let text = translation[__i18n.locale] || translation[Object.keys(translation)[0]] || key;\n");
    output.push_str("  // Replace {{param}} placeholders\n");
    output.push_str("  for (const [k, v] of Object.entries(params)) {\n");
    output.push_str("    text = text.replace(new RegExp(`{{${k}}}`, 'g'), v);\n");
    output.push_str("  }\n");
    output.push_str("  return text;\n");
    output.push_str("}\n\n");

    // Generate $i18n store
    output.push_str("const $i18n = {\n");
    output.push_str("  get locale() { return __i18n.locale; },\n");
    output.push_str("  get locales() { return __i18n.locales; },\n");
    output.push_str("  setLocale(locale) {\n");
    output.push_str("    if (__i18n.locales.includes(locale)) {\n");
    output.push_str("      __i18n.locale = locale;\n");
    output.push_str("      __i18n.subscribers.forEach(fn => fn());\n");
    output.push_str("      __rerender();\n");
    output.push_str("    }\n");
    output.push_str("  },\n");
    output.push_str("  subscribe(fn) { __i18n.subscribers.push(fn); },\n");
    output.push_str("};\n");
    output.push_str("stores.set('i18n', $i18n);\n\n");

    output
}
