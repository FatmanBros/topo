//! Commands module - CLI command handlers

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use topo::ast::{Declaration, TypeAnnotation};
use topo::config::Config;
use topo::lexer::Lexer;
use topo::parser::Parser as TopoParser;

use crate::build;
use crate::deploy;

pub fn check_project(input: &Path) -> Result<()> {
    println!("Checking project...");

    let tp_files = build::find_tp_files(input)?;
    let mut errors = 0;

    for file in &tp_files {
        print!("  Checking {:?}... ", file);

        let source = fs::read_to_string(file)?;
        let mut lexer = Lexer::new(&source);

        match lexer.tokenize() {
            Ok(tokens) => {
                let mut parser = TopoParser::new(tokens);
                match parser.parse() {
                    Ok(_) => println!("✓"),
                    Err(e) => {
                        println!("✗");
                        println!("    Parse error: {}", e);
                        errors += 1;
                    }
                }
            }
            Err(e) => {
                println!("✗");
                println!("    Lexer error: {}", e);
                errors += 1;
            }
        }
    }

    if errors == 0 {
        println!("✓ No errors found");
    } else {
        println!("✗ {} error(s) found", errors);
    }

    Ok(())
}

pub fn parse_file(file: &Path, json: bool) -> Result<()> {
    let source = fs::read_to_string(file)?;
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.tokenize()?;
    let mut parser = TopoParser::new(tokens);
    let program = parser.parse()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&program)?);
    } else {
        println!("{:#?}", program);
    }

    Ok(())
}

pub fn show_config() -> Result<()> {
    match Config::load_from_cwd() {
        Ok(config) => {
            println!("{}", serde_json::to_string_pretty(&config)?);
        }
        Err(e) => {
            println!("No config found: {}", e);
            println!();
            println!("Using default configuration:");
            println!("{}", serde_json::to_string_pretty(&Config::default())?);
        }
    }
    Ok(())
}

/// Format a TypeAnnotation for display
fn format_type_annotation(type_ann: &TypeAnnotation) -> String {
    match type_ann {
        TypeAnnotation::Primitive { name } => name.clone(),
        TypeAnnotation::Array { element_type } => {
            format!("{}[]", format_type_annotation(element_type))
        }
        TypeAnnotation::Optional { inner_type } => {
            format!("{}?", format_type_annotation(inner_type))
        }
        TypeAnnotation::Object { fields } => {
            let field_strs: Vec<String> = fields
                .iter()
                .map(|f| format!("{}: {}", f.name, format_type_annotation(&f.type_annotation)))
                .collect();
            format!("{{ {} }}", field_strs.join(", "))
        }
        TypeAnnotation::Union { types } => {
            let type_strs: Vec<String> = types.iter().map(format_type_annotation).collect();
            type_strs.join(" | ")
        }
        TypeAnnotation::Reference { name } => name.clone(),
    }
}

pub fn show_info_list(pages_only: bool, apis_only: bool) -> Result<()> {
    let config = Config::load_or_default();
    let paths_config = config.paths_config();

    // Auto-detect pages directory from common locations
    let pages_dir = {
        let config_path = PathBuf::from(&paths_config.pages);
        if config_path.exists() {
            config_path
        } else {
            let candidates = vec![
                PathBuf::from("pages"),
                PathBuf::from("src/pages"),
                PathBuf::from("demo/pages"),
                PathBuf::from("app/pages"),
            ];
            candidates
                .into_iter()
                .find(|p| p.exists())
                .unwrap_or_else(|| PathBuf::from("pages"))
        }
    };

    // Auto-detect services directory from common locations
    let services_dir = {
        let config_path = PathBuf::from(&paths_config.services);
        if config_path.exists() {
            config_path
        } else {
            let candidates = vec![
                PathBuf::from("services"),
                PathBuf::from("src/services"),
                PathBuf::from("demo/services"),
                PathBuf::from("app/services"),
            ];
            candidates
                .into_iter()
                .find(|p| p.exists())
                .unwrap_or_else(|| PathBuf::from("services"))
        }
    };

    // Auto-detect components directory from common locations
    let components_dir = {
        let config_path = PathBuf::from(&paths_config.components);
        if config_path.exists() {
            config_path
        } else {
            let candidates = vec![
                PathBuf::from("components"),
                PathBuf::from("src/components"),
                PathBuf::from("demo/components"),
                PathBuf::from("app/components"),
            ];
            candidates
                .into_iter()
                .find(|p| p.exists())
                .unwrap_or_else(|| PathBuf::from("components"))
        }
    };

    // Find all .tp files in pages directory
    let page_files = build::find_tp_files(&pages_dir)?;

    // Find all .tp files for API search (pages + services + components)
    let mut all_files = page_files.clone();
    if services_dir.exists() {
        all_files.extend(build::find_tp_files(&services_dir)?);
    }
    if components_dir.exists() {
        all_files.extend(build::find_tp_files(&components_dir)?);
    }

    let show_all = !pages_only && !apis_only;

    // Show pages
    if show_all || pages_only {
        println!("\n\x1b[1;36m📄 Pages\x1b[0m");
        println!("\x1b[90m{}\x1b[0m", "─".repeat(50));

        let routes = deploy::generate_routes(&page_files, &pages_dir)?;
        if routes.is_empty() {
            println!("  \x1b[90m(no pages found)\x1b[0m");
        } else {
            for (route, component) in &routes {
                let route_display = if route.contains('[') {
                    format!("\x1b[33m{}\x1b[0m", route) // Yellow for dynamic routes
                } else {
                    format!("\x1b[32m{}\x1b[0m", route) // Green for static routes
                };
                println!("  {} \x1b[90m→\x1b[0m {}", route_display, component);
            }
        }
        println!();
    }

    // Show APIs
    if show_all || apis_only {
        println!("\x1b[1;36m🔌 API Services\x1b[0m");
        println!("\x1b[90m{}\x1b[0m", "─".repeat(50));

        let mut api_services = Vec::new();

        // Parse all files and collect API services
        for file in &all_files {
            if let Ok(source) = fs::read_to_string(file) {
                let mut lexer = Lexer::new(&source);
                if let Ok(tokens) = lexer.tokenize() {
                    let mut parser = TopoParser::new(tokens);
                    if let Ok(program) = parser.parse() {
                        for decl in program.declarations {
                            if let Declaration::ApiService(api) = decl {
                                // Try to get relative path from various base directories
                                let rel_path = file
                                    .strip_prefix(&services_dir)
                                    .or_else(|_| file.strip_prefix(&pages_dir))
                                    .or_else(|_| file.strip_prefix(&components_dir))
                                    .unwrap_or(file)
                                    .to_string_lossy()
                                    .to_string();
                                api_services.push((api, rel_path));
                            }
                        }
                    }
                }
            }
        }

        if api_services.is_empty() {
            println!("  \x1b[90m(no API services found)\x1b[0m");
        } else {
            for (api, file_path) in &api_services {
                // Add "Api" suffix only if not already present
                let display_name = if api.name.ends_with("Api") {
                    api.name.clone()
                } else {
                    format!("{}Api", api.name)
                };
                println!(
                    "  \x1b[1;35m{}\x1b[0m \x1b[90m({})\x1b[0m",
                    display_name, file_path
                );

                // Show REST base URL if present
                if let Some(rest) = &api.rest {
                    println!("    \x1b[90mrest:\x1b[0m {}", rest);
                }

                // Show WebSocket/SSE subscription if present
                if let Some(subscribe) = &api.subscribe {
                    println!("    \x1b[90msubscribe:\x1b[0m {}", subscribe);
                }

                // Show endpoints
                if !api.endpoints.is_empty() {
                    println!("    \x1b[90mendpoints:\x1b[0m");
                    for endpoint in &api.endpoints {
                        let method_color = match endpoint.method {
                            topo::ast::HttpMethod::Get => "\x1b[32m",    // Green
                            topo::ast::HttpMethod::Post => "\x1b[33m",   // Yellow
                            topo::ast::HttpMethod::Put => "\x1b[34m",    // Blue
                            topo::ast::HttpMethod::Patch => "\x1b[35m",  // Magenta
                            topo::ast::HttpMethod::Delete => "\x1b[31m", // Red
                        };

                        // Build type signature
                        let mut type_parts = Vec::new();
                        if let Some(ref req_type) = endpoint.request_type {
                            type_parts.push(format!(
                                "\x1b[36m{}\x1b[0m",
                                format_type_annotation(req_type)
                            ));
                        }
                        let response_str = if let Some(ref res_type) = endpoint.response_type {
                            format!(
                                " \x1b[90m->\x1b[0m \x1b[32m{}\x1b[0m",
                                format_type_annotation(res_type)
                            )
                        } else {
                            String::new()
                        };
                        let error_str = if let Some(ref err_type) = endpoint.error_type {
                            format!(
                                " \x1b[90m|\x1b[0m \x1b[31m{}\x1b[0m",
                                format_type_annotation(err_type)
                            )
                        } else {
                            String::new()
                        };

                        let request_param = if type_parts.is_empty() {
                            String::new()
                        } else {
                            type_parts.join(", ")
                        };

                        println!(
                            "      {}{:6}\x1b[0m {} \x1b[90m→\x1b[0m {}({}){}{}",
                            method_color,
                            format!("{:?}", endpoint.method).to_uppercase(),
                            endpoint.path,
                            endpoint.name,
                            request_param,
                            response_str,
                            error_str
                        );
                    }
                }
                println!();
            }
        }
    }

    Ok(())
}
