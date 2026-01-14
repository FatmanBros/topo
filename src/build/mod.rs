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
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

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

/// Deduplicate function definitions in generated JS code
pub fn deduplicate_functions(js: &str, defined_names: &mut HashSet<String>) -> String {
    let mut result = String::new();
    let mut skip_until_closing_brace = false;
    let mut brace_count = 0;

    for line in js.lines() {
        // Check if this is a function definition
        if line.trim_start().starts_with("function ") {
            // Extract function name
            if let Some(name_start) = line.find("function ").map(|i| i + 9) {
                if let Some(name_end) = line[name_start..].find('(') {
                    let name = line[name_start..name_start + name_end].trim().to_string();
                    if defined_names.contains(&name) {
                        // Skip this function definition
                        skip_until_closing_brace = true;
                        brace_count = 0;
                        continue;
                    } else {
                        defined_names.insert(name);
                    }
                }
            }
        }

        if skip_until_closing_brace {
            // Count braces to find the end of the function
            for c in line.chars() {
                if c == '{' {
                    brace_count += 1;
                } else if c == '}' {
                    brace_count -= 1;
                    if brace_count == 0 {
                        skip_until_closing_brace = false;
                        break;
                    }
                }
            }
            continue;
        }

        result.push_str(line);
        result.push('\n');
    }

    result
}

/// Minify JavaScript code for production builds
pub fn minify_js(js: &str) -> String {
    let mut result = String::new();
    let mut in_string = false;
    let mut string_char = ' ';
    let mut prev_char = ' ';
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut last_was_space = false;

    let chars: Vec<char> = js.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];
        let next_char = if i + 1 < len { chars[i + 1] } else { ' ' };

        // Handle comments
        if !in_string {
            if c == '/' && next_char == '/' && !in_block_comment {
                in_line_comment = true;
                i += 1;
                continue;
            }
            if c == '/' && next_char == '*' && !in_line_comment {
                in_block_comment = true;
                i += 2;
                continue;
            }
            if in_line_comment {
                if c == '\n' {
                    in_line_comment = false;
                    if !last_was_space {
                        result.push(' ');
                        last_was_space = true;
                    }
                }
                i += 1;
                continue;
            }
            if in_block_comment {
                if c == '*' && next_char == '/' {
                    in_block_comment = false;
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }
        }

        // Handle strings
        if (c == '"' || c == '\'' || c == '`') && prev_char != '\\' {
            if in_string && c == string_char {
                in_string = false;
            } else if !in_string {
                in_string = true;
                string_char = c;
            }
        }

        // Handle whitespace
        if !in_string && (c == ' ' || c == '\t' || c == '\n' || c == '\r') {
            if !last_was_space && !result.is_empty() {
                // Check if space is needed between tokens
                let last_result_char = result.chars().last().unwrap_or(' ');
                if (last_result_char.is_alphanumeric()
                    || last_result_char == '_'
                    || last_result_char == '$')
                    && i + 1 < len
                {
                    let next_non_space = chars[i + 1..].iter().find(|&&ch| ch != ' ' && ch != '\t' && ch != '\n' && ch != '\r');
                    if let Some(&nc) = next_non_space {
                        if nc.is_alphanumeric() || nc == '_' || nc == '$' {
                            result.push(' ');
                            last_was_space = true;
                        }
                    }
                }
            }
            i += 1;
            continue;
        }

        result.push(c);
        last_was_space = false;
        prev_char = c;
        i += 1;
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
    let mut runtime = String::new();
    runtime.push_str("\n// i18n Runtime\n");
    runtime.push_str("const __i18n = {\n");
    runtime.push_str(&format!("  defaultLocale: '{}',\n", config.default_locale));
    runtime.push_str(&format!("  locales: {:?},\n", config.locales));
    runtime.push_str("  translations: {},\n");
    runtime.push_str("  currentLocale: null,\n");
    runtime.push_str("  async loadTranslations(locale) {\n");
    runtime.push_str("    if (!this.translations[locale]) {\n");
    runtime.push_str(&format!(
        "      const response = await fetch('{}/' + locale + '.json');\n",
        config.translations_dir.as_deref().unwrap_or("locales")
    ));
    runtime.push_str("      this.translations[locale] = await response.json();\n");
    runtime.push_str("    }\n");
    runtime.push_str("    this.currentLocale = locale;\n");
    runtime.push_str("    return this.translations[locale];\n");
    runtime.push_str("  },\n");
    runtime.push_str("  t(key, params = {}) {\n");
    runtime.push_str("    const locale = this.currentLocale || this.defaultLocale;\n");
    runtime.push_str("    const translations = this.translations[locale] || {};\n");
    runtime.push_str("    let text = key.split('.').reduce((obj, k) => obj?.[k], translations) || key;\n");
    runtime.push_str("    Object.entries(params).forEach(([k, v]) => {\n");
    runtime.push_str("      text = text.replace(new RegExp(`{${k}}`, 'g'), v);\n");
    runtime.push_str("    });\n");
    runtime.push_str("    return text;\n");
    runtime.push_str("  }\n");
    runtime.push_str("};\n");
    runtime.push_str("const t = (key, params) => __i18n.t(key, params);\n");
    runtime.push_str(&format!(
        "__i18n.loadTranslations('{}');\n\n",
        config.default_locale
    ));
    runtime
}
