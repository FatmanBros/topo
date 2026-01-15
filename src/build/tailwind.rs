//! Tailwind CSS build integration

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Build Tailwind CSS from input.css to output styles.css
pub fn build_tailwind(project_root: &Path, output_dir: &Path, minify: bool) -> Result<()> {
    let input_css = project_root.join("input.css");
    let output_css = output_dir.join("styles.css");

    // Check if input.css exists
    if !input_css.exists() {
        // Create default input.css if it doesn't exist
        std::fs::write(&input_css, "@tailwind base;\n@tailwind components;\n@tailwind utilities;\n")?;
        println!("  Created: input.css");
    }

    // Check if npx is available
    let npx_check = Command::new("npx")
        .arg("--version")
        .output();

    if npx_check.is_err() {
        println!("  Warning: npx not found, skipping Tailwind CSS build");
        println!("  Run 'npm install' to enable Tailwind CSS compilation");
        return Ok(());
    }

    // Check if tailwindcss is installed
    let tailwind_check = Command::new("npx")
        .args(["tailwindcss", "--help"])
        .current_dir(project_root)
        .output();

    if tailwind_check.is_err() {
        println!("  Warning: tailwindcss not found, skipping Tailwind CSS build");
        println!("  Run 'npm install tailwindcss' to enable Tailwind CSS compilation");
        return Ok(());
    }

    println!("  Building Tailwind CSS...");

    // Build arguments
    let mut args = vec![
        "tailwindcss".to_string(),
        "-i".to_string(),
        input_css.to_string_lossy().to_string(),
        "-o".to_string(),
        output_css.to_string_lossy().to_string(),
    ];

    if minify {
        args.push("--minify".to_string());
    }

    // Run tailwindcss
    let output = Command::new("npx")
        .args(&args)
        .current_dir(project_root)
        .output()
        .context("Failed to run tailwindcss")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            eprintln!("  Tailwind CSS build warning: {}", stderr);
        }
        // Don't fail the build, just warn
        println!("  Warning: Tailwind CSS build may have issues");
    } else {
        println!("  Generated: styles.css");
    }

    Ok(())
}
