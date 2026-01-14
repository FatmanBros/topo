//! Scaffold module - project creation and initialization

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

// =============================================================================
// Templates (embedded)
// =============================================================================

mod templates {
    pub const STARTER_CONFIG: &str = include_str!("../../templates/starter/topo.config.json");
    pub const STARTER_INDEX: &str = include_str!("../../templates/starter/pages/index.tp");
    pub const STARTER_GITIGNORE: &str = include_str!("../../templates/starter/.gitignore");

    pub const WITH_AUTH_CONFIG: &str = include_str!("../../templates/with-auth/topo.config.json");
    pub const WITH_AUTH_INDEX: &str = include_str!("../../templates/with-auth/pages/index.tp");
    pub const WITH_AUTH_LOGIN: &str = include_str!("../../templates/with-auth/pages/login.tp");
    pub const WITH_AUTH_DASHBOARD: &str = include_str!("../../templates/with-auth/pages/dashboard.tp");
    pub const WITH_AUTH_AUTH_STORE: &str = include_str!("../../templates/with-auth/stores/auth.tp");
    pub const WITH_AUTH_GITIGNORE: &str = include_str!("../../templates/with-auth/.gitignore");
}

pub fn create_project(name: &str) -> Result<()> {
    println!("Creating new topo project: {}", name);

    fs::create_dir_all(format!("{}/src/pages", name))?;
    fs::create_dir_all(format!("{}/src/components", name))?;
    fs::create_dir_all(format!("{}/src/stores", name))?;
    fs::create_dir_all(format!("{}/src/services", name))?;

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

pub fn list_templates() {
    println!("Available templates:");
    println!();
    println!("  starter     - Minimal starter template (default)");
    println!("  with-auth   - Template with login page and authentication");
    println!();
    println!("Usage: topo create-app my-app --template <template>");
}

pub fn create_app(name: &str, template: &str) -> Result<()> {
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

    fs::create_dir_all(format!("{}/pages", name))?;
    fs::create_dir_all(format!("{}/components", name))?;

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

    fs::create_dir_all(format!("{}/pages", name))?;
    fs::create_dir_all(format!("{}/components", name))?;
    fs::create_dir_all(format!("{}/stores", name))?;

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

pub fn init_project() -> Result<()> {
    println!("Initializing topo project in current directory...");

    if PathBuf::from("topo.config.json").exists() {
        println!("✗ topo.config.json already exists");
        return Ok(());
    }

    fs::create_dir_all("src/pages")?;
    fs::create_dir_all("src/components")?;
    fs::create_dir_all("src/stores")?;
    fs::create_dir_all("src/services")?;

    let name = std::env::current_dir()?
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "my-app".to_string());

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
