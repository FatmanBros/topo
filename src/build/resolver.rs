//! Import resolution for topo files

use anyhow::{bail, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use topo::ast::{Declaration, Program};
use topo::lexer::Lexer;
use topo::parser::Parser as TopoParser;

/// Validate that a resolved path stays within the allowed boundary
fn validate_path_boundary(resolved: &Path, allowed_root: &Path) -> Result<PathBuf> {
    let canonical = resolved.canonicalize()?;
    let canonical_root = allowed_root.canonicalize()?;
    if !canonical.starts_with(&canonical_root) {
        bail!(
            "Import path escapes allowed boundary: {:?} is outside {:?}",
            canonical,
            canonical_root
        );
    }
    Ok(canonical)
}

/// Find the workspace root by computing common ancestor of project root and alias targets
fn find_workspace_root(project_root: &Path, aliases: &HashMap<String, String>) -> PathBuf {
    let mut roots: Vec<PathBuf> = vec![project_root.to_path_buf()];

    for target in aliases.values() {
        let target_path = if target == "." {
            project_root.to_path_buf()
        } else {
            project_root.join(target)
        };
        if let Ok(canonical) = target_path.canonicalize() {
            roots.push(canonical);
        }
    }

    // Find common ancestor
    if roots.is_empty() {
        return project_root.to_path_buf();
    }

    let first = roots[0].canonicalize().unwrap_or_else(|_| roots[0].clone());
    let mut common = first.clone();

    for root in &roots[1..] {
        while !root.starts_with(&common) {
            if let Some(parent) = common.parent() {
                common = parent.to_path_buf();
            } else {
                break;
            }
        }
    }

    common
}

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
    let mut lexer = Lexer::new(&source)?;
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
    let workspace_root = find_workspace_root(project_root, aliases);
    for import_path in imports {
        let import_file =
            resolve_import_path(&import_path, file_dir, base_dir, project_root, &workspace_root, aliases)?;
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
    workspace_root: &Path,
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
            // Canonicalize target_dir for boundary checking
            // Aliases are explicitly configured, so validate against the alias target directory
            let canonical_target = target_dir.canonicalize().unwrap_or_else(|_| target_dir.clone());
            let resolved = target_dir.join(alias_path);
            if resolved.exists() {
                // Validate that path stays within the alias target directory
                return validate_path_boundary(&resolved, &canonical_target);
            }
            // Try with .tp extension
            let with_ext = target_dir.join(format!("{}.tp", alias_path));
            if with_ext.exists() {
                return validate_path_boundary(&with_ext, &canonical_target);
            }
            bail!(
                "Cannot resolve import: {} (resolved to {:?})",
                import_path,
                resolved
            )
        }
    }

    // Try relative to current file first
    // Use workspace_root for validation to allow imports from aliased directories
    let relative_path = file_dir.join(import_path);
    if relative_path.exists() {
        return validate_path_boundary(&relative_path, workspace_root);
    }

    // Try with .tp extension
    let with_ext = file_dir.join(format!("{}.tp", import_path));
    if with_ext.exists() {
        return validate_path_boundary(&with_ext, workspace_root);
    }

    // Try relative to base directory
    let base_relative = base_dir.join(import_path);
    if base_relative.exists() {
        return validate_path_boundary(&base_relative, workspace_root);
    }

    let base_with_ext = base_dir.join(format!("{}.tp", import_path));
    if base_with_ext.exists() {
        return validate_path_boundary(&base_with_ext, workspace_root);
    }

    bail!(
        "Cannot resolve import: {} (looked in {:?} and {:?})",
        import_path,
        file_dir,
        base_dir
    )
}
