use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

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

    /// Start development server
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

    // Find all .tp files
    let tp_files = find_tp_files(input)?;
    println!("  Found {} .tp files", tp_files.len());

    let mut all_output = String::new();

    for file in &tp_files {
        println!("  Compiling: {:?}", file);

        let source = fs::read_to_string(file)?;
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize()?;
        let mut parser = TopoParser::new(tokens);
        let program = parser.parse()?;
        let mut codegen = JsCodegen::new();
        let js = codegen.generate(&program);

        all_output.push_str(&js);
        all_output.push('\n');
    }

    // Write output
    let output_file = output.join("app.js");
    fs::write(&output_file, &all_output)?;
    println!("✓ Build complete: {:?}", output_file);

    // Create index.html
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>topo App</title>
    <script src="https://cdn.tailwindcss.com"></script>
</head>
<body>
    <div id="app"></div>
    <script type="module" src="./app.js"></script>
</body>
</html>
"#;
    fs::write(output.join("index.html"), html)?;

    Ok(())
}

fn start_dev_server(port: u16, _config: &Config) -> Result<()> {
    println!("Starting development server...");
    println!("  Port: {}", port);
    println!();
    println!("  Local: http://localhost:{}", port);
    println!();
    println!("(Dev server not yet implemented - use 'topo build' for now)");
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
