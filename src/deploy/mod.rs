//! Deploy module - handles route generation and SSR output

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use topo::ast::{ApiServiceDef, Declaration, Program};
use topo::codegen::{AxumCodegen, WorkersCodegen, generate_cargo_toml};
use topo::config::Config;

/// Generate routes from file-based routing structure
/// pages/index.tp -> /
/// pages/about.tp -> /about
/// pages/users/index.tp -> /users
/// pages/users/[id].tp -> /users/[id]
pub fn generate_routes(files: &[PathBuf], base_dir: &Path) -> Result<Vec<(String, String)>> {
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
pub fn generate_ssr_output(output: &Path, routes: &[(String, String)], config: &Config, target: &str) -> Result<()> {
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

// ============================================================================
// Server API Code Generation
// ============================================================================

/// Extract API services with server blocks from parsed programs
pub fn extract_api_services_with_server<'a>(programs: &'a [&'a Program]) -> Vec<&'a ApiServiceDef> {
    let mut services = Vec::new();
    for program in programs {
        for decl in &program.declarations {
            if let Declaration::ApiService(api) = decl {
                if api.server.is_some() {
                    services.push(api);
                }
            }
        }
    }
    services
}

/// Generate Cloudflare Workers API server code
pub fn generate_cloudflare_api(output: &Path, services: &[&ApiServiceDef], config: &Config) -> Result<()> {
    if services.is_empty() {
        return Ok(());
    }

    let api_output = output.join("api");
    fs::create_dir_all(&api_output)?;

    let mut codegen = WorkersCodegen::new();
    let worker_code = codegen.generate(services);

    fs::write(api_output.join("worker.js"), &worker_code)?;
    println!("  Generated: api/worker.js");

    // Generate wrangler.toml for API
    let name = config
        .project
        .as_ref()
        .map(|p| format!("{}-api", p.name))
        .unwrap_or_else(|| "topo-api".to_string());

    let wrangler = format!(r#"name = "{}"
main = "worker.js"
compatibility_date = "2024-01-01"

# D1 Database binding (optional)
# [[d1_databases]]
# binding = "DB"
# database_name = "my-database"
# database_id = "your-database-id"

# KV Namespace binding (optional)
# [[kv_namespaces]]
# binding = "KV"
# id = "your-kv-id"
"#, name.to_lowercase().replace(' ', "-"));

    fs::write(api_output.join("wrangler.toml"), &wrangler)?;
    println!("  Generated: api/wrangler.toml");

    println!("✓ Cloudflare Workers API generated");
    println!("  To deploy: cd {}/api && wrangler deploy", output.display());

    Ok(())
}

/// Generate Rust Axum API server code
pub fn generate_axum_api(output: &Path, services: &[&ApiServiceDef], config: &Config) -> Result<()> {
    if services.is_empty() {
        return Ok(());
    }

    let api_output = output.join("api-rust");
    let src_dir = api_output.join("src");
    fs::create_dir_all(&src_dir)?;

    let mut codegen = AxumCodegen::new();
    let rust_code = codegen.generate(services);

    fs::write(src_dir.join("main.rs"), &rust_code)?;
    println!("  Generated: api-rust/src/main.rs");

    // Generate Cargo.toml
    let name = config
        .project
        .as_ref()
        .map(|p| format!("{}-api", p.name))
        .unwrap_or_else(|| "topo-api".to_string());

    let cargo_toml = generate_cargo_toml(&name.to_lowercase().replace(' ', "-"));
    fs::write(api_output.join("Cargo.toml"), &cargo_toml)?;
    println!("  Generated: api-rust/Cargo.toml");

    println!("✓ Rust Axum API generated");
    println!("  To run: cd {}/api-rust && cargo run", output.display());

    Ok(())
}
