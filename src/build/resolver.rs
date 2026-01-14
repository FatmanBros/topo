//! Import resolution for topo files

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use topo::ast::{Declaration, Program};
use topo::lexer::Lexer;
use topo::parser::Parser as TopoParser;

/// Recursively resolve imports and build dependency order
pub fn resolve_imports(
    file: &PathBuf,
    base_dir: &PathBuf,
    project_root: &Path,
    parsed: &mut HashMap<PathBuf, Program>,
    order: &mut Vec<PathBuf>,
    aliases: &HashMap<String, String>,
) -> Result<()> {
    // Normalize path
    let file = if file.is_absolute() {
        file.clone()
    } else {
        std::env::current_dir()?.join(file)
    };
    let file = file.canonicalize().unwrap_or(file);

    // Skip if already parsed
    if parsed.contains_key(&file) {
        return Ok(());
    }

    // Parse the file
    let source = fs::read_to_string(&file)?;
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize()?;
    let mut parser = TopoParser::new(tokens);
    let program = parser.parse()?;

    // Find imports in this file
    let imports: Vec<String> = program
        .declarations
        .iter()
        .filter_map(|decl| {
            if let Declaration::Import(import) = decl {
                Some(import.path.clone())
            } else {
                None
            }
        })
        .collect();

    // Store the parsed program
    parsed.insert(file.clone(), program);

    // Resolve imports first (dependencies before dependents)
    let file_dir = file.parent().unwrap_or(base_dir);
    for import_path in imports {
        let import_file = resolve_import_path(&import_path, file_dir, base_dir, project_root, aliases)?;
        resolve_imports(&import_file, base_dir, project_root, parsed, order, aliases)?;
    }

    // Add this file to the order (after its dependencies)
    if !order.contains(&file) {
        order.push(file);
    }

    Ok(())
}

/// Resolve an import path relative to the current file or base directory
/// Supports configurable path aliases (e.g., @/components/atoms/text.tp)
pub fn resolve_import_path(
    import_path: &str,
    file_dir: &Path,
    base_dir: &PathBuf,
    project_root: &Path,
    aliases: &HashMap<String, String>,
) -> Result<PathBuf> {
    // Check for alias prefix (e.g., "@/", "~/", etc.)
    for (alias, target) in aliases {
        let alias_prefix = format!("{}/", alias);
        if import_path.starts_with(&alias_prefix) {
            let alias_path = &import_path[alias_prefix.len()..];
            // Resolve target relative to project root
            let target_dir = if target == "." {
                project_root.to_path_buf()
            } else {
                project_root.join(target)
            };
            let resolved = target_dir.join(alias_path);
            if resolved.exists() {
                return Ok(resolved.canonicalize()?);
            }
            // Try with .tp extension
            let with_ext = target_dir.join(format!("{}.tp", alias_path));
            if with_ext.exists() {
                return Ok(with_ext.canonicalize()?);
            }
            anyhow::bail!("Cannot resolve import: {} (resolved to {:?})", import_path, resolved)
        }
    }

    // Try relative to current file first
    let relative_path = file_dir.join(import_path);
    if relative_path.exists() {
        return Ok(relative_path.canonicalize()?);
    }

    // Try with .tp extension
    let with_ext = file_dir.join(format!("{}.tp", import_path));
    if with_ext.exists() {
        return Ok(with_ext.canonicalize()?);
    }

    // Try relative to base directory
    let base_relative = base_dir.join(import_path);
    if base_relative.exists() {
        return Ok(base_relative.canonicalize()?);
    }

    let base_with_ext = base_dir.join(format!("{}.tp", import_path));
    if base_with_ext.exists() {
        return Ok(base_with_ext.canonicalize()?);
    }

    anyhow::bail!("Cannot resolve import: {} (looked in {:?} and {:?})", import_path, file_dir, base_dir)
}
