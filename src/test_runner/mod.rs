//! Test runner module - E2E test execution with Playwright

use anyhow::Result;
use std::fs;
use std::path::PathBuf;

use topo::ast::{Expression, ObjectMember, Program, TestAssertion, TestHookDef, TestStatement, TestTarget, Declaration};
use topo::lexer::Lexer;
use topo::parser::Parser as TopoParser;

pub fn run_tests(headed: bool, ui: bool, file: Option<String>) -> Result<()> {
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

        let mut lexer = Lexer::new(&source)?;
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
    let mut output = String::new();
    output.push_str("import { test, expect } from '@playwright/test';\n\n");

    // Helper to generate test statements (test_num=0 for hooks)
    fn generate_test_statements(
        statements: &[TestStatement],
        output: &mut String,
        test_file_name: &str,
        test_num: usize,
        capture_counter: &mut usize,
    ) {
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
                        TestTarget::Url => match assertion {
                            TestAssertion::Equals { value } | TestAssertion::Value { value } => {
                                output.push_str(&format!(
                                    "  await expect(page).toHaveURL('{}');\n",
                                    value
                                ));
                            }
                            _ => {}
                        },
                        TestTarget::PageProperty { property } if property == "url" => {
                            match assertion {
                                TestAssertion::Equals { value }
                                | TestAssertion::Value { value } => {
                                    output.push_str(&format!(
                                        "  await expect(page).toHaveURL('{}');\n",
                                        value
                                    ));
                                }
                                _ => {}
                            }
                        }
                        _ => {
                            let selector = target_to_selector(target);
                            let locator = locator_with_first(&selector);
                            match assertion {
                                TestAssertion::Visible => {
                                    output.push_str(&format!(
                                        "  await expect({}).toBeVisible();\n",
                                        locator
                                    ));
                                }
                                TestAssertion::Hidden => {
                                    output.push_str(&format!(
                                        "  await expect({}).toBeHidden();\n",
                                        locator
                                    ));
                                }
                                TestAssertion::Disabled => {
                                    output.push_str(&format!(
                                        "  await expect({}).toBeDisabled();\n",
                                        locator
                                    ));
                                }
                                TestAssertion::Empty => {
                                    output.push_str(&format!(
                                        "  await expect({}).toBeEmpty();\n",
                                        locator
                                    ));
                                }
                                TestAssertion::HasText { value } => {
                                    output.push_str(&format!(
                                        "  await expect({}).toHaveText('{}');\n",
                                        locator, value
                                    ));
                                }
                                TestAssertion::Value { value } => {
                                    output.push_str(&format!(
                                        "  await expect({}).toHaveValue('{}');\n",
                                        locator, value
                                    ));
                                }
                                TestAssertion::Equals { value } => {
                                    output.push_str(&format!(
                                        "  await expect({}).toHaveText('{}');\n",
                                        locator, value
                                    ));
                                }
                                TestAssertion::Contains { value } => {
                                    output.push_str(&format!(
                                        "  await expect({}).toContainText('{}');\n",
                                        locator, value
                                    ));
                                }
                            }
                        }
                    }
                }
                TestStatement::Mock {
                    service,
                    method,
                    response,
                } => {
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
                            output.push_str(&format!(
                                "  await page.screenshot({{ path: 'screenshots/{}/{}' }});\n",
                                test_file_name, name
                            ));
                        }
                        None => {
                            output.push_str(&format!(
                                "  await page.screenshot({{ path: 'screenshots/{}/{}-{}.png' }});\n",
                                test_file_name, test_num, capture_counter
                            ));
                        }
                    }
                }
            }
        }
    }

    // Helper to generate hook (test_num=0 for hooks)
    fn generate_hook(
        hook_name: &str,
        hook_def: &TestHookDef,
        output: &mut String,
        test_file_name: &str,
        capture_counter: &mut usize,
    ) {
        output.push_str(&format!(
            "test.{}(async ({{ page }}) => {{\n",
            hook_name
        ));
        generate_test_statements(&hook_def.statements, output, test_file_name, 0, capture_counter);
        output.push_str("});\n\n");
    }

    let mut hook_capture_counter: usize = 0;

    // First pass: generate beforeAll/afterAll hooks (BeforeOnce/AfterOnce)
    for decl in &ast.declarations {
        match decl {
            Declaration::BeforeOnce(hook_def) => {
                generate_hook(
                    "beforeAll",
                    hook_def,
                    &mut output,
                    test_file_name,
                    &mut hook_capture_counter,
                );
            }
            Declaration::AfterOnce(hook_def) => {
                generate_hook(
                    "afterAll",
                    hook_def,
                    &mut output,
                    test_file_name,
                    &mut hook_capture_counter,
                );
            }
            _ => {}
        }
    }

    // Second pass: generate beforeEach/afterEach hooks
    for decl in &ast.declarations {
        match decl {
            Declaration::BeforeEach(hook_def) => {
                generate_hook(
                    "beforeEach",
                    hook_def,
                    &mut output,
                    test_file_name,
                    &mut hook_capture_counter,
                );
            }
            Declaration::AfterEach(hook_def) => {
                generate_hook(
                    "afterEach",
                    hook_def,
                    &mut output,
                    test_file_name,
                    &mut hook_capture_counter,
                );
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
            output.push_str(&format!(
                "{}('{}', async ({{ page }}) => {{\n",
                test_fn, test_def.name
            ));

            generate_test_statements(
                &test_def.statements,
                &mut output,
                test_file_name,
                test_num,
                &mut capture_counter,
            );

            output.push_str("});\n\n");
        }
    }

    Ok(output)
}

fn target_to_selector(target: &TestTarget) -> String {
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
        TestTarget::Submit => "button[type=\"submit\"]".to_string(),
        TestTarget::Button { content } => {
            format!("button:has-text(\"{}\")", content)
        }
        TestTarget::Url => {
            String::new() // Handled specially in expect
        }
        TestTarget::PageProperty { property: _ } => {
            String::new() // Handled specially in expect for page.url
        }
        TestTarget::Selector { selector } => selector.clone(),
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

fn expression_to_string(expr: &Expression) -> String {
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
                    ObjectMember::Property(p) => {
                        format!("{}: {}", p.key, expression_to_string(&p.value))
                    }
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
