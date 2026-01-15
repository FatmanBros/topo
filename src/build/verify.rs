//! Build verification module - validates generated JS at build time
//!
//! This module checks for JavaScript runtime errors in the generated code
//! by launching a headless browser and detecting page errors.

use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Verify the built application by checking for JS runtime errors
pub fn verify_build(output_dir: &Path, base_path: &str) -> Result<()> {
    println!("  Verifying build...");

    // Check if playwright is available
    let has_playwright = Command::new("npx")
        .args(["playwright", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_playwright {
        println!("  ⚠ Playwright not installed, skipping verification");
        println!("    Run 'npm install -D @playwright/test && npx playwright install chromium' to enable");
        return Ok(());
    }

    // Create a temporary verification script
    let verify_script = generate_verify_script(base_path);
    let script_path = output_dir.join("__verify__.mjs");
    fs::write(&script_path, &verify_script)?;

    // Start a simple HTTP server and run verification
    let port = find_available_port();

    // Use node to run the verification script
    let result = run_verification(output_dir, &script_path, port, base_path);

    // Clean up
    let _ = fs::remove_file(&script_path);

    result
}

fn generate_verify_script(base_path: &str) -> String {
    let path = if base_path.is_empty() { "/" } else { base_path };
    format!(r#"
import {{ chromium }} from 'playwright';
import {{ createServer }} from 'http';
import {{ readFileSync, existsSync }} from 'fs';
import {{ join, extname }} from 'path';

const PORT = parseInt(process.argv[2] || '3456');
const DIST_DIR = process.argv[3] || './dist';
const BASE_PATH = '{}';

// Simple static file server
const mimeTypes = {{
    '.html': 'text/html',
    '.js': 'application/javascript',
    '.css': 'text/css',
    '.json': 'application/json',
    '.png': 'image/png',
    '.jpg': 'image/jpeg',
    '.svg': 'image/svg+xml',
    '.ico': 'image/x-icon',
}};

const server = createServer((req, res) => {{
    let url = req.url || '/';

    // Remove base path prefix
    if (BASE_PATH && url.startsWith(BASE_PATH)) {{
        url = url.slice(BASE_PATH.length) || '/';
    }}

    // Default to index.html for directory requests
    if (url.endsWith('/')) {{
        url += 'index.html';
    }}

    const filePath = join(DIST_DIR, url);

    if (!existsSync(filePath)) {{
        // Try index.html for SPA routing
        const indexPath = join(DIST_DIR, 'index.html');
        if (existsSync(indexPath)) {{
            res.writeHead(200, {{ 'Content-Type': 'text/html' }});
            res.end(readFileSync(indexPath));
            return;
        }}
        res.writeHead(404);
        res.end('Not found');
        return;
    }}

    const ext = extname(filePath);
    const contentType = mimeTypes[ext] || 'application/octet-stream';

    res.writeHead(200, {{ 'Content-Type': contentType }});
    res.end(readFileSync(filePath));
}});

async function verify() {{
    const errors = [];

    server.listen(PORT);

    try {{
        const browser = await chromium.launch({{ headless: true }});
        const page = await browser.newPage();

        // Capture page errors (JS runtime errors)
        page.on('pageerror', (error) => {{
            errors.push({{
                type: 'runtime',
                message: error.message,
                stack: error.stack,
            }});
        }});

        // Capture console errors
        page.on('console', (msg) => {{
            if (msg.type() === 'error') {{
                errors.push({{
                    type: 'console',
                    message: msg.text(),
                }});
            }}
        }});

        // Navigate to the app
        const url = `http://localhost:${{PORT}}${{BASE_PATH || '/'}}`;
        await page.goto(url, {{ waitUntil: 'networkidle', timeout: 30000 }});

        // Wait a bit for any async errors
        await page.waitForTimeout(500);

        await browser.close();
    }} finally {{
        server.close();
    }}

    if (errors.length > 0) {{
        console.error('\\n❌ Build verification failed!');
        console.error('\\nJavaScript errors detected:\\n');
        for (const err of errors) {{
            console.error(`  [${{err.type}}] ${{err.message}}`);
            if (err.stack) {{
                // Show first few lines of stack trace
                const stackLines = err.stack.split('\\n').slice(0, 3);
                for (const line of stackLines) {{
                    console.error(`    ${{line}}`);
                }}
            }}
        }}
        process.exit(1);
    }}

    console.log('  ✓ No JavaScript errors detected');
    process.exit(0);
}}

verify().catch(err => {{
    console.error('Verification error:', err.message);
    server.close();
    process.exit(1);
}});
"#, path)
}

fn find_available_port() -> u16 {
    // Try to find an available port starting from 3456
    for port in 3456..3556 {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    3456 // Fallback
}

fn run_verification(output_dir: &Path, script_path: &Path, port: u16, _base_path: &str) -> Result<()> {
    let output = Command::new("node")
        .arg(script_path)
        .arg(port.to_string())
        .arg(output_dir)
        .output()?;

    // Print stdout/stderr
    if !output.stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }

    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!("Build verification failed - JavaScript errors detected in generated code")
    }
}
