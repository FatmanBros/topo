use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use tiny_http::{Response, Server};

use std::collections::HashMap;

use topo::ast::{Declaration, Program};
use topo::codegen::JsCodegen;
use topo::config::{Config, BuildMode};
use topo::lexer::Lexer;
use topo::parser::Parser as TopoParser;

#[derive(Parser)]
#[command(name = "topo")]
#[command(about = "A UI framework that eliminates nesting hell")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new topo project
    New {
        /// Project name
        name: String,
    },

    /// Initialize topo in current directory
    Init,

    /// Build the project
    Build {
        /// Input file or directory (overrides config)
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Output directory (overrides config)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Build mode: spa, ssg, ssr (overrides config)
        #[arg(short, long)]
        mode: Option<String>,
    },

    /// Build and start the server (alias: s)
    #[command(alias = "s")]
    Start {
        /// Port number (overrides config)
        #[arg(short, long)]
        port: Option<u16>,

        /// Don't open browser automatically
        #[arg(long)]
        no_open: bool,
    },

    /// Start development server with hot reload
    Dev {
        /// Port number (overrides config)
        #[arg(short, long)]
        port: Option<u16>,
    },

    /// Check for errors without building
    Check {
        /// Input file or directory
        #[arg(default_value = "src")]
        input: PathBuf,
    },

    /// Parse a file and output AST (for debugging)
    Parse {
        /// Input file
        file: PathBuf,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show current configuration
    Config,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name } => {
            create_project(&name)?;
        }
        Commands::Init => {
            init_project()?;
        }
        Commands::Build { input, output, mode } => {
            let config = Config::load_or_default();
            let build_config = config.build_config();
            let paths_config = config.paths_config();

            let input = input.unwrap_or_else(|| PathBuf::from(&paths_config.pages));
            let output = output.unwrap_or_else(|| PathBuf::from(&build_config.output));
            let mode = mode.unwrap_or_else(|| match build_config.mode {
                BuildMode::Spa => "spa".to_string(),
                BuildMode::Ssg => "ssg".to_string(),
                BuildMode::Ssr => "ssr".to_string(),
            });

            build_project(&input, &output, &mode)?;
        }
        Commands::Start { port, no_open } => {
            let config = Config::load_or_default();
            let build_config = config.build_config();
            let dev_config = config.dev_config();
            let paths_config = config.paths_config();

            let port = port.unwrap_or(dev_config.port);
            let input = PathBuf::from(&paths_config.pages);
            let output = PathBuf::from(&build_config.output);
            let mode = match build_config.mode {
                BuildMode::Spa => "spa".to_string(),
                BuildMode::Ssg => "ssg".to_string(),
                BuildMode::Ssr => "ssr".to_string(),
            };

            // Build first
            build_project(&input, &output, &mode)?;

            // Then start server
            start_server(port, &output, !no_open && dev_config.open)?;
        }
        Commands::Dev { port } => {
            let config = Config::load_or_default();
            let dev_config = config.dev_config();
            let port = port.unwrap_or(dev_config.port);

            start_dev_server(port, &config)?;
        }
        Commands::Check { input } => {
            check_project(&input)?;
        }
        Commands::Parse { file, json } => {
            parse_file(&file, json)?;
        }
        Commands::Config => {
            show_config()?;
        }
    }

    Ok(())
}

fn create_project(name: &str) -> Result<()> {
    println!("Creating new topo project: {}", name);

    // Create directory structure
    fs::create_dir_all(format!("{}/src/pages", name))?;
    fs::create_dir_all(format!("{}/src/components", name))?;
    fs::create_dir_all(format!("{}/src/stores", name))?;
    fs::create_dir_all(format!("{}/src/services", name))?;

    // Create topo.config.json
    let config = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/user/topo/main/topo.schema.json",
        "project": {
            "name": name,
            "version": "0.1.0"
        },
        "build": {
            "mode": "spa",
            "output": "dist",
            "minify": true
        },
        "dev": {
            "port": 3000,
            "open": true
        },
        "paths": {
            "pages": "src/pages",
            "components": "src/components",
            "stores": "src/stores",
            "services": "src/services"
        }
    });
    fs::write(
        format!("{}/topo.config.json", name),
        serde_json::to_string_pretty(&config)?,
    )?;

    // Create index.tp
    let index = r#"// Main page

Title -> {
    type: text
    content: "Welcome to topo!"
    style: "text-4xl font-bold text-center"
}

Subtitle -> {
    type: text
    content: "A UI framework that eliminates nesting hell"
    style: "text-lg text-gray-600 text-center mt-2"
}

App -> {
    style: "min-h-screen flex flex-col items-center justify-center"
    align: vertical
    children: [Title, Subtitle]
}
"#;
    fs::write(format!("{}/src/pages/index.tp", name), index)?;

    // Create .gitignore
    let gitignore = r#"# Build output
dist/
node_modules/

# IDE
.vscode/
.idea/

# OS
.DS_Store
"#;
    fs::write(format!("{}/.gitignore", name), gitignore)?;

    println!("✓ Project created successfully!");
    println!();
    println!("  cd {}", name);
    println!("  topo dev");

    Ok(())
}

fn init_project() -> Result<()> {
    println!("Initializing topo project in current directory...");

    // Check if config already exists
    if PathBuf::from("topo.config.json").exists() {
        println!("✗ topo.config.json already exists");
        return Ok(());
    }

    // Create directories if they don't exist
    fs::create_dir_all("src/pages")?;
    fs::create_dir_all("src/components")?;
    fs::create_dir_all("src/stores")?;
    fs::create_dir_all("src/services")?;

    // Get project name from current directory
    let name = std::env::current_dir()?
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "my-app".to_string());

    // Create topo.config.json
    let config = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/user/topo/main/topo.schema.json",
        "project": {
            "name": name,
            "version": "0.1.0"
        },
        "build": {
            "mode": "spa",
            "output": "dist"
        },
        "dev": {
            "port": 3000
        }
    });
    fs::write("topo.config.json", serde_json::to_string_pretty(&config)?)?;

    println!("✓ Created topo.config.json");
    println!("✓ Created src/pages/, src/components/, src/stores/, src/services/");

    Ok(())
}

fn build_project(input: &PathBuf, output: &PathBuf, mode: &str) -> Result<()> {
    println!("Building project...");
    println!("  Input: {:?}", input);
    println!("  Output: {:?}", output);
    println!("  Mode: {}", mode);

    // Create output directory
    fs::create_dir_all(output)?;

    // Find all .tp files or use single file
    let entry_files = find_tp_files(input)?;
    println!("  Found {} .tp files", entry_files.len());

    // Parse all files and resolve imports
    let mut parsed_files: HashMap<PathBuf, Program> = HashMap::new();
    let mut compile_order: Vec<PathBuf> = Vec::new();

    // Parse entry files and their dependencies
    for file in &entry_files {
        resolve_imports(file, input, &mut parsed_files, &mut compile_order)?;
    }

    println!("  Compiling {} files in dependency order", compile_order.len());

    // Generate code in dependency order
    let mut all_output = String::new();
    let mut codegen = JsCodegen::new();

    // Generate runtime once at the beginning
    all_output.push_str(&codegen.generate_runtime());

    // Generate file-based routes
    let routes = generate_routes(&entry_files, input)?;
    if !routes.is_empty() {
        all_output.push_str("\n// File-based routes\n");
        for (pattern, component) in &routes {
            all_output.push_str(&format!("registerRoute('{}', {});\n", pattern, component));
        }
        all_output.push_str("\n");
    }

    for file in &compile_order {
        println!("  Compiling: {:?}", file);
        if let Some(program) = parsed_files.get(file) {
            let js = codegen.generate(program);
            all_output.push_str(&js);
            all_output.push('\n');
        }
    }

    // Write output
    let output_file = output.join("app.js");
    fs::write(&output_file, &all_output)?;
    println!("✓ Build complete: {:?}", output_file);

    // Load config for HTML generation
    let config = Config::load_or_default();
    let html = generate_html(&config);
    fs::write(output.join("index.html"), html)?;

    Ok(())
}

/// Recursively resolve imports and build dependency order
fn resolve_imports(
    file: &PathBuf,
    base_dir: &PathBuf,
    parsed: &mut HashMap<PathBuf, Program>,
    order: &mut Vec<PathBuf>,
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
        let import_file = resolve_import_path(&import_path, file_dir, base_dir)?;
        resolve_imports(&import_file, base_dir, parsed, order)?;
    }

    // Add this file to the order (after its dependencies)
    if !order.contains(&file) {
        order.push(file);
    }

    Ok(())
}

/// Resolve an import path relative to the current file or base directory
fn resolve_import_path(import_path: &str, file_dir: &std::path::Path, base_dir: &PathBuf) -> Result<PathBuf> {
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

fn generate_html(config: &Config) -> String {
    let style_config = config.style.clone().unwrap_or_default();
    let tailwind_config = style_config.tailwind.unwrap_or_default();

    // Generate Tailwind script tag based on config
    let tailwind_script = if tailwind_config.enabled && tailwind_config.cdn {
        if let Some(custom_url) = &tailwind_config.cdn_url {
            format!("    <script src=\"{}\"></script>\n", custom_url)
        } else {
            // Use versioned CDN URL
            format!(
                "    <script src=\"https://cdn.tailwindcss.com/{}\"></script>\n",
                tailwind_config.version
            )
        }
    } else if tailwind_config.enabled {
        // Local Tailwind - user needs to set up their own build
        "    <!-- Tailwind CSS: Configure local build in tailwind.config.js -->\n    <link rel=\"stylesheet\" href=\"./styles.css\">\n".to_string()
    } else {
        String::new()
    };

    // Get project name
    let title = config
        .project
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "topo App".to_string());

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
{}</head>
<body>
    <div id="app"></div>
    <script type="module" src="./app.js"></script>
</body>
</html>
"#,
        title, tailwind_script
    )
}

fn start_server(port: u16, output_dir: &PathBuf, open_browser: bool) -> Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let server = Server::http(&addr).map_err(|e| anyhow::anyhow!("Failed to start server: {}", e))?;

    println!();
    println!("  Server running at:");
    println!("  Local:   http://localhost:{}", port);
    println!();
    println!("  Press Ctrl+C to stop");
    println!();

    // Open browser if configured
    if open_browser {
        let url = format!("http://localhost:{}", port);
        if let Err(e) = open_in_browser(&url) {
            eprintln!("  Warning: Could not open browser: {}", e);
        }
    }

    // Serve files
    for request in server.incoming_requests() {
        let url_path = request.url().trim_start_matches('/');
        let file_path = if url_path.is_empty() || url_path == "/" {
            output_dir.join("index.html")
        } else {
            output_dir.join(url_path)
        };

        let response = if file_path.exists() && file_path.is_file() {
            match fs::read(&file_path) {
                Ok(content) => {
                    let content_type = get_content_type(&file_path);
                    Response::from_data(content).with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()).unwrap()
                    )
                }
                Err(_) => Response::from_string("500 Internal Server Error")
                    .with_status_code(500),
            }
        } else {
            // For SPA, serve index.html for non-existent paths
            match fs::read(output_dir.join("index.html")) {
                Ok(content) => Response::from_data(content).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], b"text/html").unwrap()
                ),
                Err(_) => Response::from_string("404 Not Found").with_status_code(404),
            }
        };

        let _ = request.respond(response);
    }

    Ok(())
}

fn get_content_type(path: &PathBuf) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html",
        Some("js") => "application/javascript",
        Some("css") => "text/css",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    }
    .to_string()
}

fn open_in_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()?;
    }
    Ok(())
}

fn start_dev_server(port: u16, _config: &Config) -> Result<()> {
    println!("Starting development server...");
    println!("  Port: {}", port);
    println!();
    println!("  Local: http://localhost:{}", port);
    println!();
    println!("(Dev server with HMR not yet implemented - use 'topo start' for now)");
    Ok(())
}

fn check_project(input: &PathBuf) -> Result<()> {
    println!("Checking project...");

    let tp_files = find_tp_files(input)?;
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

fn parse_file(file: &PathBuf, json: bool) -> Result<()> {
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

fn show_config() -> Result<()> {
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

fn find_tp_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if dir.is_file() {
        if dir.extension().map_or(false, |ext| ext == "tp") {
            files.push(dir.clone());
        }
        return Ok(files);
    }

    if !dir.exists() {
        return Ok(files);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            files.extend(find_tp_files(&path)?);
        } else if path.extension().map_or(false, |ext| ext == "tp") {
            files.push(path);
        }
    }

    Ok(files)
}

/// Generate file-based routes from pages directory
/// pages/index.tp -> /
/// pages/about.tp -> /about
/// pages/users/index.tp -> /users
/// pages/users/[id].tp -> /users/[id]
fn generate_routes(files: &[PathBuf], base_dir: &PathBuf) -> Result<Vec<(String, String)>> {
    let mut routes = Vec::new();

    // Look for pages directory
    let pages_dir = if base_dir.join("pages").exists() {
        base_dir.join("pages")
    } else if base_dir.ends_with("pages") {
        base_dir.clone()
    } else {
        // No pages directory, no file-based routing
        return Ok(routes);
    };

    for file in files {
        // Only process files in pages directory
        if !file.starts_with(&pages_dir) {
            continue;
        }

        // Get relative path from pages directory
        let relative = file.strip_prefix(&pages_dir)?;
        let path_str = relative.to_string_lossy();

        // Convert file path to route pattern
        let route_pattern = file_path_to_route(&path_str);

        // Extract component name from file (assumes App or last defined component)
        // For simplicity, use the capitalized file name without extension
        let component_name = file
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| {
                if s == "index" {
                    // For index files, use parent dir name or "App"
                    relative
                        .parent()
                        .and_then(|p| p.file_name())
                        .and_then(|s| s.to_str())
                        .map(|s| capitalize(s))
                        .unwrap_or_else(|| "App".to_string())
                } else if s.starts_with('[') && s.ends_with(']') {
                    // Dynamic route like [id] -> use parent directory name + "Detail"
                    relative
                        .parent()
                        .and_then(|p| p.file_name())
                        .and_then(|s| s.to_str())
                        .map(|s| format!("{}Detail", capitalize(s)))
                        .unwrap_or_else(|| "Detail".to_string())
                } else {
                    capitalize(s)
                }
            })
            .unwrap_or_else(|| "App".to_string());

        // Look for component ending with "Page" or the capitalized filename
        let page_component = format!("{}Page", component_name);

        routes.push((route_pattern, page_component));
    }

    // Sort routes: specific routes before dynamic ones
    routes.sort_by(|a, b| {
        let a_dynamic = a.0.contains('[');
        let b_dynamic = b.0.contains('[');
        match (a_dynamic, b_dynamic) {
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            _ => a.0.cmp(&b.0),
        }
    });

    Ok(routes)
}

/// Convert file path to route pattern
/// index.tp -> /
/// about.tp -> /about
/// users/index.tp -> /users
/// users/[id].tp -> /users/[id]
/// [...slug].tp -> /[...slug]
fn file_path_to_route(path: &str) -> String {
    let path = path.trim_end_matches(".tp");

    // Handle index files
    let path = if path == "index" {
        "/"
    } else if path.ends_with("/index") {
        &path[..path.len() - 6]
    } else {
        path
    };

    // Ensure path starts with /
    if path.starts_with('/') {
        path.to_string()
    } else if path == "/" {
        "/".to_string()
    } else {
        format!("/{}", path)
    }
}

/// Capitalize first letter
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
