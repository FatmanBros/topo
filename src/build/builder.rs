//! Main build functions for production and development builds

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use topo::ast::{Declaration, Program};
use topo::codegen::JsCodegen;
use topo::config::Config;
use topo::lexer::Lexer;
use topo::parser::Parser as TopoParser;

use super::{
    build_tailwind, copy_dir_contents, deduplicate_functions, find_project_root, find_tp_files,
    generate_i18n_runtime, minify_js,
};
use super::html::{generate_html, generate_html_dev, generate_html_ssg};
use super::resolver::resolve_imports;
use crate::deploy::generate_routes;
use crate::deploy::generate_ssr_output;
use crate::deploy::{extract_api_services_with_server, generate_cloudflare_api, generate_axum_api};

/// Build project for production
pub fn build_project(input: &PathBuf, output: &PathBuf, mode: &str, target: &str) -> Result<()> {
    println!("Building project...");
    println!("  Input: {:?}", input);
    println!("  Output: {:?}", output);
    println!("  Mode: {}", mode);
    if mode == "ssr" {
        println!("  Target: {}", target);
    }

    // Create output directory
    fs::create_dir_all(output)?;

    // Find all .tp files or use single file
    let entry_files = find_tp_files(input)?;
    println!("  Found {} .tp files", entry_files.len());

    // Project root is where topo.config.json is located
    let project_root = find_project_root(input)?;

    // Load config from project root
    let config = Config::load(&project_root.join("topo.config.json")).unwrap_or_default();
    let paths_config = config.paths_config();
    let aliases = paths_config.aliases;

    // Parse all files and resolve imports
    let mut parsed_files: HashMap<PathBuf, Program> = HashMap::new();
    let mut compile_order: Vec<PathBuf> = Vec::new();

    // Parse entry files and their dependencies
    for file in &entry_files {
        resolve_imports(file, input, &project_root, &mut parsed_files, &mut compile_order, &aliases)?;
    }

    println!("  Compiling {} files in dependency order", compile_order.len());

    // Generate code in dependency order
    let mut all_output = String::new();
    let mut codegen = JsCodegen::new();

    // First pass: collect all component params and store state fields from all files
    for file in &compile_order {
        if let Some(program) = parsed_files.get(file) {
            let file_path = file.to_str();
            codegen.collect_component_params(program);
            codegen.collect_store_state_fields(program, file_path);
        }
    }

    // Generate runtime once at the beginning
    all_output.push_str(&codegen.generate_runtime());

    // Generate i18n runtime if configured
    if let Some(i18n_config) = &config.i18n {
        all_output.push_str(&generate_i18n_runtime(i18n_config));
    }

    // Load http.setup.tp if exists
    let http_setup_path = project_root.join("http.setup.tp");
    if http_setup_path.exists() {
        println!("  Loading http.setup.tp...");
        let setup_source = fs::read_to_string(&http_setup_path)?;
        all_output.push_str("\n// HTTP Setup\n");
        all_output.push_str(&setup_source);
        all_output.push('\n');
    }

    // Load routes.tp if exists
    let routes_def_path = project_root.join("routes.tp");
    if routes_def_path.exists() {
        println!("  Loading routes.tp...");
        let routes_source = fs::read_to_string(&routes_def_path)?;
        let mut lexer = match Lexer::new(&routes_source) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Error creating lexer for routes.tp: {}", e);
                return Ok(());
            }
        };
        match lexer.tokenize() {
            Ok(tokens) => {
                let mut parser = TopoParser::new(tokens);
                match parser.parse() {
                    Ok(program) => {
                        all_output.push_str("\n// Routes Definition\n");
                        let routes_js = codegen.generate(&program);
                        all_output.push_str(&routes_js);
                        all_output.push('\n');
                    }
                    Err(e) => {
                        eprintln!("Error parsing routes.tp: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error tokenizing routes.tp: {}", e);
            }
        }
    }

    // Generate file-based routes
    let routes = generate_routes(&entry_files, input)?;

    // Track defined function names to avoid duplicates
    let mut defined_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut has_app = false;
    let mut entry_component: Option<String> = None;
    for file in &compile_order {
        println!("  Compiling: {:?}", file);
        if let Some(program) = parsed_files.get(file) {
            for decl in &program.declarations {
                if let Declaration::Component(comp) = decl {
                    if comp.name == "App" {
                        has_app = true;
                        entry_component = Some("App".to_string());
                    } else if (comp.name == "AppPage" || comp.name == "Page") && entry_component.is_none() {
                        entry_component = Some(comp.name.clone());
                    }
                }
            }
            let js = codegen.generate_with_file_path(program, file.to_str());
            let js = deduplicate_functions(&js, &mut defined_names);
            all_output.push_str(&js);
            all_output.push('\n');
        }
    }

    // Register file-based routes
    if !routes.is_empty() {
        all_output.push_str("\n// File-based routes\n");
        for (pattern, component) in &routes {
            all_output.push_str(&format!("registerRoute('{}', {});\n", pattern, component));
            all_output.push_str(&format!("registerComponent('{}', {});\n", component, component));
        }
        all_output.push('\n');
    }

    // Add mount call
    if !routes.is_empty() {
        all_output.push_str("// Mount with router\n");
        all_output.push_str("mount(null, '#app');\n");
    } else if has_app {
        all_output.push_str("// Mount app\n");
        all_output.push_str("mount(App, '#app');\n");
    } else if let Some(entry) = &entry_component {
        all_output.push_str("// Mount entry component\n");
        all_output.push_str(&format!("mount({}, '#app');\n", entry));
    }

    // Minify JS for production
    let final_js = if mode == "ssg" {
        minify_js(&all_output)
    } else {
        all_output
    };

    // Write output
    let output_file = output.join("app.js");
    fs::write(&output_file, &final_js)?;
    println!("✓ Build complete: {:?}", output_file);

    // Generate HTML
    let html = if mode == "ssg" {
        generate_html_ssg(&config, &final_js)
    } else {
        generate_html(&config)
    };
    fs::write(output.join("index.html"), &html)?;

    // Build Tailwind CSS
    let minify_css = mode == "ssg";
    build_tailwind(&project_root, output, minify_css)?;

    // SSG mode: generate HTML files for each static route
    if mode == "ssg" {
        fs::write(output.join("404.html"), &html)?;
        println!("  Generated: 404.html");

        for (route_pattern, _component) in &routes {
            if route_pattern.contains('[') {
                continue;
            }
            if route_pattern == "/" {
                continue;
            }

            let route_path = route_pattern.trim_start_matches('/');
            let route_dir = output.join(route_path);
            fs::create_dir_all(&route_dir)?;
            fs::write(route_dir.join("index.html"), &html)?;
            println!("  Generated: {}/index.html", route_path);
        }
    }

    // Copy public folder
    let public_dir = project_root.join("public");
    if public_dir.exists() && public_dir.is_dir() {
        copy_dir_contents(&public_dir, output)?;
    }

    // SSR mode
    if mode == "ssr" {
        generate_ssr_output(output, &routes, &config, target)?;
    }

    // Generate Server API code if API services with server blocks exist
    let programs: Vec<&Program> = parsed_files.values().collect();
    let api_services = extract_api_services_with_server(&programs);
    if !api_services.is_empty() {
        println!("  Found {} API services with server blocks", api_services.len());
        match target {
            "cloudflare" => generate_cloudflare_api(output, &api_services, &config)?,
            "axum" | "rust" => generate_axum_api(output, &api_services, &config)?,
            _ => {
                // Default: generate both
                generate_cloudflare_api(output, &api_services, &config)?;
            }
        }
    }

    Ok(())
}

/// Build project for development mode (with hot reload script)
pub fn build_project_dev(
    input: &PathBuf,
    output: &PathBuf,
    _mode: &str,
    ws_port: u16,
    config: &Config,
) -> Result<()> {
    fs::create_dir_all(output)?;

    let entry_files = find_tp_files(input)?;
    let project_root = find_project_root(input)?;

    let project_config = Config::load(&project_root.join("topo.config.json")).unwrap_or_default();
    let paths_config = project_config.paths_config();
    let aliases = paths_config.aliases;

    let mut parsed_files: HashMap<PathBuf, Program> = HashMap::new();
    let mut compile_order: Vec<PathBuf> = Vec::new();

    for file in &entry_files {
        resolve_imports(file, input, &project_root, &mut parsed_files, &mut compile_order, &aliases)?;
    }

    let mut all_output = String::new();
    let mut codegen = JsCodegen::new();

    for file in &compile_order {
        if let Some(program) = parsed_files.get(file) {
            let file_path = file.to_str();
            codegen.collect_component_params(program);
            codegen.collect_store_state_fields(program, file_path);
        }
    }

    all_output.push_str(&codegen.generate_runtime());

    if let Some(i18n_config) = &config.i18n {
        all_output.push_str(&generate_i18n_runtime(i18n_config));
    }

    let http_setup_path = project_root.join("http.setup.tp");
    if http_setup_path.exists() {
        let setup_source = fs::read_to_string(&http_setup_path)?;
        all_output.push_str("\n// HTTP Setup\n");
        all_output.push_str(&setup_source);
        all_output.push('\n');
    }

    let routes = generate_routes(&entry_files, input)?;

    let mut defined_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut has_app = false;
    for file in &compile_order {
        if let Some(program) = parsed_files.get(file) {
            for decl in &program.declarations {
                if let Declaration::Component(comp) = decl {
                    if comp.name == "App" {
                        has_app = true;
                    }
                }
            }
            let js = codegen.generate_with_file_path(program, file.to_str());
            let js = deduplicate_functions(&js, &mut defined_names);
            all_output.push_str(&js);
            all_output.push('\n');
        }
    }

    if !routes.is_empty() {
        all_output.push_str("\n// File-based routes\n");
        for (pattern, component) in &routes {
            all_output.push_str(&format!("registerRoute('{}', {});\n", pattern, component));
            all_output.push_str(&format!("registerComponent('{}', {});\n", component, component));
        }
        all_output.push('\n');
    }

    if has_app {
        all_output.push_str("// Mount app\n");
        all_output.push_str("mount(App, '#app');\n");
    } else if !routes.is_empty() {
        all_output.push_str("// Mount with router\n");
        all_output.push_str("mount(null, '#app');\n");
    }

    let output_file = output.join("app.js");
    fs::write(&output_file, &all_output)?;

    let html = generate_html_dev(config, ws_port + 1);
    fs::write(output.join("index.html"), html)?;

    let public_dir = project_root.join("public");
    if public_dir.exists() && public_dir.is_dir() {
        copy_dir_contents(&public_dir, output)?;
    }

    Ok(())
}
