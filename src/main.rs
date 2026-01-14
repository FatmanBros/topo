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

use topo::ast::{Declaration, ObjectMember, Program, TypeAnnotation};
use topo::config::{Config, BuildMode};
use topo::info_server::start_info_server;
use topo::lexer::Lexer;
use topo::parser::Parser as TopoParser;

mod build;
mod deploy;

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

            build::build_project(&input, &output, &mode, &target)?;
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
            build::build_project(&input, &output, &mode, target)?;

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
    build::build_project_dev(&input, &output, &mode, port, config)?;

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

                match build::build_project_dev(&input_clone, &output_clone, &mode_clone, port, &config_clone) {
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
