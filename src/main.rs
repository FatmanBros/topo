//! Topo CLI - A UI framework that eliminates nesting hell

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use topo::config::{BuildMode, Config};
use topo::info_server::start_info_server;

mod build;
mod commands;
mod deploy;
mod scaffold;
mod server;
mod test_runner;

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

        /// Verify build by checking for JS runtime errors (requires Playwright)
        #[arg(long)]
        verify: bool,
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
            scaffold::create_project(&name)?;
        }
        Commands::CreateApp { name, template, list } => {
            if list {
                scaffold::list_templates();
            } else {
                scaffold::create_app(&name, &template)?;
            }
        }
        Commands::Init => {
            scaffold::init_project()?;
        }
        Commands::Build { input, output, mode, target, verify } => {
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

            // Verify build if requested
            if verify {
                let base_path = config
                    .build
                    .as_ref()
                    .and_then(|b| b.base_path.clone())
                    .unwrap_or_default();
                build::verify_build(&output, &base_path)?;
            }
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
            let target = "cloudflare";

            // Build first
            build::build_project(&input, &output, &mode, target)?;

            // Get base_path from config
            let base_path = config
                .build
                .as_ref()
                .and_then(|b| b.base_path.clone())
                .unwrap_or_default();

            // Then start server
            server::start_server(port, &output, !no_open && dev_config.open, &base_path)?;
        }
        Commands::Dev { port } => {
            let config = Config::load_or_default();
            let dev_config = config.dev_config();
            let port = port.unwrap_or(dev_config.port);

            server::start_dev_server(port, &config)?;
        }
        Commands::Test { headed, ui, file } => {
            test_runner::run_tests(headed, ui, file)?;
        }
        Commands::Check { input } => {
            commands::check_project(&input)?;
        }
        Commands::Parse { file, json } => {
            commands::parse_file(&file, json)?;
        }
        Commands::Config => {
            commands::show_config()?;
        }
        Commands::Info { command } => {
            match command {
                Some(InfoCommands::List { pages, apis }) => {
                    commands::show_info_list(pages, apis)?;
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
