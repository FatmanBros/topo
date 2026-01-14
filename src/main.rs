use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use tiny_http::{Response, Server};
use std::sync::{Arc, Mutex};
use std::net::TcpListener;
use std::time::Duration;
use notify::{Watcher, RecursiveMode};
use tungstenite::{accept, Message};

use std::collections::HashMap;

use topo::ast::{Declaration, ObjectMember, Program, TypeAnnotation};
use topo::codegen::JsCodegen;
use topo::config::{Config, BuildMode, I18nConfig};
use topo::info_server::start_info_server;
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

    /// Create a new app from template
    #[command(name = "create-app")]
    CreateApp {
        /// Project name
        name: String,

        /// Template to use (starter, with-auth)
        #[arg(short, long, default_value = "starter")]
        template: String,

        /// List available templates
        #[arg(long)]
        list: bool,
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

        /// SSR target: cloudflare, rust (default: cloudflare)
        #[arg(short, long)]
        target: Option<String>,
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

    /// Run E2E tests with Playwright
    Test {
        /// Run tests in headed mode
        #[arg(long)]
        headed: bool,

        /// Open Playwright UI
        #[arg(long)]
        ui: bool,

        /// Specific test file to run
        file: Option<String>,
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

    /// Show project info (pages, APIs, and navigation graph)
    Info {
        #[command(subcommand)]
        command: Option<InfoCommands>,
    },
}

#[derive(Subcommand)]
enum InfoCommands {
    /// List pages and APIs in terminal
    List {
        /// Show only pages
        #[arg(long)]
        pages: bool,

        /// Show only APIs
        #[arg(long)]
        apis: bool,
    },
    /// Visualize page navigation graph in browser
    Web {
        /// Port number for the visualization server
        #[arg(short, long, default_value = "7091")]
        port: u16,

        /// Don't open browser automatically
        #[arg(long)]
        no_open: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name } => {
            create_project(&name)?;
        }
        Commands::CreateApp { name, template, list } => {
            if list {
                list_templates();
            } else {
                create_app(&name, &template)?;
            }
        }
        Commands::Init => {
            init_project()?;
        }
        Commands::Build { input, output, mode, target } => {
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
            let target = target.unwrap_or_else(|| "cloudflare".to_string());

            build_project(&input, &output, &mode, &target)?;
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
            let target = "cloudflare"; // Default target for start command

            // Build first
            build_project(&input, &output, &mode, target)?;

            // Get base_path from config
            let base_path = config
                .build
                .as_ref()
                .and_then(|b| b.base_path.clone())
                .unwrap_or_default();

            // Then start server
            start_server(port, &output, !no_open && dev_config.open, &base_path)?;
        }
        Commands::Dev { port } => {
            let config = Config::load_or_default();
            let dev_config = config.dev_config();
            let port = port.unwrap_or(dev_config.port);

            start_dev_server(port, &config)?;
        }
        Commands::Test { headed, ui, file } => {
            run_tests(headed, ui, file)?;
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
        Commands::Info { command } => {
            match command {
                Some(InfoCommands::List { pages, apis }) => {
                    show_info_list(pages, apis)?;
                }
                Some(InfoCommands::Web { port, no_open }) => {
                    start_info_server(port, no_open)?;
                }
                None => {
                    // Default: show web visualization
                    start_info_server(7091, false)?;
                }
            }
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

// =============================================================================
// Templates (embedded)
// =============================================================================

mod templates {
    // Starter template
    pub const STARTER_CONFIG: &str = include_str!("../templates/starter/topo.config.json");
    pub const STARTER_INDEX: &str = include_str!("../templates/starter/pages/index.tp");
    pub const STARTER_GITIGNORE: &str = include_str!("../templates/starter/.gitignore");

    // With-auth template
    pub const WITH_AUTH_CONFIG: &str = include_str!("../templates/with-auth/topo.config.json");
    pub const WITH_AUTH_INDEX: &str = include_str!("../templates/with-auth/pages/index.tp");
    pub const WITH_AUTH_LOGIN: &str = include_str!("../templates/with-auth/pages/login.tp");
    pub const WITH_AUTH_DASHBOARD: &str = include_str!("../templates/with-auth/pages/dashboard.tp");
    pub const WITH_AUTH_AUTH_STORE: &str = include_str!("../templates/with-auth/stores/auth.tp");
    pub const WITH_AUTH_GITIGNORE: &str = include_str!("../templates/with-auth/.gitignore");
}

fn list_templates() {
    println!("Available templates:");
    println!();
    println!("  starter     - Minimal starter template (default)");
    println!("  with-auth   - Template with login page and authentication");
    println!();
    println!("Usage: topo create-app my-app --template <template>");
}

fn create_app(name: &str, template: &str) -> Result<()> {
    match template {
        "starter" => create_starter_app(name),
        "with-auth" => create_with_auth_app(name),
        _ => {
            println!("✗ Unknown template: {}", template);
            println!();
            list_templates();
            Ok(())
        }
    }
}

fn create_starter_app(name: &str) -> Result<()> {
    println!("Creating new topo app: {} (template: starter)", name);

    // Create directory structure
    fs::create_dir_all(format!("{}/pages", name))?;
    fs::create_dir_all(format!("{}/components", name))?;

    // Write files with project name substitution
    let config = templates::STARTER_CONFIG.replace("{{PROJECT_NAME}}", name);
    fs::write(format!("{}/topo.config.json", name), config)?;
    fs::write(format!("{}/pages/index.tp", name), templates::STARTER_INDEX)?;
    fs::write(format!("{}/.gitignore", name), templates::STARTER_GITIGNORE)?;

    println!("✓ App created successfully!");
    println!();
    println!("  cd {}", name);
    println!("  topo dev");

    Ok(())
}

fn create_with_auth_app(name: &str) -> Result<()> {
    println!("Creating new topo app: {} (template: with-auth)", name);

    // Create directory structure
    fs::create_dir_all(format!("{}/pages", name))?;
    fs::create_dir_all(format!("{}/components", name))?;
    fs::create_dir_all(format!("{}/stores", name))?;

    // Write files with project name substitution
    let config = templates::WITH_AUTH_CONFIG.replace("{{PROJECT_NAME}}", name);
    fs::write(format!("{}/topo.config.json", name), config)?;
    fs::write(format!("{}/pages/index.tp", name), templates::WITH_AUTH_INDEX)?;
    fs::write(format!("{}/pages/login.tp", name), templates::WITH_AUTH_LOGIN)?;
    fs::write(format!("{}/pages/dashboard.tp", name), templates::WITH_AUTH_DASHBOARD)?;
    fs::write(format!("{}/stores/auth.tp", name), templates::WITH_AUTH_AUTH_STORE)?;
    fs::write(format!("{}/.gitignore", name), templates::WITH_AUTH_GITIGNORE)?;

    println!("✓ App created successfully!");
    println!();
    println!("  cd {}", name);
    println!("  topo dev");
    println!();
    println!("Demo credentials:");
    println!("  Email: demo@example.com");
    println!("  Password: password");

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

fn build_project(input: &PathBuf, output: &PathBuf, mode: &str, target: &str) -> Result<()> {
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
    // This enables cross-file param detection and store state access
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

    // Load http.setup.tp if exists (for HTTP client configuration)
    let http_setup_path = project_root.join("http.setup.tp");
    if http_setup_path.exists() {
        println!("  Loading http.setup.tp...");
        let setup_source = fs::read_to_string(&http_setup_path)?;
        all_output.push_str("\n// HTTP Setup\n");
        all_output.push_str(&setup_source);
        all_output.push('\n');
    }

    // Load routes.tp if exists (for type-safe route definitions)
    let routes_def_path = project_root.join("routes.tp");
    if routes_def_path.exists() {
        println!("  Loading routes.tp...");
        let routes_source = fs::read_to_string(&routes_def_path)?;
        let mut lexer = Lexer::new(&routes_source);
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

    // Generate file-based routes (registration happens after component definitions)
    let routes = generate_routes(&entry_files, input)?;

    // Track defined function names to avoid duplicates
    let mut defined_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut has_app = false;
    let mut entry_component: Option<String> = None;
    for file in &compile_order {
        println!("  Compiling: {:?}", file);
        if let Some(program) = parsed_files.get(file) {
            // Check if this file contains App/AppPage/Page component
            for decl in &program.declarations {
                if let Declaration::Component(comp) = decl {
                    if comp.name == "App" {
                        has_app = true;
                        entry_component = Some("App".to_string());
                    } else if comp.name == "AppPage" || comp.name == "Page" {
                        // Track entry component for SSG builds
                        if entry_component.is_none() {
                            entry_component = Some(comp.name.clone());
                        }
                    }
                }
            }
            let js = codegen.generate_with_file_path(program, file.to_str());
            // Deduplicate function names to avoid conflicts
            let js = deduplicate_functions(&js, &mut defined_names);
            all_output.push_str(&js);
            all_output.push('\n');
        }
    }

    // Register file-based routes (after all components are defined)
    if !routes.is_empty() {
        all_output.push_str("\n// File-based routes\n");
        for (pattern, component) in &routes {
            all_output.push_str(&format!("registerRoute('{}', {});\n", pattern, component));
            all_output.push_str(&format!("registerComponent('{}', {});\n", component, component));
        }
        all_output.push('\n');
    }

    // Add mount call at the end
    // When routes exist, let router handle it; otherwise use App/entry component
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

    // Minify JS for production (ssg mode)
    let final_js = if mode == "ssg" {
        minify_js(&all_output)
    } else {
        all_output
    };

    // Write output
    let output_file = output.join("app.js");
    fs::write(&output_file, &final_js)?;
    println!("✓ Build complete: {:?}", output_file);

    // Generate HTML (use SSG version for production)
    let html = if mode == "ssg" {
        generate_html_ssg(&config, &final_js)
    } else {
        generate_html(&config)
    };
    fs::write(output.join("index.html"), &html)?;

    // SSG mode: generate HTML files for each static route
    if mode == "ssg" {
        // Generate 404.html for dynamic routes fallback
        fs::write(output.join("404.html"), &html)?;
        println!("  Generated: 404.html");

        // Generate HTML for each static route (excluding dynamic routes with [param])
        for (route_pattern, _component) in &routes {
            // Skip dynamic routes (contain [param])
            if route_pattern.contains('[') {
                continue;
            }
            // Skip root route (already have index.html)
            if route_pattern == "/" {
                continue;
            }

            // Create directory structure for the route
            // e.g., /about -> output/about/index.html
            let route_path = route_pattern.trim_start_matches('/');
            let route_dir = output.join(route_path);
            fs::create_dir_all(&route_dir)?;
            fs::write(route_dir.join("index.html"), &html)?;
            println!("  Generated: {}/index.html", route_path);
        }
    }

    // Copy public folder contents to output
    let public_dir = project_root.join("public");
    if public_dir.exists() && public_dir.is_dir() {
        copy_dir_contents(&public_dir, output)?;
    }

    // SSR mode: generate server-side rendering code
    if mode == "ssr" {
        generate_ssr_output(output, &routes, &config, target)?;
    }

    Ok(())
}

/// Copy all files from source directory to destination
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
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

/// Build project for development mode (with hot reload script)
fn build_project_dev(input: &PathBuf, output: &PathBuf, _mode: &str, ws_port: u16, config: &Config) -> Result<()> {
    // Create output directory
    fs::create_dir_all(output)?;

    // Find all .tp files or use single file
    let entry_files = find_tp_files(input)?;

    // Project root is where topo.config.json is located
    let project_root = find_project_root(input)?;

    // Load config from project root for aliases (may differ from passed config)
    let project_config = Config::load(&project_root.join("topo.config.json")).unwrap_or_default();
    let paths_config = project_config.paths_config();
    let aliases = paths_config.aliases;

    // Parse all files and resolve imports
    let mut parsed_files: HashMap<PathBuf, Program> = HashMap::new();
    let mut compile_order: Vec<PathBuf> = Vec::new();

    // Parse entry files and their dependencies
    for file in &entry_files {
        resolve_imports(file, input, &project_root, &mut parsed_files, &mut compile_order, &aliases)?;
    }

    // Generate code in dependency order
    let mut all_output = String::new();
    let mut codegen = JsCodegen::new();

    // First pass: collect all component params and store state fields from all files
    // This enables cross-file param detection and store state access
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

    // Load http.setup.tp if exists (for HTTP client configuration)
    let http_setup_path = project_root.join("http.setup.tp");
    if http_setup_path.exists() {
        let setup_source = fs::read_to_string(&http_setup_path)?;
        all_output.push_str("\n// HTTP Setup\n");
        all_output.push_str(&setup_source);
        all_output.push('\n');
    }

    // Generate file-based routes (registration happens after component definitions)
    let routes = generate_routes(&entry_files, input)?;

    // Track defined function names to avoid duplicates
    let mut defined_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut has_app = false;
    for file in &compile_order {
        if let Some(program) = parsed_files.get(file) {
            // Check if this file contains App component
            for decl in &program.declarations {
                if let Declaration::Component(comp) = decl {
                    if comp.name == "App" {
                        has_app = true;
                    }
                }
            }
            let js = codegen.generate_with_file_path(program, file.to_str());
            // Deduplicate function names to avoid conflicts
            let js = deduplicate_functions(&js, &mut defined_names);
            all_output.push_str(&js);
            all_output.push('\n');
        }
    }

    // Register file-based routes (after all components are defined)
    if !routes.is_empty() {
        all_output.push_str("\n// File-based routes\n");
        for (pattern, component) in &routes {
            all_output.push_str(&format!("registerRoute('{}', {});\n", pattern, component));
            all_output.push_str(&format!("registerComponent('{}', {});\n", component, component));
        }
        all_output.push('\n');
    }

    // Add mount call at the end
    if has_app {
        all_output.push_str("// Mount app\n");
        all_output.push_str("mount(App, '#app');\n");
    } else if !routes.is_empty() {
        all_output.push_str("// Mount with router\n");
        all_output.push_str("mount(null, '#app');\n");
    }

    // Write output
    let output_file = output.join("app.js");
    fs::write(&output_file, &all_output)?;

    // Generate HTML with hot reload script
    let html = generate_html_dev(config, ws_port + 1);
    fs::write(output.join("index.html"), html)?;

    // Copy public folder contents to output
    let public_dir = project_root.join("public");
    if public_dir.exists() && public_dir.is_dir() {
        copy_dir_contents(&public_dir, output)?;
    }

    Ok(())
}

/// Recursively resolve imports and build dependency order
fn resolve_imports(
    file: &PathBuf,
    base_dir: &PathBuf,
    project_root: &PathBuf,
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
fn resolve_import_path(
    import_path: &str,
    file_dir: &std::path::Path,
    base_dir: &PathBuf,
    project_root: &Path,
    aliases: &HashMap<String, String>,
) -> Result<PathBuf> {
    // Check for alias prefix (e.g., "@/", "~/", etc.)
    for (alias, target) in aliases {
        let alias_prefix = format!("{}/", alias);
        if import_path.starts_with(&alias_prefix) {
            let alias_path = &import_path[alias_prefix.len()..];
            // Resolve target relative to project root (where topo.config.json is)
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

    let title_script = format!("    <script>window.__TOPO_DEFAULT_TITLE = '{}';</script>\n", title);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link rel="icon" type="image/x-icon" href="/favicon.ico">
    <link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">
    <link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">
    <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png">
    <title>{}</title>
{}{}</head>
<body>
    <div id="app"></div>
    <script type="module" src="/app.js"></script>
    <script>
    // Error Overlay for development
    (function() {{
      const overlay = document.createElement('div');
      overlay.id = 'topo-error-overlay';
      overlay.style.cssText = 'display:none;position:fixed;inset:0;background:rgba(0,0,0,0.85);z-index:99999;padding:32px;overflow:auto;font-family:ui-monospace,monospace';

      function showError(title, message, stack) {{
        overlay.innerHTML = `
          <div style="max-width:900px;margin:0 auto;background:#1a1a1a;border-radius:12px;border:1px solid #333;overflow:hidden">
            <div style="background:#dc2626;color:white;padding:16px 20px;display:flex;justify-content:space-between;align-items:center">
              <span style="font-weight:600;font-size:16px">${{title}}</span>
              <div>
                <button id="topo-copy-btn" style="background:#fff2;border:none;color:white;padding:8px 16px;border-radius:6px;cursor:pointer;margin-right:8px;font-size:13px">Copy</button>
                <button id="topo-close-btn" style="background:#fff2;border:none;color:white;padding:8px 16px;border-radius:6px;cursor:pointer;font-size:13px">✕</button>
              </div>
            </div>
            <div style="padding:20px">
              <div style="color:#f87171;font-size:18px;font-weight:500;margin-bottom:16px;word-break:break-word">${{message}}</div>
              ${{stack ? `<pre style="color:#a1a1aa;font-size:13px;line-height:1.6;margin:0;white-space:pre-wrap;word-break:break-word">${{stack}}</pre>` : ''}}
            </div>
          </div>
        `;
        overlay.style.display = 'block';
        document.getElementById('topo-close-btn').onclick = () => overlay.style.display = 'none';
        document.getElementById('topo-copy-btn').onclick = () => {{
          navigator.clipboard.writeText(message + (stack ? '\\n\\n' + stack : ''));
          document.getElementById('topo-copy-btn').textContent = 'Copied!';
          setTimeout(() => document.getElementById('topo-copy-btn').textContent = 'Copy', 2000);
        }};
      }}

      document.body.appendChild(overlay);

      window.onerror = (msg, src, line, col, err) => {{
        const loc = src ? `${{src}}:${{line}}:${{col}}` : '';
        showError('Runtime Error', msg, err?.stack || loc);
        return false;
      }};

      window.onunhandledrejection = (e) => {{
        showError('Unhandled Promise Rejection', e.reason?.message || String(e.reason), e.reason?.stack);
      }};
    }})();
    </script>
</body>
</html>
"#,
        title, tailwind_script, title_script
    )
}

/// Deduplicate function and const names in JS output
/// Takes a chunk of JS code and a set of already-defined names
/// Returns the modified JS and updates the defined_names set
fn deduplicate_functions(js: &str, defined_names: &mut std::collections::HashSet<String>) -> String {
    use regex::Regex;

    // Find all function declarations: "function Name(" or "function Name ("
    let func_regex = Regex::new(r"function\s+([A-Z][a-zA-Z0-9_]*)\s*\(").unwrap();
    // Find all const declarations: "const Name =" or "const Name="
    let const_regex = Regex::new(r"const\s+([A-Z][a-zA-Z0-9_]*)\s*=").unwrap();

    // First pass: find all names defined in this chunk
    let mut local_functions: Vec<String> = Vec::new();
    for cap in func_regex.captures_iter(js) {
        if let Some(name_match) = cap.get(1) {
            let name = name_match.as_str().to_string();
            if !local_functions.contains(&name) {
                local_functions.push(name);
            }
        }
    }
    for cap in const_regex.captures_iter(js) {
        if let Some(name_match) = cap.get(1) {
            let name = name_match.as_str().to_string();
            if !local_functions.contains(&name) {
                local_functions.push(name);
            }
        }
    }

    // Build rename map for duplicates
    let mut rename_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
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
        let decl_regex = Regex::new(&decl_pattern).unwrap();
        result = decl_regex.replace_all(&result, format!("function {}(", new_name)).to_string();

        // Replace const declaration: "const OldName =" -> "const NewName ="
        let const_pattern = format!(r"const\s+{}\s*=", regex::escape(old_name));
        let const_regex = Regex::new(&const_pattern).unwrap();
        result = const_regex.replace_all(&result, format!("const {} =", new_name)).to_string();

        // Replace references using word boundaries
        // \b matches word boundary, so we match Name followed by certain characters
        // This pattern: word boundary + Name + (optional whitespace + one of the expected chars)
        let ref_pattern = format!(r"\b{}\b", regex::escape(old_name));
        let ref_regex = Regex::new(&ref_pattern).unwrap();
        result = ref_regex.replace_all(&result, new_name.as_str()).to_string();
    }

    result
}

/// Simple JS minification - removes line comments at start of lines and collapses whitespace
/// Does NOT try to parse strings (too complex with template literals)
fn minify_js(js: &str) -> String {
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

/// Extract Tailwind classes from JS and generate minimal CSS
// Note: This function is kept for potential future use but SSG now uses Tailwind CLI
#[allow(dead_code)]
fn extract_tailwind_css(js: &str) -> String {
    use std::collections::HashSet;
    use regex::Regex;

    let mut classes: HashSet<&str> = HashSet::new();

    // Method 1: Find class strings in style/class attributes
    let class_regex = Regex::new(r#"(?:class|className|style)\s*[:=]\s*["'`]([^"'`]+)["'`]"#).unwrap();
    for cap in class_regex.captures_iter(js) {
        if let Some(class_str) = cap.get(1) {
            for class in class_str.as_str().split_whitespace() {
                classes.insert(class);
            }
        }
    }

    // Method 2: Find ALL quoted strings that look like Tailwind classes
    // This catches dynamic values like bgColor: 'bg-pink-50'
    // Find single-quoted strings using string splitting (more reliable than regex)
    let mut in_quote = false;
    for part in js.split('\'') {
        if in_quote {
            let content = part;
            if content.contains("bg-") || content.contains("text-") ||
               content.contains("border-") || content.contains("w-") ||
               content.contains("h-") || content.contains("blur-") ||
               content.contains("mix-blend-") || content.contains("-top-") ||
               content.contains("-right-") || content.contains("-bottom-") ||
               content.contains("-left-") || content.contains("filter") ||
               content.contains("opacity-") || content.contains("rounded-") {
                for class in content.split_whitespace() {
                    classes.insert(class);
                }
            }
        }
        in_quote = !in_quote;
    }

    // Also check double-quoted strings
    let all_strings_regex = Regex::new(r#""([^"]+)""#).unwrap();
    for cap in all_strings_regex.captures_iter(js) {
        if let Some(str_content) = cap.get(1) {
            let content = str_content.as_str();
            if content.contains("bg-") || content.contains("text-") ||
               content.contains("border-") || content.contains("rounded-") ||
               content.contains("flex") || content.contains("grid") ||
               content.contains("px-") || content.contains("py-") ||
               content.contains("p-") || content.contains("m-") ||
               content.contains("mx-") || content.contains("my-") ||
               content.contains("mt-") || content.contains("mb-") ||
               content.contains("w-") || content.contains("h-") ||
               content.contains("gap-") || content.contains("space-") ||
               content.contains("font-") || content.contains("shadow") ||
               content.contains("hover:") || content.contains("focus:") ||
               content.contains("sm:") || content.contains("md:") ||
               content.contains("lg:") || content.contains("xl:") ||
               content.contains("items-") || content.contains("justify-") ||
               content.contains("opacity-") || content.contains("z-") ||
               content.contains("transition") || content.contains("duration-") ||
               content.contains("overflow-") || content.contains("cursor-") ||
               content.contains("-top-") || content.contains("-right-") ||
               content.contains("-bottom-") || content.contains("-left-") ||
               content.contains("filter") || content.contains("blur-") ||
               content.contains("mix-blend-") || content.contains("backdrop-") ||
               content.contains("from-") || content.contains("to-") ||
               content.contains("via-") || content.contains("translate-") {
                for class in content.split_whitespace() {
                    classes.insert(class);
                }
            }
        }
    }

    // Generate minimal Tailwind CSS for extracted classes
    generate_tailwind_css_for_classes(&classes)
}

/// Generate CSS for specific Tailwind classes
#[allow(dead_code)]
fn generate_tailwind_css_for_classes(classes: &std::collections::HashSet<&str>) -> String {
    let mut css = String::new();

    // CSS Reset
    css.push_str("*,::after,::before{box-sizing:border-box;border:0 solid #e5e7eb}html{line-height:1.5;-webkit-text-size-adjust:100%;font-family:ui-sans-serif,system-ui,sans-serif}body{margin:0;line-height:inherit}");

    // Common Tailwind utilities
    let utilities: Vec<(&str, &str)> = vec![
        // Display
        ("flex", ".flex{display:flex}"),
        ("inline-flex", ".inline-flex{display:inline-flex}"),
        ("grid", ".grid{display:grid}"),
        ("hidden", ".hidden{display:none}"),
        ("block", ".block{display:block}"),
        ("inline-block", ".inline-block{display:inline-block}"),
        ("inline", ".inline{display:inline}"),
        // Flex
        ("flex-col", ".flex-col{flex-direction:column}"),
        ("flex-row", ".flex-row{flex-direction:row}"),
        ("flex-wrap", ".flex-wrap{flex-wrap:wrap}"),
        ("flex-1", ".flex-1{flex:1 1 0%}"),
        ("flex-auto", ".flex-auto{flex:1 1 auto}"),
        ("flex-none", ".flex-none{flex:none}"),
        ("grow", ".grow{flex-grow:1}"),
        ("shrink-0", ".shrink-0{flex-shrink:0}"),
        // Justify/Align
        ("justify-start", ".justify-start{justify-content:flex-start}"),
        ("justify-end", ".justify-end{justify-content:flex-end}"),
        ("justify-center", ".justify-center{justify-content:center}"),
        ("justify-between", ".justify-between{justify-content:space-between}"),
        ("justify-around", ".justify-around{justify-content:space-around}"),
        ("items-start", ".items-start{align-items:flex-start}"),
        ("items-end", ".items-end{align-items:flex-end}"),
        ("items-center", ".items-center{align-items:center}"),
        ("items-baseline", ".items-baseline{align-items:baseline}"),
        ("items-stretch", ".items-stretch{align-items:stretch}"),
        ("self-center", ".self-center{align-self:center}"),
        // Gap
        ("gap-0", ".gap-0{gap:0}"),
        ("gap-1", ".gap-1{gap:0.25rem}"),
        ("gap-2", ".gap-2{gap:0.5rem}"),
        ("gap-3", ".gap-3{gap:0.75rem}"),
        ("gap-4", ".gap-4{gap:1rem}"),
        ("gap-5", ".gap-5{gap:1.25rem}"),
        ("gap-6", ".gap-6{gap:1.5rem}"),
        ("gap-8", ".gap-8{gap:2rem}"),
        ("gap-10", ".gap-10{gap:2.5rem}"),
        ("gap-12", ".gap-12{gap:3rem}"),
        // Width
        ("w-full", ".w-full{width:100%}"),
        ("w-auto", ".w-auto{width:auto}"),
        ("w-screen", ".w-screen{width:100vw}"),
        ("w-fit", ".w-fit{width:fit-content}"),
        ("w-max", ".w-max{width:max-content}"),
        ("w-min", ".w-min{width:min-content}"),
        // Max-width
        ("max-w-sm", ".max-w-sm{max-width:24rem}"),
        ("max-w-md", ".max-w-md{max-width:28rem}"),
        ("max-w-lg", ".max-w-lg{max-width:32rem}"),
        ("max-w-xl", ".max-w-xl{max-width:36rem}"),
        ("max-w-2xl", ".max-w-2xl{max-width:42rem}"),
        ("max-w-3xl", ".max-w-3xl{max-width:48rem}"),
        ("max-w-4xl", ".max-w-4xl{max-width:56rem}"),
        ("max-w-5xl", ".max-w-5xl{max-width:64rem}"),
        ("max-w-6xl", ".max-w-6xl{max-width:72rem}"),
        ("max-w-7xl", ".max-w-7xl{max-width:80rem}"),
        ("max-w-full", ".max-w-full{max-width:100%}"),
        ("max-w-screen-sm", ".max-w-screen-sm{max-width:640px}"),
        ("max-w-screen-md", ".max-w-screen-md{max-width:768px}"),
        ("max-w-screen-lg", ".max-w-screen-lg{max-width:1024px}"),
        ("max-w-screen-xl", ".max-w-screen-xl{max-width:1280px}"),
        // Height
        ("h-full", ".h-full{height:100%}"),
        ("h-auto", ".h-auto{height:auto}"),
        ("h-screen", ".h-screen{height:100vh}"),
        ("h-fit", ".h-fit{height:fit-content}"),
        ("min-h-screen", ".min-h-screen{min-height:100vh}"),
        ("min-h-full", ".min-h-full{min-height:100%}"),
        // Padding
        ("p-0", ".p-0{padding:0}"),
        ("p-1", ".p-1{padding:0.25rem}"),
        ("p-2", ".p-2{padding:0.5rem}"),
        ("p-3", ".p-3{padding:0.75rem}"),
        ("p-4", ".p-4{padding:1rem}"),
        ("p-5", ".p-5{padding:1.25rem}"),
        ("p-6", ".p-6{padding:1.5rem}"),
        ("p-8", ".p-8{padding:2rem}"),
        ("p-10", ".p-10{padding:2.5rem}"),
        ("p-12", ".p-12{padding:3rem}"),
        ("p-16", ".p-16{padding:4rem}"),
        ("p-20", ".p-20{padding:5rem}"),
        ("px-0", ".px-0{padding-left:0;padding-right:0}"),
        ("px-1", ".px-1{padding-left:0.25rem;padding-right:0.25rem}"),
        ("px-2", ".px-2{padding-left:0.5rem;padding-right:0.5rem}"),
        ("px-3", ".px-3{padding-left:0.75rem;padding-right:0.75rem}"),
        ("px-4", ".px-4{padding-left:1rem;padding-right:1rem}"),
        ("px-5", ".px-5{padding-left:1.25rem;padding-right:1.25rem}"),
        ("px-6", ".px-6{padding-left:1.5rem;padding-right:1.5rem}"),
        ("px-8", ".px-8{padding-left:2rem;padding-right:2rem}"),
        ("px-10", ".px-10{padding-left:2.5rem;padding-right:2.5rem}"),
        ("px-12", ".px-12{padding-left:3rem;padding-right:3rem}"),
        ("py-0", ".py-0{padding-top:0;padding-bottom:0}"),
        ("py-1", ".py-1{padding-top:0.25rem;padding-bottom:0.25rem}"),
        ("py-2", ".py-2{padding-top:0.5rem;padding-bottom:0.5rem}"),
        ("py-3", ".py-3{padding-top:0.75rem;padding-bottom:0.75rem}"),
        ("py-4", ".py-4{padding-top:1rem;padding-bottom:1rem}"),
        ("py-5", ".py-5{padding-top:1.25rem;padding-bottom:1.25rem}"),
        ("py-6", ".py-6{padding-top:1.5rem;padding-bottom:1.5rem}"),
        ("py-8", ".py-8{padding-top:2rem;padding-bottom:2rem}"),
        ("py-10", ".py-10{padding-top:2.5rem;padding-bottom:2.5rem}"),
        ("py-12", ".py-12{padding-top:3rem;padding-bottom:3rem}"),
        ("py-16", ".py-16{padding-top:4rem;padding-bottom:4rem}"),
        ("py-20", ".py-20{padding-top:5rem;padding-bottom:5rem}"),
        ("pt-0", ".pt-0{padding-top:0}"),
        ("pt-4", ".pt-4{padding-top:1rem}"),
        ("pt-8", ".pt-8{padding-top:2rem}"),
        ("pt-16", ".pt-16{padding-top:4rem}"),
        ("pt-20", ".pt-20{padding-top:5rem}"),
        ("pb-0", ".pb-0{padding-bottom:0}"),
        ("pb-4", ".pb-4{padding-bottom:1rem}"),
        ("pb-8", ".pb-8{padding-bottom:2rem}"),
        ("pb-16", ".pb-16{padding-bottom:4rem}"),
        ("pb-20", ".pb-20{padding-bottom:5rem}"),
        ("pl-4", ".pl-4{padding-left:1rem}"),
        ("pr-4", ".pr-4{padding-right:1rem}"),
        // Margin
        ("m-0", ".m-0{margin:0}"),
        ("m-auto", ".m-auto{margin:auto}"),
        ("m-1", ".m-1{margin:0.25rem}"),
        ("m-2", ".m-2{margin:0.5rem}"),
        ("m-4", ".m-4{margin:1rem}"),
        ("mx-auto", ".mx-auto{margin-left:auto;margin-right:auto}"),
        ("mx-0", ".mx-0{margin-left:0;margin-right:0}"),
        ("mx-4", ".mx-4{margin-left:1rem;margin-right:1rem}"),
        ("my-0", ".my-0{margin-top:0;margin-bottom:0}"),
        ("my-2", ".my-2{margin-top:0.5rem;margin-bottom:0.5rem}"),
        ("my-4", ".my-4{margin-top:1rem;margin-bottom:1rem}"),
        ("my-8", ".my-8{margin-top:2rem;margin-bottom:2rem}"),
        ("mt-0", ".mt-0{margin-top:0}"),
        ("mt-1", ".mt-1{margin-top:0.25rem}"),
        ("mt-2", ".mt-2{margin-top:0.5rem}"),
        ("mt-4", ".mt-4{margin-top:1rem}"),
        ("mt-6", ".mt-6{margin-top:1.5rem}"),
        ("mt-8", ".mt-8{margin-top:2rem}"),
        ("mt-10", ".mt-10{margin-top:2.5rem}"),
        ("mt-12", ".mt-12{margin-top:3rem}"),
        ("mt-16", ".mt-16{margin-top:4rem}"),
        ("mt-20", ".mt-20{margin-top:5rem}"),
        ("mb-0", ".mb-0{margin-bottom:0}"),
        ("mb-1", ".mb-1{margin-bottom:0.25rem}"),
        ("mb-2", ".mb-2{margin-bottom:0.5rem}"),
        ("mb-4", ".mb-4{margin-bottom:1rem}"),
        ("mb-6", ".mb-6{margin-bottom:1.5rem}"),
        ("mb-8", ".mb-8{margin-bottom:2rem}"),
        ("mb-10", ".mb-10{margin-bottom:2.5rem}"),
        ("mb-12", ".mb-12{margin-bottom:3rem}"),
        ("mb-16", ".mb-16{margin-bottom:4rem}"),
        ("ml-0", ".ml-0{margin-left:0}"),
        ("ml-2", ".ml-2{margin-left:0.5rem}"),
        ("ml-4", ".ml-4{margin-left:1rem}"),
        ("ml-auto", ".ml-auto{margin-left:auto}"),
        ("mr-0", ".mr-0{margin-right:0}"),
        ("mr-2", ".mr-2{margin-right:0.5rem}"),
        ("mr-4", ".mr-4{margin-right:1rem}"),
        ("mr-auto", ".mr-auto{margin-right:auto}"),
        // Text
        ("text-xs", ".text-xs{font-size:0.75rem;line-height:1rem}"),
        ("text-sm", ".text-sm{font-size:0.875rem;line-height:1.25rem}"),
        ("text-base", ".text-base{font-size:1rem;line-height:1.5rem}"),
        ("text-lg", ".text-lg{font-size:1.125rem;line-height:1.75rem}"),
        ("text-xl", ".text-xl{font-size:1.25rem;line-height:1.75rem}"),
        ("text-2xl", ".text-2xl{font-size:1.5rem;line-height:2rem}"),
        ("text-3xl", ".text-3xl{font-size:1.875rem;line-height:2.25rem}"),
        ("text-4xl", ".text-4xl{font-size:2.25rem;line-height:2.5rem}"),
        ("text-5xl", ".text-5xl{font-size:3rem;line-height:1}"),
        ("text-6xl", ".text-6xl{font-size:3.75rem;line-height:1}"),
        ("text-left", ".text-left{text-align:left}"),
        ("text-center", ".text-center{text-align:center}"),
        ("text-right", ".text-right{text-align:right}"),
        ("text-justify", ".text-justify{text-align:justify}"),
        // Font
        ("font-thin", ".font-thin{font-weight:100}"),
        ("font-light", ".font-light{font-weight:300}"),
        ("font-normal", ".font-normal{font-weight:400}"),
        ("font-medium", ".font-medium{font-weight:500}"),
        ("font-semibold", ".font-semibold{font-weight:600}"),
        ("font-bold", ".font-bold{font-weight:700}"),
        ("font-extrabold", ".font-extrabold{font-weight:800}"),
        ("font-mono", ".font-mono{font-family:ui-monospace,SFMono-Regular,monospace}"),
        ("italic", ".italic{font-style:italic}"),
        ("underline", ".underline{text-decoration-line:underline}"),
        ("line-through", ".line-through{text-decoration-line:line-through}"),
        ("no-underline", ".no-underline{text-decoration-line:none}"),
        ("leading-none", ".leading-none{line-height:1}"),
        ("leading-tight", ".leading-tight{line-height:1.25}"),
        ("leading-snug", ".leading-snug{line-height:1.375}"),
        ("leading-normal", ".leading-normal{line-height:1.5}"),
        ("leading-relaxed", ".leading-relaxed{line-height:1.625}"),
        ("leading-loose", ".leading-loose{line-height:2}"),
        ("tracking-tight", ".tracking-tight{letter-spacing:-0.025em}"),
        ("tracking-normal", ".tracking-normal{letter-spacing:0}"),
        ("tracking-wide", ".tracking-wide{letter-spacing:0.025em}"),
        ("tracking-wider", ".tracking-wider{letter-spacing:0.05em}"),
        ("tracking-widest", ".tracking-widest{letter-spacing:0.1em}"),
        ("uppercase", ".uppercase{text-transform:uppercase}"),
        ("lowercase", ".lowercase{text-transform:lowercase}"),
        ("capitalize", ".capitalize{text-transform:capitalize}"),
        ("truncate", ".truncate{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}"),
        ("whitespace-nowrap", ".whitespace-nowrap{white-space:nowrap}"),
        ("whitespace-pre", ".whitespace-pre{white-space:pre}"),
        ("whitespace-pre-wrap", ".whitespace-pre-wrap{white-space:pre-wrap}"),
        // Colors
        ("text-white", ".text-white{color:#fff}"),
        ("text-black", ".text-black{color:#000}"),
        ("text-transparent", ".text-transparent{color:transparent}"),
        ("text-gray-50", ".text-gray-50{color:#f9fafb}"),
        ("text-gray-100", ".text-gray-100{color:#f3f4f6}"),
        ("text-gray-200", ".text-gray-200{color:#e5e7eb}"),
        ("text-gray-300", ".text-gray-300{color:#d1d5db}"),
        ("text-gray-400", ".text-gray-400{color:#9ca3af}"),
        ("text-gray-500", ".text-gray-500{color:#6b7280}"),
        ("text-gray-600", ".text-gray-600{color:#4b5563}"),
        ("text-gray-700", ".text-gray-700{color:#374151}"),
        ("text-gray-800", ".text-gray-800{color:#1f2937}"),
        ("text-gray-900", ".text-gray-900{color:#111827}"),
        ("text-slate-50", ".text-slate-50{color:#f8fafc}"),
        ("text-slate-100", ".text-slate-100{color:#f1f5f9}"),
        ("text-slate-200", ".text-slate-200{color:#e2e8f0}"),
        ("text-slate-300", ".text-slate-300{color:#cbd5e1}"),
        ("text-slate-400", ".text-slate-400{color:#94a3b8}"),
        ("text-slate-500", ".text-slate-500{color:#64748b}"),
        ("text-slate-600", ".text-slate-600{color:#475569}"),
        ("text-slate-700", ".text-slate-700{color:#334155}"),
        ("text-slate-800", ".text-slate-800{color:#1e293b}"),
        ("text-slate-900", ".text-slate-900{color:#0f172a}"),
        ("text-indigo-400", ".text-indigo-400{color:#818cf8}"),
        ("text-indigo-500", ".text-indigo-500{color:#6366f1}"),
        ("text-indigo-600", ".text-indigo-600{color:#4f46e5}"),
        ("text-purple-400", ".text-purple-400{color:#c084fc}"),
        ("text-purple-500", ".text-purple-500{color:#a855f7}"),
        ("text-purple-600", ".text-purple-600{color:#9333ea}"),
        ("text-pink-400", ".text-pink-400{color:#f472b6}"),
        ("text-pink-500", ".text-pink-500{color:#ec4899}"),
        ("text-blue-400", ".text-blue-400{color:#60a5fa}"),
        ("text-blue-500", ".text-blue-500{color:#3b82f6}"),
        ("text-blue-600", ".text-blue-600{color:#2563eb}"),
        ("text-green-400", ".text-green-400{color:#4ade80}"),
        ("text-green-500", ".text-green-500{color:#22c55e}"),
        ("text-green-600", ".text-green-600{color:#16a34a}"),
        ("text-red-400", ".text-red-400{color:#f87171}"),
        ("text-red-500", ".text-red-500{color:#ef4444}"),
        ("text-red-600", ".text-red-600{color:#dc2626}"),
        ("text-yellow-400", ".text-yellow-400{color:#facc15}"),
        ("text-yellow-500", ".text-yellow-500{color:#eab308}"),
        ("text-orange-400", ".text-orange-400{color:#fb923c}"),
        ("text-orange-500", ".text-orange-500{color:#f97316}"),
        // Background
        ("bg-white", ".bg-white{background-color:#fff}"),
        ("bg-black", ".bg-black{background-color:#000}"),
        ("bg-transparent", ".bg-transparent{background-color:transparent}"),
        ("bg-gray-50", ".bg-gray-50{background-color:#f9fafb}"),
        ("bg-gray-100", ".bg-gray-100{background-color:#f3f4f6}"),
        ("bg-gray-200", ".bg-gray-200{background-color:#e5e7eb}"),
        ("bg-gray-300", ".bg-gray-300{background-color:#d1d5db}"),
        ("bg-gray-400", ".bg-gray-400{background-color:#9ca3af}"),
        ("bg-gray-500", ".bg-gray-500{background-color:#6b7280}"),
        ("bg-gray-600", ".bg-gray-600{background-color:#4b5563}"),
        ("bg-gray-700", ".bg-gray-700{background-color:#374151}"),
        ("bg-gray-800", ".bg-gray-800{background-color:#1f2937}"),
        ("bg-gray-900", ".bg-gray-900{background-color:#111827}"),
        ("bg-slate-50", ".bg-slate-50{background-color:#f8fafc}"),
        ("bg-slate-100", ".bg-slate-100{background-color:#f1f5f9}"),
        ("bg-slate-200", ".bg-slate-200{background-color:#e2e8f0}"),
        ("bg-slate-700", ".bg-slate-700{background-color:#334155}"),
        ("bg-slate-800", ".bg-slate-800{background-color:#1e293b}"),
        ("bg-slate-900", ".bg-slate-900{background-color:#0f172a}"),
        ("bg-indigo-50", ".bg-indigo-50{background-color:#eef2ff}"),
        ("bg-indigo-100", ".bg-indigo-100{background-color:#e0e7ff}"),
        ("bg-indigo-500", ".bg-indigo-500{background-color:#6366f1}"),
        ("bg-indigo-600", ".bg-indigo-600{background-color:#4f46e5}"),
        ("bg-indigo-700", ".bg-indigo-700{background-color:#4338ca}"),
        ("bg-purple-50", ".bg-purple-50{background-color:#faf5ff}"),
        ("bg-purple-100", ".bg-purple-100{background-color:#f3e8ff}"),
        ("bg-purple-500", ".bg-purple-500{background-color:#a855f7}"),
        ("bg-purple-600", ".bg-purple-600{background-color:#9333ea}"),
        ("bg-pink-50", ".bg-pink-50{background-color:#fdf2f8}"),
        ("bg-pink-100", ".bg-pink-100{background-color:#fce7f3}"),
        ("bg-blue-50", ".bg-blue-50{background-color:#eff6ff}"),
        ("bg-blue-100", ".bg-blue-100{background-color:#dbeafe}"),
        ("bg-blue-500", ".bg-blue-500{background-color:#3b82f6}"),
        ("bg-blue-600", ".bg-blue-600{background-color:#2563eb}"),
        ("bg-green-50", ".bg-green-50{background-color:#f0fdf4}"),
        ("bg-green-100", ".bg-green-100{background-color:#dcfce7}"),
        ("bg-green-500", ".bg-green-500{background-color:#22c55e}"),
        ("bg-red-50", ".bg-red-50{background-color:#fef2f2}"),
        ("bg-red-100", ".bg-red-100{background-color:#fee2e2}"),
        ("bg-yellow-50", ".bg-yellow-50{background-color:#fefce8}"),
        ("bg-yellow-100", ".bg-yellow-100{background-color:#fef9c3}"),
        // Gradient
        ("bg-gradient-to-r", ".bg-gradient-to-r{background-image:linear-gradient(to right,var(--tw-gradient-stops))}"),
        ("bg-gradient-to-l", ".bg-gradient-to-l{background-image:linear-gradient(to left,var(--tw-gradient-stops))}"),
        ("bg-gradient-to-t", ".bg-gradient-to-t{background-image:linear-gradient(to top,var(--tw-gradient-stops))}"),
        ("bg-gradient-to-b", ".bg-gradient-to-b{background-image:linear-gradient(to bottom,var(--tw-gradient-stops))}"),
        ("bg-gradient-to-br", ".bg-gradient-to-br{background-image:linear-gradient(to bottom right,var(--tw-gradient-stops))}"),
        ("bg-gradient-to-tr", ".bg-gradient-to-tr{background-image:linear-gradient(to top right,var(--tw-gradient-stops))}"),
        ("from-indigo-500", ".from-indigo-500{--tw-gradient-from:#6366f1;--tw-gradient-stops:var(--tw-gradient-from),var(--tw-gradient-to,rgba(99,102,241,0))}"),
        ("from-indigo-600", ".from-indigo-600{--tw-gradient-from:#4f46e5;--tw-gradient-stops:var(--tw-gradient-from),var(--tw-gradient-to,rgba(79,70,229,0))}"),
        ("from-purple-500", ".from-purple-500{--tw-gradient-from:#a855f7;--tw-gradient-stops:var(--tw-gradient-from),var(--tw-gradient-to,rgba(168,85,247,0))}"),
        ("from-purple-600", ".from-purple-600{--tw-gradient-from:#9333ea;--tw-gradient-stops:var(--tw-gradient-from),var(--tw-gradient-to,rgba(147,51,234,0))}"),
        ("from-pink-500", ".from-pink-500{--tw-gradient-from:#ec4899;--tw-gradient-stops:var(--tw-gradient-from),var(--tw-gradient-to,rgba(236,72,153,0))}"),
        ("from-blue-500", ".from-blue-500{--tw-gradient-from:#3b82f6;--tw-gradient-stops:var(--tw-gradient-from),var(--tw-gradient-to,rgba(59,130,246,0))}"),
        ("to-indigo-500", ".to-indigo-500{--tw-gradient-to:#6366f1}"),
        ("to-purple-500", ".to-purple-500{--tw-gradient-to:#a855f7}"),
        ("to-purple-600", ".to-purple-600{--tw-gradient-to:#9333ea}"),
        ("to-pink-500", ".to-pink-500{--tw-gradient-to:#ec4899}"),
        ("to-pink-600", ".to-pink-600{--tw-gradient-to:#db2777}"),
        ("to-blue-500", ".to-blue-500{--tw-gradient-to:#3b82f6}"),
        ("via-purple-500", ".via-purple-500{--tw-gradient-stops:var(--tw-gradient-from),#a855f7,var(--tw-gradient-to,rgba(168,85,247,0))}"),
        // Border
        ("border", ".border{border-width:1px}"),
        ("border-0", ".border-0{border-width:0}"),
        ("border-2", ".border-2{border-width:2px}"),
        ("border-4", ".border-4{border-width:4px}"),
        ("border-t", ".border-t{border-top-width:1px}"),
        ("border-b", ".border-b{border-bottom-width:1px}"),
        ("border-l", ".border-l{border-left-width:1px}"),
        ("border-r", ".border-r{border-right-width:1px}"),
        ("border-solid", ".border-solid{border-style:solid}"),
        ("border-dashed", ".border-dashed{border-style:dashed}"),
        ("border-none", ".border-none{border-style:none}"),
        ("border-transparent", ".border-transparent{border-color:transparent}"),
        ("border-white", ".border-white{border-color:#fff}"),
        ("border-gray-100", ".border-gray-100{border-color:#f3f4f6}"),
        ("border-gray-200", ".border-gray-200{border-color:#e5e7eb}"),
        ("border-gray-300", ".border-gray-300{border-color:#d1d5db}"),
        ("border-slate-200", ".border-slate-200{border-color:#e2e8f0}"),
        ("border-slate-700", ".border-slate-700{border-color:#334155}"),
        ("border-indigo-500", ".border-indigo-500{border-color:#6366f1}"),
        // Rounded
        ("rounded", ".rounded{border-radius:0.25rem}"),
        ("rounded-sm", ".rounded-sm{border-radius:0.125rem}"),
        ("rounded-md", ".rounded-md{border-radius:0.375rem}"),
        ("rounded-lg", ".rounded-lg{border-radius:0.5rem}"),
        ("rounded-xl", ".rounded-xl{border-radius:0.75rem}"),
        ("rounded-2xl", ".rounded-2xl{border-radius:1rem}"),
        ("rounded-3xl", ".rounded-3xl{border-radius:1.5rem}"),
        ("rounded-full", ".rounded-full{border-radius:9999px}"),
        ("rounded-none", ".rounded-none{border-radius:0}"),
        ("rounded-t", ".rounded-t{border-top-left-radius:0.25rem;border-top-right-radius:0.25rem}"),
        ("rounded-t-lg", ".rounded-t-lg{border-top-left-radius:0.5rem;border-top-right-radius:0.5rem}"),
        ("rounded-b", ".rounded-b{border-bottom-left-radius:0.25rem;border-bottom-right-radius:0.25rem}"),
        ("rounded-b-lg", ".rounded-b-lg{border-bottom-left-radius:0.5rem;border-bottom-right-radius:0.5rem}"),
        // Shadow
        ("shadow", ".shadow{box-shadow:0 1px 3px 0 rgba(0,0,0,.1),0 1px 2px -1px rgba(0,0,0,.1)}"),
        ("shadow-sm", ".shadow-sm{box-shadow:0 1px 2px 0 rgba(0,0,0,.05)}"),
        ("shadow-md", ".shadow-md{box-shadow:0 4px 6px -1px rgba(0,0,0,.1),0 2px 4px -2px rgba(0,0,0,.1)}"),
        ("shadow-lg", ".shadow-lg{box-shadow:0 10px 15px -3px rgba(0,0,0,.1),0 4px 6px -4px rgba(0,0,0,.1)}"),
        ("shadow-xl", ".shadow-xl{box-shadow:0 20px 25px -5px rgba(0,0,0,.1),0 8px 10px -6px rgba(0,0,0,.1)}"),
        ("shadow-2xl", ".shadow-2xl{box-shadow:0 25px 50px -12px rgba(0,0,0,.25)}"),
        ("shadow-none", ".shadow-none{box-shadow:none}"),
        // Opacity
        ("opacity-0", ".opacity-0{opacity:0}"),
        ("opacity-25", ".opacity-25{opacity:0.25}"),
        ("opacity-50", ".opacity-50{opacity:0.5}"),
        ("opacity-75", ".opacity-75{opacity:0.75}"),
        ("opacity-100", ".opacity-100{opacity:1}"),
        // Position
        ("relative", ".relative{position:relative}"),
        ("absolute", ".absolute{position:absolute}"),
        ("fixed", ".fixed{position:fixed}"),
        ("sticky", ".sticky{position:sticky}"),
        ("static", ".static{position:static}"),
        ("inset-0", ".inset-0{inset:0}"),
        ("top-0", ".top-0{top:0}"),
        ("right-0", ".right-0{right:0}"),
        ("bottom-0", ".bottom-0{bottom:0}"),
        ("left-0", ".left-0{left:0}"),
        ("top-1/2", ".top-1\\/2{top:50%}"),
        ("left-1/2", ".left-1\\/2{left:50%}"),
        // Z-index
        ("z-0", ".z-0{z-index:0}"),
        ("z-10", ".z-10{z-index:10}"),
        ("z-20", ".z-20{z-index:20}"),
        ("z-30", ".z-30{z-index:30}"),
        ("z-40", ".z-40{z-index:40}"),
        ("z-50", ".z-50{z-index:50}"),
        // Overflow
        ("overflow-auto", ".overflow-auto{overflow:auto}"),
        ("overflow-hidden", ".overflow-hidden{overflow:hidden}"),
        ("overflow-scroll", ".overflow-scroll{overflow:scroll}"),
        ("overflow-visible", ".overflow-visible{overflow:visible}"),
        ("overflow-x-auto", ".overflow-x-auto{overflow-x:auto}"),
        ("overflow-y-auto", ".overflow-y-auto{overflow-y:auto}"),
        // Cursor
        ("cursor-pointer", ".cursor-pointer{cursor:pointer}"),
        ("cursor-default", ".cursor-default{cursor:default}"),
        ("cursor-not-allowed", ".cursor-not-allowed{cursor:not-allowed}"),
        // Pointer events
        ("pointer-events-none", ".pointer-events-none{pointer-events:none}"),
        ("pointer-events-auto", ".pointer-events-auto{pointer-events:auto}"),
        // User select
        ("select-none", ".select-none{user-select:none}"),
        ("select-text", ".select-text{user-select:text}"),
        ("select-all", ".select-all{user-select:all}"),
        // Transform
        ("transform", ".transform{transform:translateX(var(--tw-translate-x,0)) translateY(var(--tw-translate-y,0)) rotate(var(--tw-rotate,0)) skewX(var(--tw-skew-x,0)) skewY(var(--tw-skew-y,0)) scaleX(var(--tw-scale-x,1)) scaleY(var(--tw-scale-y,1))}"),
        ("-translate-x-1/2", ".-translate-x-1\\/2{--tw-translate-x:-50%}"),
        ("-translate-y-1/2", ".-translate-y-1\\/2{--tw-translate-y:-50%}"),
        ("translate-x-0", ".translate-x-0{--tw-translate-x:0}"),
        ("translate-y-0", ".translate-y-0{--tw-translate-y:0}"),
        ("scale-100", ".scale-100{--tw-scale-x:1;--tw-scale-y:1}"),
        ("scale-105", ".scale-105{--tw-scale-x:1.05;--tw-scale-y:1.05}"),
        ("scale-110", ".scale-110{--tw-scale-x:1.1;--tw-scale-y:1.1}"),
        ("rotate-0", ".rotate-0{--tw-rotate:0deg}"),
        ("rotate-45", ".rotate-45{--tw-rotate:45deg}"),
        ("rotate-90", ".rotate-90{--tw-rotate:90deg}"),
        ("rotate-180", ".rotate-180{--tw-rotate:180deg}"),
        // Transition
        ("transition", ".transition{transition-property:color,background-color,border-color,text-decoration-color,fill,stroke,opacity,box-shadow,transform,filter,backdrop-filter;transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}"),
        ("transition-all", ".transition-all{transition-property:all;transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}"),
        ("transition-colors", ".transition-colors{transition-property:color,background-color,border-color,text-decoration-color,fill,stroke;transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}"),
        ("transition-opacity", ".transition-opacity{transition-property:opacity;transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}"),
        ("transition-transform", ".transition-transform{transition-property:transform;transition-timing-function:cubic-bezier(.4,0,.2,1);transition-duration:.15s}"),
        ("duration-150", ".duration-150{transition-duration:.15s}"),
        ("duration-200", ".duration-200{transition-duration:.2s}"),
        ("duration-300", ".duration-300{transition-duration:.3s}"),
        ("duration-500", ".duration-500{transition-duration:.5s}"),
        ("ease-in", ".ease-in{transition-timing-function:cubic-bezier(.4,0,1,1)}"),
        ("ease-out", ".ease-out{transition-timing-function:cubic-bezier(0,0,.2,1)}"),
        ("ease-in-out", ".ease-in-out{transition-timing-function:cubic-bezier(.4,0,.2,1)}"),
        // Animation
        ("animate-spin", "@keyframes spin{to{transform:rotate(360deg)}}.animate-spin{animation:spin 1s linear infinite}"),
        ("animate-pulse", "@keyframes pulse{50%{opacity:.5}}.animate-pulse{animation:pulse 2s cubic-bezier(.4,0,.6,1) infinite}"),
        ("animate-bounce", "@keyframes bounce{0%,100%{transform:translateY(-25%);animation-timing-function:cubic-bezier(.8,0,1,1)}50%{transform:none;animation-timing-function:cubic-bezier(0,0,.2,1)}}.animate-bounce{animation:bounce 1s infinite}"),
        // Object
        ("object-cover", ".object-cover{object-fit:cover}"),
        ("object-contain", ".object-contain{object-fit:contain}"),
        ("object-fill", ".object-fill{object-fit:fill}"),
        ("object-center", ".object-center{object-position:center}"),
        // Aspect ratio
        ("aspect-video", ".aspect-video{aspect-ratio:16/9}"),
        ("aspect-square", ".aspect-square{aspect-ratio:1/1}"),
        // Ring
        ("ring-1", ".ring-1{box-shadow:0 0 0 1px var(--tw-ring-color,rgba(59,130,246,.5))}"),
        ("ring-2", ".ring-2{box-shadow:0 0 0 2px var(--tw-ring-color,rgba(59,130,246,.5))}"),
        ("ring-indigo-500", ".ring-indigo-500{--tw-ring-color:#6366f1}"),
        ("ring-offset-2", ".ring-offset-2{--tw-ring-offset-width:2px}"),
        // Focus
        ("focus:outline-none", ".focus\\:outline-none:focus{outline:2px solid transparent;outline-offset:2px}"),
        ("focus:ring-2", ".focus\\:ring-2:focus{box-shadow:0 0 0 2px var(--tw-ring-color,rgba(59,130,246,.5))}"),
        ("outline-none", ".outline-none{outline:2px solid transparent;outline-offset:2px}"),
        // Hover states (approximation - needs JS for full support in SSG)
        ("hover:bg-gray-100", ".hover\\:bg-gray-100:hover{background-color:#f3f4f6}"),
        ("hover:bg-gray-50", ".hover\\:bg-gray-50:hover{background-color:#f9fafb}"),
        ("hover:bg-indigo-700", ".hover\\:bg-indigo-700:hover{background-color:#4338ca}"),
        ("hover:bg-indigo-600", ".hover\\:bg-indigo-600:hover{background-color:#4f46e5}"),
        ("hover:text-gray-900", ".hover\\:text-gray-900:hover{color:#111827}"),
        ("hover:text-indigo-600", ".hover\\:text-indigo-600:hover{color:#4f46e5}"),
        ("hover:underline", ".hover\\:underline:hover{text-decoration-line:underline}"),
        ("hover:opacity-80", ".hover\\:opacity-80:hover{opacity:.8}"),
        ("hover:scale-105", ".hover\\:scale-105:hover{--tw-scale-x:1.05;--tw-scale-y:1.05;transform:scale(1.05)}"),
        ("hover:shadow-lg", ".hover\\:shadow-lg:hover{box-shadow:0 10px 15px -3px rgba(0,0,0,.1),0 4px 6px -4px rgba(0,0,0,.1)}"),
        ("hover:shadow-xl", ".hover\\:shadow-xl:hover{box-shadow:0 20px 25px -5px rgba(0,0,0,.1),0 8px 10px -6px rgba(0,0,0,.1)}"),
        // Space
        ("space-x-1", ".space-x-1>:not([hidden])~:not([hidden]){margin-left:0.25rem}"),
        ("space-x-2", ".space-x-2>:not([hidden])~:not([hidden]){margin-left:0.5rem}"),
        ("space-x-3", ".space-x-3>:not([hidden])~:not([hidden]){margin-left:0.75rem}"),
        ("space-x-4", ".space-x-4>:not([hidden])~:not([hidden]){margin-left:1rem}"),
        ("space-x-6", ".space-x-6>:not([hidden])~:not([hidden]){margin-left:1.5rem}"),
        ("space-x-8", ".space-x-8>:not([hidden])~:not([hidden]){margin-left:2rem}"),
        ("space-y-1", ".space-y-1>:not([hidden])~:not([hidden]){margin-top:0.25rem}"),
        ("space-y-2", ".space-y-2>:not([hidden])~:not([hidden]){margin-top:0.5rem}"),
        ("space-y-3", ".space-y-3>:not([hidden])~:not([hidden]){margin-top:0.75rem}"),
        ("space-y-4", ".space-y-4>:not([hidden])~:not([hidden]){margin-top:1rem}"),
        ("space-y-6", ".space-y-6>:not([hidden])~:not([hidden]){margin-top:1.5rem}"),
        ("space-y-8", ".space-y-8>:not([hidden])~:not([hidden]){margin-top:2rem}"),
        // Grid
        ("grid-cols-1", ".grid-cols-1{grid-template-columns:repeat(1,minmax(0,1fr))}"),
        ("grid-cols-2", ".grid-cols-2{grid-template-columns:repeat(2,minmax(0,1fr))}"),
        ("grid-cols-3", ".grid-cols-3{grid-template-columns:repeat(3,minmax(0,1fr))}"),
        ("grid-cols-4", ".grid-cols-4{grid-template-columns:repeat(4,minmax(0,1fr))}"),
        ("grid-cols-5", ".grid-cols-5{grid-template-columns:repeat(5,minmax(0,1fr))}"),
        ("grid-cols-6", ".grid-cols-6{grid-template-columns:repeat(6,minmax(0,1fr))}"),
        ("grid-cols-12", ".grid-cols-12{grid-template-columns:repeat(12,minmax(0,1fr))}"),
        ("col-span-1", ".col-span-1{grid-column:span 1/span 1}"),
        ("col-span-2", ".col-span-2{grid-column:span 2/span 2}"),
        ("col-span-3", ".col-span-3{grid-column:span 3/span 3}"),
        ("col-span-4", ".col-span-4{grid-column:span 4/span 4}"),
        ("col-span-6", ".col-span-6{grid-column:span 6/span 6}"),
        ("col-span-full", ".col-span-full{grid-column:1/-1}"),
        // Backdrop
        ("backdrop-blur", ".backdrop-blur{backdrop-filter:blur(8px)}"),
        ("backdrop-blur-sm", ".backdrop-blur-sm{backdrop-filter:blur(4px)}"),
        ("backdrop-blur-md", ".backdrop-blur-md{backdrop-filter:blur(12px)}"),
        ("backdrop-blur-lg", ".backdrop-blur-lg{backdrop-filter:blur(16px)}"),
        // List
        ("list-none", ".list-none{list-style-type:none}"),
        ("list-disc", ".list-disc{list-style-type:disc}"),
        ("list-decimal", ".list-decimal{list-style-type:decimal}"),
        // SVG
        ("fill-current", ".fill-current{fill:currentColor}"),
        ("stroke-current", ".stroke-current{stroke:currentColor}"),
        // sr-only
        ("sr-only", ".sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border-width:0}"),
        // Violet colors
        ("bg-violet-50", ".bg-violet-50{background-color:#f5f3ff}"),
        ("bg-violet-100", ".bg-violet-100{background-color:#ede9fe}"),
        ("bg-violet-200", ".bg-violet-200{background-color:#ddd6fe}"),
        ("bg-violet-500", ".bg-violet-500{background-color:#8b5cf6}"),
        ("bg-violet-600", ".bg-violet-600{background-color:#7c3aed}"),
        ("text-violet-500", ".text-violet-500{color:#8b5cf6}"),
        ("text-violet-700", ".text-violet-700{color:#6d28d9}"),
        ("border-violet-200", ".border-violet-200{border-color:#ddd6fe}"),
        ("from-violet-50", ".from-violet-50{--tw-gradient-from:#f5f3ff;--tw-gradient-stops:var(--tw-gradient-from),var(--tw-gradient-to,rgba(245,243,255,0))}"),
        ("from-violet-500", ".from-violet-500{--tw-gradient-from:#8b5cf6;--tw-gradient-stops:var(--tw-gradient-from),var(--tw-gradient-to,rgba(139,92,246,0))}"),
        ("to-violet-100", ".to-violet-100{--tw-gradient-to:#ede9fe}"),
        ("to-violet-500", ".to-violet-500{--tw-gradient-to:#8b5cf6}"),
        ("to-violet-600", ".to-violet-600{--tw-gradient-to:#7c3aed}"),
        ("via-violet-50", ".via-violet-50{--tw-gradient-stops:var(--tw-gradient-from),#f5f3ff,var(--tw-gradient-to,rgba(245,243,255,0))}"),
        // Sky colors
        ("bg-sky-50", ".bg-sky-50{background-color:#f0f9ff}"),
        ("bg-sky-100", ".bg-sky-100{background-color:#e0f2fe}"),
        ("bg-sky-200", ".bg-sky-200{background-color:#bae6fd}"),
        ("bg-sky-500", ".bg-sky-500{background-color:#0ea5e9}"),
        ("text-sky-700", ".text-sky-700{color:#0369a1}"),
        ("border-sky-200", ".border-sky-200{border-color:#bae6fd}"),
        ("to-sky-50", ".to-sky-50{--tw-gradient-to:#f0f9ff}"),
        ("to-sky-500", ".to-sky-500{--tw-gradient-to:#0ea5e9}"),
        // Amber colors
        ("bg-amber-50", ".bg-amber-50{background-color:#fffbeb}"),
        ("bg-amber-100", ".bg-amber-100{background-color:#fef3c7}"),
        ("text-amber-700", ".text-amber-700{color:#b45309}"),
        ("border-amber-200", ".border-amber-200{border-color:#fde68a}"),
        ("to-amber-100", ".to-amber-100{--tw-gradient-to:#fef3c7}"),
        // Orange colors
        ("from-orange-50", ".from-orange-50{--tw-gradient-from:#fff7ed;--tw-gradient-stops:var(--tw-gradient-from),var(--tw-gradient-to,rgba(255,247,237,0))}"),
        // Pink colors (additional)
        ("bg-pink-200", ".bg-pink-200{background-color:#fbcfe8}"),
        ("text-pink-700", ".text-pink-700{color:#be185d}"),
        ("from-pink-600", ".from-pink-600{--tw-gradient-from:#db2777;--tw-gradient-stops:var(--tw-gradient-from),var(--tw-gradient-to,rgba(219,39,119,0))}"),
        // Width/Height specific
        ("w-10", ".w-10{width:2.5rem}"),
        ("w-80", ".w-80{width:20rem}"),
        ("h-10", ".h-10{height:2.5rem}"),
        ("h-80", ".h-80{height:20rem}"),
        // Negative positioning
        ("-top-40", ".-top-40{top:-10rem}"),
        ("-right-40", ".-right-40{right:-10rem}"),
        ("-bottom-40", ".-bottom-40{bottom:-10rem}"),
        ("-left-40", ".-left-40{left:-10rem}"),
        // Filters
        ("filter", ".filter{filter:var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,)}"),
        ("blur-3xl", ".blur-3xl{--tw-blur:blur(64px);filter:var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,)}"),
        ("mix-blend-multiply", ".mix-blend-multiply{mix-blend-mode:multiply}"),
        // Background clip
        ("bg-clip-text", ".bg-clip-text{-webkit-background-clip:text;background-clip:text}"),
        // Opacity modifier backgrounds
        ("bg-white/80", ".bg-white\\/80{background-color:rgba(255,255,255,0.8)}"),
        // Padding additional
        ("py-0.5", ".py-0\\.5{padding-top:0.125rem;padding-bottom:0.125rem}"),
        ("py-1.5", ".py-1\\.5{padding-top:0.375rem;padding-bottom:0.375rem}"),
        ("px-0.5", ".px-0\\.5{padding-left:0.125rem;padding-right:0.125rem}"),
        ("pt-32", ".pt-32{padding-top:8rem}"),
        ("pb-24", ".pb-24{padding-bottom:6rem}"),
        // More hover states
        ("hover:from-pink-600", ".hover\\:from-pink-600:hover{--tw-gradient-from:#db2777;--tw-gradient-stops:var(--tw-gradient-from),var(--tw-gradient-to,rgba(219,39,119,0))}"),
        ("hover:to-violet-600", ".hover\\:to-violet-600:hover{--tw-gradient-to:#7c3aed}"),
        ("hover:-translate-y-0.5", ".hover\\:-translate-y-0\\.5:hover{--tw-translate-y:-0.125rem;transform:translateX(var(--tw-translate-x,0)) translateY(-0.125rem)}"),
        // Flex shrink
        ("flex-shrink-0", ".flex-shrink-0{flex-shrink:0}"),
    ];

    // Responsive prefixes
    let breakpoints = [
        ("sm:", "@media(min-width:640px){", "}"),
        ("md:", "@media(min-width:768px){", "}"),
        ("lg:", "@media(min-width:1024px){", "}"),
        ("xl:", "@media(min-width:1280px){", "}"),
        ("2xl:", "@media(min-width:1536px){", "}"),
    ];

    // Add utilities for matching classes
    for (name, style) in &utilities {
        if classes.contains(name) {
            css.push_str(style);
        }
    }

    // Add responsive variants
    for (prefix, media_start, media_end) in &breakpoints {
        let mut responsive_css = String::new();
        for (name, style) in &utilities {
            let responsive_class = format!("{}{}", prefix, name);
            if classes.contains(responsive_class.as_str()) {
                // Convert .class to .prefix\\:class
                let responsive_style = style.replace(
                    &format!(".{}", name),
                    &format!(".{}\\:{}", prefix.trim_end_matches(':'), name)
                );
                responsive_css.push_str(&responsive_style);
            }
        }
        if !responsive_css.is_empty() {
            css.push_str(media_start);
            css.push_str(&responsive_css);
            css.push_str(media_end);
        }
    }

    css
}

/// Generate HTML for SSG (production) - relative paths, inlined CSS, no dev features
fn generate_html_ssg(config: &Config, _js: &str) -> String {
    let title = config
        .project
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "topo App".to_string());

    // Get basePath from build config
    let base_path = config
        .build
        .as_ref()
        .and_then(|b| b.base_path.clone())
        .unwrap_or_default();

    let base_path_script = if !base_path.is_empty() {
        format!("window.__TOPO_BASE_PATH = '{}';", base_path)
    } else {
        String::new()
    };

    let title_script = format!("window.__TOPO_DEFAULT_TITLE = '{}';", title);

    let config_script = if base_path_script.is_empty() {
        format!("    <script>{}</script>\n", title_script)
    } else {
        format!("    <script>{} {}</script>\n", base_path_script, title_script)
    };

    // Asset prefix: use basePath for absolute paths (required for SPA 404 fallback)
    let asset_prefix = if base_path.is_empty() {
        String::from("/")
    } else {
        format!("{}/", base_path)
    };

    // SSG uses external CSS generated by Tailwind CLI
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link rel="icon" type="image/x-icon" href="{asset_prefix}favicon.ico">
    <link rel="icon" type="image/png" sizes="32x32" href="{asset_prefix}favicon-32x32.png">
    <link rel="icon" type="image/png" sizes="16x16" href="{asset_prefix}favicon-16x16.png">
    <link rel="apple-touch-icon" sizes="180x180" href="{asset_prefix}apple-touch-icon.png">
    <title>{title}</title>
    <link rel="stylesheet" href="{asset_prefix}styles.css">
</head>
<body>
    <div id="app"></div>
{config_script}    <script type="module" src="{asset_prefix}app.js"></script>
</body>
</html>
"#,
        asset_prefix = asset_prefix,
        title = title,
        config_script = config_script
    )
}

fn generate_html_dev(config: &Config, ws_port: u16) -> String {
    let style_config = config.style.clone().unwrap_or_default();
    let tailwind_config = style_config.tailwind.unwrap_or_default();

    // Generate Tailwind script tag based on config
    let tailwind_script = if tailwind_config.enabled && tailwind_config.cdn {
        if let Some(custom_url) = &tailwind_config.cdn_url {
            format!("    <script src=\"{}\"></script>\n", custom_url)
        } else {
            format!(
                "    <script src=\"https://cdn.tailwindcss.com/{}\"></script>\n",
                tailwind_config.version
            )
        }
    } else if tailwind_config.enabled {
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

    let title_script = format!("    <script>window.__TOPO_DEFAULT_TITLE = '{}';</script>\n", title);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link rel="icon" type="image/x-icon" href="/favicon.ico">
    <link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">
    <link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">
    <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png">
    <title>{}</title>
{}{}</head>
<body>
    <div id="app"></div>
    <script type="module" src="/app.js"></script>
    <script>
    // Hot Reload WebSocket
    (function() {{
      let connected = false;
      const ws = new WebSocket('ws://localhost:{}');
      ws.onopen = () => {{
        connected = true;
        console.log('[topo] Hot reload connected');
      }};
      ws.onmessage = (e) => {{
        if (e.data === 'reload') {{
          console.log('[topo] Reloading...');
          location.reload();
        }}
      }};
      ws.onclose = () => {{
        if (connected) {{
          console.log('[topo] Connection lost, attempting reconnect...');
          setTimeout(() => location.reload(), 1000);
        }}
      }};
      ws.onerror = () => {{}};
    }})();

    // Error Overlay for development
    (function() {{
      const overlay = document.createElement('div');
      overlay.id = 'topo-error-overlay';
      overlay.style.cssText = 'display:none;position:fixed;inset:0;background:rgba(0,0,0,0.85);z-index:99999;padding:32px;overflow:auto;font-family:ui-monospace,monospace';

      function showError(title, message, stack) {{
        overlay.innerHTML = `
          <div style="max-width:900px;margin:0 auto;background:#1a1a1a;border-radius:12px;border:1px solid #333;overflow:hidden">
            <div style="background:#dc2626;color:white;padding:16px 20px;display:flex;justify-content:space-between;align-items:center">
              <span style="font-weight:600;font-size:16px">${{title}}</span>
              <div>
                <button id="topo-copy-btn" style="background:#fff2;border:none;color:white;padding:8px 16px;border-radius:6px;cursor:pointer;margin-right:8px;font-size:13px">Copy</button>
                <button id="topo-close-btn" style="background:#fff2;border:none;color:white;padding:8px 16px;border-radius:6px;cursor:pointer;font-size:13px">✕</button>
              </div>
            </div>
            <div style="padding:20px">
              <div style="color:#f87171;font-size:18px;font-weight:500;margin-bottom:16px;word-break:break-word">${{message}}</div>
              ${{stack ? `<pre style="color:#a1a1aa;font-size:13px;line-height:1.6;margin:0;white-space:pre-wrap;word-break:break-word">${{stack}}</pre>` : ''}}
            </div>
          </div>
        `;
        overlay.style.display = 'block';
        document.getElementById('topo-close-btn').onclick = () => overlay.style.display = 'none';
        document.getElementById('topo-copy-btn').onclick = () => {{
          navigator.clipboard.writeText(message + (stack ? '\\n\\n' + stack : ''));
          document.getElementById('topo-copy-btn').textContent = 'Copied!';
          setTimeout(() => document.getElementById('topo-copy-btn').textContent = 'Copy', 2000);
        }};
      }}

      document.body.appendChild(overlay);

      window.onerror = (msg, src, line, col, err) => {{
        const loc = src ? `${{src}}:${{line}}:${{col}}` : '';
        showError('Runtime Error', msg, err?.stack || loc);
        return false;
      }};

      window.onunhandledrejection = (e) => {{
        showError('Unhandled Promise Rejection', e.reason?.message || String(e.reason), e.reason?.stack);
      }};
    }})();
    </script>
</body>
</html>
"#,
        title, tailwind_script, title_script, ws_port
    )
}

fn start_server(port: u16, output_dir: &PathBuf, open_browser: bool, base_path: &str) -> Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let server = Server::http(&addr).map_err(|e| {
        let err_str = e.to_string();
        if err_str.contains("Address already in use") || err_str.contains("os error 98") {
            anyhow::anyhow!(
                "Port {} is already in use.\n\n\
                 Try one of the following:\n\
                 • Stop the other process using port {}\n\
                 • Use a different port: topo start --port {}\n\
                 • Kill the process: lsof -ti:{} | xargs kill -9",
                port, port, port + 1, port
            )
        } else {
            anyhow::anyhow!("Failed to start server: {}", e)
        }
    })?;

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
        let raw_url_path = request.url().trim_start_matches('/');

        // Strip base_path prefix if present
        let url_path = if !base_path.is_empty() {
            let bp = base_path.trim_start_matches('/');
            if raw_url_path.starts_with(bp) {
                raw_url_path.strip_prefix(bp)
                    .unwrap_or(raw_url_path)
                    .trim_start_matches('/')
            } else {
                raw_url_path
            }
        } else {
            raw_url_path
        };

        // Safely resolve the file path to prevent path traversal attacks
        let file_path = if url_path.is_empty() || url_path == "/" {
            Some(output_dir.join("index.html"))
        } else {
            safe_resolve_path(output_dir, url_path)
        };

        let response = match file_path {
            Some(path) if path.exists() && path.is_file() => {
                match fs::read(&path) {
                    Ok(content) => {
                        let content_type = get_content_type(&path);
                        Response::from_data(content).with_header(
                            tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()).unwrap()
                        )
                    }
                    Err(_) => Response::from_string("500 Internal Server Error")
                        .with_status_code(500),
                }
            }
            _ => {
                // For SPA, serve index.html for non-existent paths
                match fs::read(output_dir.join("index.html")) {
                    Ok(content) => Response::from_data(content).with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], b"text/html").unwrap()
                    ),
                    Err(_) => Response::from_string("404 Not Found").with_status_code(404),
                }
            }
        };

        let _ = request.respond(response);
    }

    Ok(())
}

/// Safely resolve a file path within a base directory, preventing path traversal attacks.
/// Returns None if the resolved path would escape the base directory.
fn safe_resolve_path(base: &PathBuf, url_path: &str) -> Option<PathBuf> {
    // Normalize the base directory
    let base_canonical = match base.canonicalize() {
        Ok(p) => p,
        Err(_) => return None,
    };

    // Block paths containing path traversal sequences
    if url_path.contains("..") || url_path.contains('\0') {
        return None;
    }

    let file_path = base.join(url_path);

    // For non-existent files, verify the parent is within base
    if !file_path.exists() {
        // Check that the path doesn't try to escape
        let normalized = file_path.components()
            .fold(PathBuf::new(), |mut path, comp| {
                match comp {
                    std::path::Component::ParentDir => { path.pop(); }
                    std::path::Component::Normal(s) => { path.push(s); }
                    std::path::Component::RootDir => { path.push("/"); }
                    _ => {}
                }
                path
            });
        if !normalized.starts_with(&base_canonical) && !base.join(&normalized).starts_with(base) {
            return None;
        }
        return Some(file_path);
    }

    // For existing files, canonicalize and verify
    match file_path.canonicalize() {
        Ok(resolved) if resolved.starts_with(&base_canonical) => Some(resolved),
        _ => None,
    }
}

fn get_content_type(path: &Path) -> String {
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

fn start_dev_server(port: u16, config: &Config) -> Result<()> {
    let build_config = config.build_config();
    let paths_config = config.paths_config();
    let dev_config = config.dev_config();

    let input = PathBuf::from(&paths_config.pages);
    let output = PathBuf::from(&build_config.output);
    let mode = match build_config.mode {
        BuildMode::Spa => "spa".to_string(),
        BuildMode::Ssg => "ssg".to_string(),
        BuildMode::Ssr => "ssr".to_string(),
    };

    // Initial build with dev mode HTML (includes hot reload script)
    build_project_dev(&input, &output, &mode, port, config)?;

    // WebSocket clients list
    let ws_clients: Arc<Mutex<Vec<std::net::TcpStream>>> = Arc::new(Mutex::new(Vec::new()));
    let ws_clients_clone = Arc::clone(&ws_clients);

    // Start WebSocket server in separate thread
    let ws_port = port + 1;
    std::thread::spawn(move || {
        let listener = match TcpListener::bind(format!("0.0.0.0:{}", ws_port)) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("  Warning: Could not start WebSocket server: {}", e);
                return;
            }
        };

        for stream in listener.incoming().flatten() {
            let ws_clients = Arc::clone(&ws_clients_clone);
            std::thread::spawn(move || {
                if let Ok(mut websocket) = accept(stream.try_clone().unwrap()) {
                    // Add to clients list
                    if let Ok(mut clients) = ws_clients.lock() {
                        clients.push(stream);
                    }
                    // Keep connection alive
                    loop {
                        match websocket.read() {
                            Ok(Message::Close(_)) | Err(_) => break,
                            Ok(Message::Ping(data)) => {
                                let _ = websocket.send(Message::Pong(data));
                            }
                            _ => {}
                        }
                    }
                }
            });
        }
    });

    // File watcher setup
    let ws_clients_for_watcher = Arc::clone(&ws_clients);
    let input_clone = input.clone();
    let output_clone = output.clone();
    let mode_clone = mode.clone();
    let config_clone = config.clone();

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        if let Ok(event) = res {
            if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                let _ = tx.send(());
            }
        }
    })?;

    // Watch the pages directory and components directory
    watcher.watch(&input, RecursiveMode::Recursive)?;

    // Also watch components directory if it exists
    let components_dir = PathBuf::from(&paths_config.components);
    if components_dir.exists() {
        watcher.watch(&components_dir, RecursiveMode::Recursive)?;
    }

    // Rebuild thread
    std::thread::spawn(move || {
        let mut last_rebuild = std::time::Instant::now();
        loop {
            if rx.recv().is_ok() {
                // Debounce: wait a bit to batch multiple changes
                std::thread::sleep(Duration::from_millis(100));
                // Drain any additional events
                while rx.try_recv().is_ok() {}

                // Avoid rebuilding too frequently
                if last_rebuild.elapsed() < Duration::from_millis(200) {
                    continue;
                }

                println!("\n  File changed, rebuilding...");

                match build_project_dev(&input_clone, &output_clone, &mode_clone, port, &config_clone) {
                    Ok(_) => {
                        println!("  ✓ Rebuild complete");

                        // Notify all WebSocket clients
                        if let Ok(mut clients) = ws_clients_for_watcher.lock() {
                            clients.retain(|client| {
                                if let Ok(mut ws) = accept(client.try_clone().unwrap_or_else(|_| client.try_clone().unwrap())) {
                                    ws.send(Message::Text("reload".into())).is_ok()
                                } else {
                                    false
                                }
                            });
                        }
                    }
                    Err(e) => {
                        eprintln!("  ✗ Build error: {}", e);
                    }
                }
                last_rebuild = std::time::Instant::now();
            }
        }
    });

    // Start HTTP server
    let addr = format!("0.0.0.0:{}", port);
    let server = Server::http(&addr).map_err(|e| {
        let err_str = e.to_string();
        if err_str.contains("Address already in use") || err_str.contains("os error 98") {
            anyhow::anyhow!(
                "Port {} is already in use.\n\n\
                 Try: topo dev --port {}",
                port, port + 10
            )
        } else {
            anyhow::anyhow!("Failed to start server: {}", e)
        }
    })?;

    println!();
    println!("  Dev server running at:");
    println!("  Local:     http://localhost:{}", port);
    println!("  WebSocket: ws://localhost:{}", ws_port);
    println!();
    println!("  Watching for file changes...");
    println!("  Press Ctrl+C to stop");
    println!();

    // Open browser if configured
    if dev_config.open {
        let url = format!("http://localhost:{}", port);
        if let Err(e) = open_in_browser(&url) {
            eprintln!("  Warning: Could not open browser: {}", e);
        }
    }

    // Load mock data if exists (look in parent directory of pages, i.e., demo/mocks)
    let mocks_dir = input.parent().unwrap_or(&input).join("mocks");

    // Serve files
    for request in server.incoming_requests() {
        let url_path = request.url().trim_start_matches('/');

        // Handle API mock routes
        let response = if url_path.starts_with("api/") {
            serve_mock_api(url_path, &mocks_dir)
        } else {
            // Safely resolve the file path to prevent path traversal attacks
            let file_path = if url_path.is_empty() || url_path == "/" {
                Some(output.join("index.html"))
            } else {
                safe_resolve_path(&output, url_path)
            };

            match file_path {
                Some(path) if path.exists() && path.is_file() => {
                    match fs::read(&path) {
                        Ok(content) => {
                            let content_type = get_content_type(&path);
                            Response::from_data(content).with_header(
                                tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()).unwrap()
                            )
                        }
                        Err(_) => Response::from_string("500 Internal Server Error")
                            .with_status_code(500),
                    }
                }
                _ => {
                    // For SPA, serve index.html for non-existent paths
                    match fs::read(output.join("index.html")) {
                        Ok(content) => Response::from_data(content).with_header(
                            tiny_http::Header::from_bytes(&b"Content-Type"[..], b"text/html").unwrap()
                        ),
                        Err(_) => Response::from_string("404 Not Found").with_status_code(404),
                    }
                }
            }
        };

        let _ = request.respond(response);
    }

    Ok(())
}

/// Serve mock API responses from mocks directory
/// URL pattern: /api/{service}/{endpoint} -> mocks/{service}/{endpoint}.json
fn serve_mock_api(url_path: &str, mocks_dir: &PathBuf) -> Response<std::io::Cursor<Vec<u8>>> {
    // Parse: api/dashboard/stats -> mocks/dashboard/stats.json
    let api_path = url_path.strip_prefix("api/").unwrap_or(url_path);

    // Block path traversal attempts
    if api_path.contains("..") || api_path.contains('\0') {
        return Response::from_string(r#"{"error": "Invalid path"}"#)
            .with_status_code(400)
            .with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], b"application/json").unwrap()
            );
    }

    let mock_file = match safe_resolve_path(mocks_dir, &format!("{}.json", api_path)) {
        Some(path) => path,
        None => {
            return Response::from_string(r#"{"error": "Invalid path"}"#)
                .with_status_code(400)
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], b"application/json").unwrap()
                );
        }
    };

    if mock_file.exists() {
        match fs::read(&mock_file) {
            Ok(content) => {
                Response::from_data(content)
                    .with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], b"application/json").unwrap()
                    )
                    .with_header(
                        tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], b"*").unwrap()
                    )
            }
            Err(_) => Response::from_string(r#"{"error": "Failed to read mock file"}"#)
                .with_status_code(500)
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], b"application/json").unwrap()
                ),
        }
    } else {
        Response::from_string(format!(r#"{{"error": "Mock not found", "path": "{}"}}"#, mock_file.display()))
            .with_status_code(404)
            .with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], b"application/json").unwrap()
            )
    }
}

fn run_tests(headed: bool, ui: bool, file: Option<String>) -> Result<()> {
    // Check if package.json exists
    if !PathBuf::from("package.json").exists() {
        println!("No package.json found. Creating test setup...");
        create_test_setup()?;
    }

    // Check if node_modules exists
    if !PathBuf::from("node_modules").exists() {
        println!("Installing dependencies...");
        let status = std::process::Command::new("npm")
            .arg("install")
            .status()?;
        if !status.success() {
            anyhow::bail!("Failed to install dependencies");
        }

        // Install Playwright browsers
        println!("Installing Playwright browsers...");
        let status = std::process::Command::new("npx")
            .args(["playwright", "install", "chromium"])
            .status()?;
        if !status.success() {
            anyhow::bail!("Failed to install Playwright browsers");
        }
    }

    // Compile .test.tp files to Playwright specs
    compile_test_files()?;

    // Build args
    let mut args = vec!["playwright", "test"];

    if headed {
        args.push("--headed");
    }

    if ui {
        args.push("--ui");
    }

    if let Some(ref f) = file {
        args.push(f);
    }

    println!("Running tests...");
    let status = std::process::Command::new("npx")
        .args(&args)
        .status()?;

    if !status.success() {
        anyhow::bail!("Tests failed");
    }

    Ok(())
}

fn compile_test_files() -> Result<()> {
    use glob::glob;

    // Find all .test.tp files only
    let mut test_files = Vec::new();

    for path in glob("**/*.test.tp")?.flatten() {
        // Skip node_modules
        if !path.to_string_lossy().contains("node_modules") {
            test_files.push(path);
        }
    }

    if test_files.is_empty() {
        println!("No .test.tp files found");
        return Ok(());
    }

    // Ensure tests directory exists
    fs::create_dir_all("tests")?;

    for test_file in test_files {
        println!("  Compiling test: {:?}", test_file);
        let source = fs::read_to_string(&test_file)?;

        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize()?;
        let mut parser = TopoParser::new(tokens);
        let ast = parser.parse()?;

        // Get test name from file (e.g., "login" from "login.test.tp")
        let test_name = test_file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("test")
            .replace(".test", "");

        // Generate Playwright test code
        let playwright_code = generate_playwright_test(&ast, &test_name)?;

        // Write to tests directory
        let output_path = format!("tests/{}.spec.ts", test_name);

        fs::write(&output_path, playwright_code)?;
        println!("  Generated: {}", output_path);
    }

    Ok(())
}

fn generate_playwright_test(ast: &Program, test_file_name: &str) -> Result<String> {
    use topo::ast::{TestStatement, TestTarget, TestAssertion, TestHookDef};

    let mut output = String::new();
    output.push_str("import { test, expect } from '@playwright/test';\n\n");

    // Helper to generate test statements (test_num=0 for hooks)
    fn generate_test_statements(statements: &[TestStatement], output: &mut String, test_file_name: &str, test_num: usize, capture_counter: &mut usize) {
        for stmt in statements {
            match stmt {
                TestStatement::Goto { path } => {
                    output.push_str(&format!("  await page.goto('{}');\n", path));
                    output.push_str("  await page.waitForLoadState('networkidle');\n");
                }
                TestStatement::Click { target } => {
                    let selector = target_to_selector(target);
                    let locator = locator_with_first(&selector);
                    output.push_str(&format!("  await {}.click();\n", locator));
                }
                TestStatement::Fill { target, value } => {
                    let selector = target_to_selector(target);
                    let locator = locator_with_first(&selector);
                    let val = expression_to_string(value);
                    output.push_str(&format!("  await {}.fill({});\n", locator, val));
                }
                TestStatement::Type { target, value } => {
                    let selector = target_to_selector(target);
                    let locator = locator_with_first(&selector);
                    let val = expression_to_string(value);
                    output.push_str(&format!("  await {}.type({});\n", locator, val));
                }
                TestStatement::Expect { target, assertion } => {
                    match target {
                        TestTarget::Url => {
                            match assertion {
                                TestAssertion::Equals { value } | TestAssertion::Value { value } => {
                                    output.push_str(&format!("  await expect(page).toHaveURL('{}');\n", value));
                                }
                                _ => {}
                            }
                        }
                        TestTarget::PageProperty { property } if property == "url" => {
                            match assertion {
                                TestAssertion::Equals { value } | TestAssertion::Value { value } => {
                                    output.push_str(&format!("  await expect(page).toHaveURL('{}');\n", value));
                                }
                                _ => {}
                            }
                        }
                        _ => {
                            let selector = target_to_selector(target);
                            let locator = locator_with_first(&selector);
                            match assertion {
                                TestAssertion::Visible => {
                                    output.push_str(&format!("  await expect({}).toBeVisible();\n", locator));
                                }
                                TestAssertion::Hidden => {
                                    output.push_str(&format!("  await expect({}).toBeHidden();\n", locator));
                                }
                                TestAssertion::Disabled => {
                                    output.push_str(&format!("  await expect({}).toBeDisabled();\n", locator));
                                }
                                TestAssertion::Empty => {
                                    output.push_str(&format!("  await expect({}).toBeEmpty();\n", locator));
                                }
                                TestAssertion::HasText { value } => {
                                    output.push_str(&format!("  await expect({}).toHaveText('{}');\n", locator, value));
                                }
                                TestAssertion::Value { value } => {
                                    output.push_str(&format!("  await expect({}).toHaveValue('{}');\n", locator, value));
                                }
                                TestAssertion::Equals { value } => {
                                    output.push_str(&format!("  await expect({}).toHaveText('{}');\n", locator, value));
                                }
                                TestAssertion::Contains { value } => {
                                    output.push_str(&format!("  await expect({}).toContainText('{}');\n", locator, value));
                                }
                            }
                        }
                    }
                }
                TestStatement::Mock { service, method, response } => {
                    // Generate route mock based on service/method
                    let response_str = expression_to_string(response);
                    let route_pattern = format!("**/api/{}/**", service.to_lowercase());
                    output.push_str(&format!(
                        "  // Mock {}.{}\n  await page.route('{}', route => route.fulfill({{ json: {} }}));\n",
                        service, method, route_pattern, response_str
                    ));
                }
                TestStatement::Wait { ms } => {
                    output.push_str(&format!("  await page.waitForTimeout({});\n", ms));
                }
                TestStatement::Capture { filename } => {
                    *capture_counter += 1;
                    match filename {
                        Some(name) => {
                            output.push_str(&format!("  await page.screenshot({{ path: 'screenshots/{}/{}' }});\n", test_file_name, name));
                        }
                        None => {
                            output.push_str(&format!("  await page.screenshot({{ path: 'screenshots/{}/{}-{}.png' }});\n", test_file_name, test_num, capture_counter));
                        }
                    }
                }
            }
        }
    }

    // Helper to generate hook (test_num=0 for hooks)
    fn generate_hook(hook_name: &str, hook_def: &TestHookDef, output: &mut String, test_file_name: &str, capture_counter: &mut usize) {
        output.push_str(&format!("test.{}(async ({{ page }}) => {{\n", hook_name));
        generate_test_statements(&hook_def.statements, output, test_file_name, 0, capture_counter);
        output.push_str("});\n\n");
    }

    let mut hook_capture_counter: usize = 0;

    // First pass: generate beforeAll/afterAll hooks (BeforeOnce/AfterOnce)
    for decl in &ast.declarations {
        match decl {
            Declaration::BeforeOnce(hook_def) => {
                generate_hook("beforeAll", hook_def, &mut output, test_file_name, &mut hook_capture_counter);
            }
            Declaration::AfterOnce(hook_def) => {
                generate_hook("afterAll", hook_def, &mut output, test_file_name, &mut hook_capture_counter);
            }
            _ => {}
        }
    }

    // Second pass: generate beforeEach/afterEach hooks
    for decl in &ast.declarations {
        match decl {
            Declaration::BeforeEach(hook_def) => {
                generate_hook("beforeEach", hook_def, &mut output, test_file_name, &mut hook_capture_counter);
            }
            Declaration::AfterEach(hook_def) => {
                generate_hook("afterEach", hook_def, &mut output, test_file_name, &mut hook_capture_counter);
            }
            _ => {}
        }
    }

    // Third pass: generate tests
    let mut test_num: usize = 0;
    for decl in &ast.declarations {
        if let Declaration::Test(test_def) = decl {
            test_num += 1;
            let mut capture_counter: usize = 0;
            // Use test.skip for skipped tests
            let test_fn = if test_def.skip { "test.skip" } else { "test" };
            output.push_str(&format!("{}('{}', async ({{ page }}) => {{\n", test_fn, test_def.name));

            generate_test_statements(&test_def.statements, &mut output, test_file_name, test_num, &mut capture_counter);

            output.push_str("});\n\n");
        }
    }

    Ok(output)
}

fn target_to_selector(target: &topo::ast::TestTarget) -> String {
    use topo::ast::TestTarget;

    match target {
        TestTarget::Field { store, field } => {
            // Use data-error for error fields, data-field for others
            if field.ends_with("Error") {
                format!("[data-error=\"{}.{}\"]", store, field)
            } else {
                format!("[data-field=\"{}.{}\"]", store, field)
            }
        }
        TestTarget::Text { content } => {
            format!("text={}", content)
        }
        TestTarget::Submit => {
            "button[type=\"submit\"]".to_string()
        }
        TestTarget::Button { content } => {
            format!("button:has-text(\"{}\")", content)
        }
        TestTarget::Url => {
            "".to_string() // Handled specially in expect
        }
        TestTarget::PageProperty { property: _ } => {
            "".to_string() // Handled specially in expect for page.url
        }
        TestTarget::Selector { selector } => {
            selector.clone()
        }
    }
}

// Generate locator with .first() for text selectors to avoid strict mode violations
fn locator_with_first(selector: &str) -> String {
    if selector.starts_with("text=") {
        format!("page.locator('{}').first()", selector)
    } else {
        format!("page.locator('{}')", selector)
    }
}

fn expression_to_string(expr: &topo::ast::Expression) -> String {
    use topo::ast::Expression;

    match expr {
        Expression::String { value } => format!("'{}'", value),
        Expression::Number { value } => value.to_string(),
        Expression::Boolean { value } => value.to_string(),
        Expression::Null => "null".to_string(),
        Expression::Array { elements } => {
            let elems: Vec<String> = elements.iter().map(expression_to_string).collect();
            format!("[{}]", elems.join(", "))
        }
        Expression::Object { members } => {
            let props: Vec<String> = members
                .iter()
                .map(|m| match m {
                    ObjectMember::Property(p) => format!("{}: {}", p.key, expression_to_string(&p.value)),
                    ObjectMember::Spread { expr } => format!("...{}", expression_to_string(expr)),
                })
                .collect();
            format!("{{ {} }}", props.join(", "))
        }
        _ => "''".to_string(),
    }
}

fn create_test_setup() -> Result<()> {
    // Create package.json
    let package_json = r#"{
  "name": "topo-app",
  "version": "0.1.0",
  "scripts": {
    "test": "playwright test",
    "test:ui": "playwright test --ui",
    "test:headed": "playwright test --headed"
  },
  "devDependencies": {
    "@playwright/test": "^1.40.0"
  }
}
"#;
    fs::write("package.json", package_json)?;

    // Create playwright.config.ts
    let playwright_config = r#"import { defineConfig, devices } from '@playwright/test';
import { readFileSync, existsSync } from 'fs';

// Read basePath from topo.config.json
function getBasePath(): string {
  const configPath = './topo.config.json';
  if (existsSync(configPath)) {
    try {
      const config = JSON.parse(readFileSync(configPath, 'utf-8'));
      return config.build?.basePath || '';
    } catch {
      return '';
    }
  }
  return '';
}

const basePath = getBasePath();
const port = 3333;

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    baseURL: `http://localhost:${port}`,
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: `topo start --port ${port} --no-open`,
    url: `http://localhost:${port}${basePath || '/'}`,
    reuseExistingServer: false,
    timeout: 120 * 1000,
  },
});
"#;
    fs::write("playwright.config.ts", playwright_config)?;

    // Create tests directory
    fs::create_dir_all("tests")?;

    // Create sample test
    let sample_test = r#"import { test, expect } from '@playwright/test';

test('has title', async ({ page }) => {
  await page.goto('/');
  await expect(page).toHaveTitle(/topo/);
});

test('can navigate', async ({ page }) => {
  await page.goto('/');
  // Add your navigation tests here
});
"#;
    fs::write("tests/app.spec.ts", sample_test)?;

    println!("✓ Created test setup");
    println!("  - package.json");
    println!("  - playwright.config.ts");
    println!("  - tests/app.spec.ts");

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

fn show_info_list(pages_only: bool, apis_only: bool) -> Result<()> {
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
            candidates.into_iter().find(|p| p.exists()).unwrap_or_else(|| PathBuf::from("pages"))
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
            candidates.into_iter().find(|p| p.exists()).unwrap_or_else(|| PathBuf::from("services"))
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
            candidates.into_iter().find(|p| p.exists()).unwrap_or_else(|| PathBuf::from("components"))
        }
    };

    // Find all .tp files in pages directory
    let page_files = find_tp_files(&pages_dir)?;

    // Find all .tp files for API search (pages + services + components)
    let mut all_files = page_files.clone();
    if services_dir.exists() {
        all_files.extend(find_tp_files(&services_dir)?);
    }
    if components_dir.exists() {
        all_files.extend(find_tp_files(&components_dir)?);
    }

    let show_all = !pages_only && !apis_only;

    // Show pages
    if show_all || pages_only {
        println!("\n\x1b[1;36m📄 Pages\x1b[0m");
        println!("\x1b[90m{}\x1b[0m", "─".repeat(50));

        let routes = generate_routes(&page_files, &pages_dir)?;
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
                                let rel_path = file.strip_prefix(&services_dir)
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
                println!("  \x1b[1;35m{}\x1b[0m \x1b[90m({})\x1b[0m", display_name, file_path);

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
                            type_parts.push(format!("\x1b[36m{}\x1b[0m", format_type_annotation(req_type)));
                        }
                        let response_str = if let Some(ref res_type) = endpoint.response_type {
                            format!(" \x1b[90m->\x1b[0m \x1b[32m{}\x1b[0m", format_type_annotation(res_type))
                        } else {
                            String::new()
                        };
                        let error_str = if let Some(ref err_type) = endpoint.error_type {
                            format!(" \x1b[90m|\x1b[0m \x1b[31m{}\x1b[0m", format_type_annotation(err_type))
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

/// Find project root by looking for topo.config.json
/// Searches from input directory upwards, falls back to input's parent
fn find_project_root(input: &Path) -> Result<PathBuf> {
    let start_dir = if input.is_file() {
        input.parent().unwrap_or(input).to_path_buf()
    } else {
        input.to_path_buf()
    };

    // Search upwards for topo.config.json
    let mut current = start_dir.canonicalize().unwrap_or(start_dir.clone());
    loop {
        if current.join("topo.config.json").exists() {
            return Ok(current);
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    // Fallback: use input directory's parent (for pages -> project root)
    Ok(start_dir.parent().unwrap_or(&start_dir).to_path_buf())
}

fn find_tp_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if dir.is_file() {
        if dir.extension().is_some_and(|ext| ext == "tp") {
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
        } else if path.extension().is_some_and(|ext| ext == "tp") {
            // Skip http.setup.tp (raw JavaScript file, not topo component)
            if path.file_name().is_some_and(|name| name == "http.setup.tp") {
                continue;
            }
            files.push(path);
        }
    }

    Ok(files)
}

/// Generate i18n runtime code
fn generate_i18n_runtime(config: &I18nConfig) -> String {
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

/// Generate file-based routes from pages directory
/// pages/index.tp -> /
/// pages/about.tp -> /about
/// pages/users/index.tp -> /users
/// pages/users/[id].tp -> /users/[id]
fn generate_routes(files: &[PathBuf], base_dir: &Path) -> Result<Vec<(String, String)>> {
    let mut routes = Vec::new();

    // Look for pages directory
    let pages_dir = if base_dir.join("pages").exists() {
        base_dir.join("pages")
    } else if base_dir.ends_with("pages") {
        base_dir.to_path_buf()
    } else {
        // No pages directory, no file-based routing
        return Ok(routes);
    };

    for file in files {
        // Only process files in pages directory
        if !file.starts_with(&pages_dir) {
            continue;
        }

        // Get file stem (name without extension)
        let file_stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        // Skip non-page files (template, store, etc.)
        // Only index.tp, [param].tp, or root-level named files are pages
        if file_stem == "template" || file_stem == "store" || file_stem == "layout" {
            continue;
        }

        // Get relative path from pages directory
        let relative = file.strip_prefix(&pages_dir)?;
        let path_str = relative.to_string_lossy();

        // Skip files inside components/ directory (not routes)
        if path_str.contains("/components/") || path_str.starts_with("components/") {
            continue;
        }

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
                        .map(capitalize)
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
    } else if let Some(stripped) = path.strip_suffix("/index") {
        stripped
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

/// Convert to PascalCase (handles hyphens and underscores)
/// e.g., "quick-start" -> "QuickStart", "my_component" -> "MyComponent"
fn capitalize(s: &str) -> String {
    s.split(['-', '_'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}

/// Generate SSR output files for the specified target
fn generate_ssr_output(output: &Path, routes: &[(String, String)], config: &Config, target: &str) -> Result<()> {
    match target {
        "cloudflare" => generate_cloudflare_worker(output, routes, config),
        "rust" => {
            println!("  Rust SSR target is not yet implemented");
            Ok(())
        }
        _ => {
            eprintln!("Unknown SSR target: {}. Using cloudflare.", target);
            generate_cloudflare_worker(output, routes, config)
        }
    }
}

/// Generate Cloudflare Workers code for SSR
fn generate_cloudflare_worker(output: &Path, routes: &[(String, String)], config: &Config) -> Result<()> {
    let base_path = config
        .build
        .as_ref()
        .and_then(|b| b.base_path.clone())
        .unwrap_or_default();

    let title = config
        .project
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "topo App".to_string());

    // Generate worker.js
    let worker_js = generate_worker_js(&base_path, &title, routes);
    fs::write(output.join("worker.js"), &worker_js)?;
    println!("  Generated: worker.js");

    // Generate wrangler.toml
    let wrangler_toml = generate_wrangler_toml(&title);
    fs::write(output.join("wrangler.toml"), &wrangler_toml)?;
    println!("  Generated: wrangler.toml");

    println!("✓ SSR build complete for Cloudflare Workers");
    println!("  To deploy: cd {} && wrangler deploy", output.display());

    Ok(())
}

/// Generate Cloudflare Worker JavaScript
fn generate_worker_js(base_path: &str, title: &str, routes: &[(String, String)]) -> String {
    let routes_json: Vec<String> = routes
        .iter()
        .map(|(pattern, component)| format!("  {{ pattern: '{}', component: '{}' }}", pattern, component))
        .collect();

    format!(r#"// Cloudflare Worker for topo SSR
// Auto-generated - do not edit directly

import {{ renderPage }} from './app.js';

const BASE_PATH = '{base_path}';
const DEFAULT_TITLE = '{title}';

const ROUTES = [
{routes}
];

// Match route pattern to path
function matchRoute(path) {{
  for (const route of ROUTES) {{
    const paramNames = [];
    const regexPattern = route.pattern.replace(/\[([^\]]+)\]/g, (_, name) => {{
      if (name.startsWith('...')) {{
        paramNames.push(name.slice(3));
        return '(.*)';
      }}
      paramNames.push(name);
      return '([^/]+)';
    }});
    const regex = new RegExp(`^${{regexPattern}}$`);
    const match = path.match(regex);
    if (match) {{
      const params = {{}};
      paramNames.forEach((name, i) => {{ params[name] = match[i + 1]; }});
      return {{ component: route.component, params }};
    }}
  }}
  return null;
}}

// Generate HTML shell
function generateHtml(content, pageTitle) {{
  const fullTitle = pageTitle ? `${{pageTitle}} | ${{DEFAULT_TITLE}}` : DEFAULT_TITLE;
  const assetPrefix = BASE_PATH ? `${{BASE_PATH}}/` : '/';

  return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>${{fullTitle}}</title>
    <link rel="stylesheet" href="${{assetPrefix}}styles.css">
</head>
<body>
    <div id="app">${{content}}</div>
    <script>window.__TOPO_BASE_PATH = '${{BASE_PATH}}'; window.__TOPO_DEFAULT_TITLE = '${{DEFAULT_TITLE}}';</script>
    <script type="module" src="${{assetPrefix}}app.js"></script>
</body>
</html>`;
}}

export default {{
  async fetch(request, env, ctx) {{
    const url = new URL(request.url);
    let path = url.pathname;

    // Remove base path prefix
    if (BASE_PATH && path.startsWith(BASE_PATH)) {{
      path = path.slice(BASE_PATH.length) || '/';
    }}

    // Normalize trailing slash
    if (path !== '/' && path.endsWith('/')) {{
      path = path.slice(0, -1);
    }}

    // Check for static assets
    if (path.match(/\.(js|css|ico|png|jpg|jpeg|gif|svg|woff|woff2|ttf|eot)$/)) {{
      // Let static assets be served by Cloudflare Pages or R2
      return env.ASSETS ? env.ASSETS.fetch(request) : new Response('Not Found', {{ status: 404 }});
    }}

    // Match route
    const matched = matchRoute(path);

    if (matched) {{
      try {{
        // Call renderPage from app.js (server-side render)
        const {{ content, title }} = await renderPage(matched.component, matched.params);
        const html = generateHtml(content, title);
        return new Response(html, {{
          headers: {{ 'content-type': 'text/html;charset=UTF-8' }}
        }});
      }} catch (e) {{
        console.error('SSR Error:', e);
        // Fallback to client-side rendering
        const html = generateHtml('', null);
        return new Response(html, {{
          headers: {{ 'content-type': 'text/html;charset=UTF-8' }}
        }});
      }}
    }}

    // 404 - render empty shell for client-side routing
    const html = generateHtml('', null);
    return new Response(html, {{
      status: 404,
      headers: {{ 'content-type': 'text/html;charset=UTF-8' }}
    }});
  }}
}};
"#,
        base_path = base_path,
        title = title,
        routes = routes_json.join(",\n")
    )
}

/// Generate wrangler.toml configuration
fn generate_wrangler_toml(name: &str) -> String {
    format!(r#"name = "{name}"
main = "worker.js"
compatibility_date = "2024-01-01"

# Uncomment for custom domain
# routes = [
#   {{ pattern = "example.com/*", zone_name = "example.com" }}
# ]

# Static assets (if using Cloudflare Pages)
# [site]
# bucket = "./"

# Environment variables
# [vars]
# MY_VAR = "value"
"#,
        name = name.to_lowercase().replace(' ', "-")
    )
}
