use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use tiny_http::{Response, Server};
use std::sync::{Arc, Mutex};
use std::net::TcpListener;
use std::time::Duration;
use notify::{Watcher, RecursiveMode};
use tungstenite::{accept, Message};

use std::collections::HashMap;

use topo::ast::{Declaration, Program};
use topo::codegen::JsCodegen;
use topo::config::{Config, BuildMode, I18nConfig};
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

    // Load config early for i18n generation
    let config = Config::load_or_default();

    // Generate code in dependency order
    let mut all_output = String::new();
    let mut codegen = JsCodegen::new();

    // First pass: collect all component params from all files for cross-file param detection
    for file in &compile_order {
        if let Some(program) = parsed_files.get(file) {
            codegen.collect_component_params(program);
        }
    }

    // Generate runtime once at the beginning
    all_output.push_str(&codegen.generate_runtime());

    // Generate i18n runtime if configured
    if let Some(i18n_config) = &config.i18n {
        all_output.push_str(&generate_i18n_runtime(i18n_config));
    }

    // Generate file-based routes
    let routes = generate_routes(&entry_files, input)?;
    if !routes.is_empty() {
        all_output.push_str("\n// File-based routes\n");
        for (pattern, component) in &routes {
            all_output.push_str(&format!("registerRoute('{}', {});\n", pattern, component));
        }
        all_output.push_str("\n");
    }

    let mut has_app = false;
    for file in &compile_order {
        println!("  Compiling: {:?}", file);
        if let Some(program) = parsed_files.get(file) {
            // Check if this file contains App component
            for decl in &program.declarations {
                if let Declaration::Component(comp) = decl {
                    if comp.name == "App" {
                        has_app = true;
                    }
                }
            }
            let js = codegen.generate(program);
            all_output.push_str(&js);
            all_output.push('\n');
        }
    }

    // Add mount call at the end
    // If App component exists, use it; otherwise if routes exist, let router handle it
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
    println!("✓ Build complete: {:?}", output_file);

    // Generate HTML
    let html = generate_html(&config);
    fs::write(output.join("index.html"), html)?;

    Ok(())
}

/// Build project for development mode (with hot reload script)
fn build_project_dev(input: &PathBuf, output: &PathBuf, _mode: &str, ws_port: u16, config: &Config) -> Result<()> {
    // Create output directory
    fs::create_dir_all(output)?;

    // Find all .tp files or use single file
    let entry_files = find_tp_files(input)?;

    // Parse all files and resolve imports
    let mut parsed_files: HashMap<PathBuf, Program> = HashMap::new();
    let mut compile_order: Vec<PathBuf> = Vec::new();

    // Parse entry files and their dependencies
    for file in &entry_files {
        resolve_imports(file, input, &mut parsed_files, &mut compile_order)?;
    }

    // Generate code in dependency order
    let mut all_output = String::new();
    let mut codegen = JsCodegen::new();

    // First pass: collect all component params from all files for cross-file param detection
    for file in &compile_order {
        if let Some(program) = parsed_files.get(file) {
            codegen.collect_component_params(program);
        }
    }

    // Generate runtime once at the beginning
    all_output.push_str(&codegen.generate_runtime());

    // Generate i18n runtime if configured
    if let Some(i18n_config) = &config.i18n {
        all_output.push_str(&generate_i18n_runtime(i18n_config));
    }

    // Generate file-based routes
    let routes = generate_routes(&entry_files, input)?;
    if !routes.is_empty() {
        all_output.push_str("\n// File-based routes\n");
        for (pattern, component) in &routes {
            all_output.push_str(&format!("registerRoute('{}', {});\n", pattern, component));
        }
        all_output.push_str("\n");
    }

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
            let js = codegen.generate(program);
            all_output.push_str(&js);
            all_output.push('\n');
        }
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
        title, tailwind_script
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
    <script>
    // Hot Reload WebSocket
    (function() {{
      const ws = new WebSocket('ws://localhost:{}');
      ws.onmessage = (e) => {{
        if (e.data === 'reload') {{
          console.log('[topo] Reloading...');
          location.reload();
        }}
      }};
      ws.onclose = () => {{
        console.log('[topo] Connection lost, attempting reconnect...');
        setTimeout(() => location.reload(), 1000);
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
        title, tailwind_script, ws_port
    )
}

fn start_server(port: u16, output_dir: &PathBuf, open_browser: bool) -> Result<()> {
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

        for stream in listener.incoming() {
            if let Ok(stream) = stream {
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
                                    ws.send(Message::Text("reload".to_string())).is_ok()
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

    // Serve files
    for request in server.incoming_requests() {
        let url_path = request.url().trim_start_matches('/');
        let file_path = if url_path.is_empty() || url_path == "/" {
            output.join("index.html")
        } else {
            output.join(url_path)
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
            match fs::read(output.join("index.html")) {
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

    for entry in glob("**/*.test.tp")? {
        if let Ok(path) = entry {
            // Skip node_modules
            if !path.to_string_lossy().contains("node_modules") {
                test_files.push(path);
            }
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

        // Generate Playwright test code
        let playwright_code = generate_playwright_test(&ast)?;

        // Write to tests directory
        let output_name = test_file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("test");
        let output_path = format!("tests/{}.spec.ts", output_name.replace(".test", ""));

        fs::write(&output_path, playwright_code)?;
        println!("  Generated: {}", output_path);
    }

    Ok(())
}

fn generate_playwright_test(ast: &Program) -> Result<String> {
    use topo::ast::{TestStatement, TestTarget, TestAssertion, TestHookDef};

    let mut output = String::new();
    output.push_str("import { test, expect } from '@playwright/test';\n\n");

    // Helper to generate test statements (test_num=0 for hooks)
    fn generate_test_statements(statements: &[TestStatement], output: &mut String, test_num: usize, capture_counter: &mut usize) {
        for stmt in statements {
            match stmt {
                TestStatement::Goto { path } => {
                    output.push_str(&format!("  await page.goto('{}');\n", path));
                }
                TestStatement::Click { target } => {
                    let selector = target_to_selector(target);
                    output.push_str(&format!("  await page.locator('{}').click();\n", selector));
                }
                TestStatement::Fill { target, value } => {
                    let selector = target_to_selector(target);
                    let val = expression_to_string(value);
                    output.push_str(&format!("  await page.locator('{}').fill({});\n", selector, val));
                }
                TestStatement::Type { target, value } => {
                    let selector = target_to_selector(target);
                    let val = expression_to_string(value);
                    output.push_str(&format!("  await page.locator('{}').type({});\n", selector, val));
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
                            match assertion {
                                TestAssertion::Visible => {
                                    output.push_str(&format!("  await expect(page.locator('{}')).toBeVisible();\n", selector));
                                }
                                TestAssertion::Hidden => {
                                    output.push_str(&format!("  await expect(page.locator('{}')).toBeHidden();\n", selector));
                                }
                                TestAssertion::Disabled => {
                                    output.push_str(&format!("  await expect(page.locator('{}')).toBeDisabled();\n", selector));
                                }
                                TestAssertion::Empty => {
                                    output.push_str(&format!("  await expect(page.locator('{}')).toBeEmpty();\n", selector));
                                }
                                TestAssertion::HasText { value } => {
                                    output.push_str(&format!("  await expect(page.locator('{}')).toHaveText('{}');\n", selector, value));
                                }
                                TestAssertion::Value { value } => {
                                    output.push_str(&format!("  await expect(page.locator('{}')).toHaveValue('{}');\n", selector, value));
                                }
                                TestAssertion::Equals { value } => {
                                    output.push_str(&format!("  await expect(page.locator('{}')).toHaveText('{}');\n", selector, value));
                                }
                                TestAssertion::Contains { value } => {
                                    output.push_str(&format!("  await expect(page.locator('{}')).toContainText('{}');\n", selector, value));
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
                            output.push_str(&format!("  await page.screenshot({{ path: 'screenshots/{}' }});\n", name));
                        }
                        None => {
                            output.push_str(&format!("  await page.screenshot({{ path: 'screenshots/{}-{}.png' }});\n", test_num, capture_counter));
                        }
                    }
                }
            }
        }
    }

    // Helper to generate hook (test_num=0 for hooks)
    fn generate_hook(hook_name: &str, hook_def: &TestHookDef, output: &mut String, capture_counter: &mut usize) {
        output.push_str(&format!("test.{}(async ({{ page }}) => {{\n", hook_name));
        generate_test_statements(&hook_def.statements, output, 0, capture_counter);
        output.push_str("});\n\n");
    }

    let mut hook_capture_counter: usize = 0;

    // First pass: generate beforeAll/afterAll hooks (BeforeOnce/AfterOnce)
    for decl in &ast.declarations {
        match decl {
            Declaration::BeforeOnce(hook_def) => {
                generate_hook("beforeAll", hook_def, &mut output, &mut hook_capture_counter);
            }
            Declaration::AfterOnce(hook_def) => {
                generate_hook("afterAll", hook_def, &mut output, &mut hook_capture_counter);
            }
            _ => {}
        }
    }

    // Second pass: generate beforeEach/afterEach hooks
    for decl in &ast.declarations {
        match decl {
            Declaration::BeforeEach(hook_def) => {
                generate_hook("beforeEach", hook_def, &mut output, &mut hook_capture_counter);
            }
            Declaration::AfterEach(hook_def) => {
                generate_hook("afterEach", hook_def, &mut output, &mut hook_capture_counter);
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

            generate_test_statements(&test_def.statements, &mut output, test_num, &mut capture_counter);

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

fn expression_to_string(expr: &topo::ast::Expression) -> String {
    use topo::ast::Expression;

    match expr {
        Expression::String { value } => format!("'{}'", value),
        Expression::Number { value } => value.to_string(),
        Expression::Boolean { value } => value.to_string(),
        Expression::Null => "null".to_string(),
        Expression::Array { elements } => {
            let elems: Vec<String> = elements.iter().map(|e| expression_to_string(e)).collect();
            format!("[{}]", elems.join(", "))
        }
        Expression::Object { properties } => {
            let props: Vec<String> = properties
                .iter()
                .map(|p| format!("{}: {}", p.key, expression_to_string(&p.value)))
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

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    baseURL: 'http://localhost:3000',
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'topo start --port 3000 --no-open',
    url: 'http://localhost:3000',
    reuseExistingServer: !process.env.CI,
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
