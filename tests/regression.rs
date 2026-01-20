//! Regression tests for topo compiler
//!
//! These tests ensure that refactoring does not break existing functionality.
//! Run before and after refactoring to verify correctness.

use std::fs;
use std::path::Path;
use topo::codegen::JsCodegen;
use topo::parser::Parser;
use topo::{Lexer, TypeChecker};

// ============================================================================
// Helper functions
// ============================================================================

fn parse_source(source: &str) -> Result<topo::ast::Program, String> {
    let mut lexer = Lexer::new(source).map_err(|e| format!("{:?}", e))?;
    let tokens = lexer.tokenize().map_err(|e| format!("{:?}", e))?;
    let mut parser = Parser::new(tokens);
    parser.parse().map_err(|e| format!("{:?}", e))
}

fn generate_js(program: &topo::ast::Program) -> String {
    let mut codegen = JsCodegen::new();
    codegen.generate(program)
}

fn parse_and_generate(source: &str) -> Result<String, String> {
    let program = parse_source(source)?;
    Ok(generate_js(&program))
}

// ============================================================================
// Parse Tests - Verify various syntax constructs parse correctly
// ============================================================================

mod parse_tests {
    use super::*;

    #[test]
    fn test_parse_simple_component() {
        let source = r#"
            Button(label) -> {
                type: button
                content: label
                style: "px-4 py-2 bg-blue-500 text-white rounded"
            }
        "#;

        let program = parse_source(source).expect("Failed to parse simple component");
        assert_eq!(program.declarations.len(), 1);
        assert!(matches!(
            &program.declarations[0],
            topo::ast::Declaration::Component(_)
        ));
    }

    #[test]
    fn test_parse_component_with_children() {
        let source = r#"
            Card(title) -> {
                style: "p-4 bg-white rounded shadow"
                children: [
                    Header({ text: title }),
                    Content
                ]
            }
        "#;

        let program = parse_source(source).expect("Failed to parse component with children");
        assert_eq!(program.declarations.len(), 1);
    }

    #[test]
    fn test_parse_component_with_conditional_style() {
        let source = r#"
            Badge(variant) -> {
                type: text
                content: "Badge"
                style: variant == "primary"
                    ? "bg-blue-500 text-white"
                    : variant == "secondary"
                    ? "bg-gray-500 text-white"
                    : "bg-white text-gray-900"
            }
        "#;

        let program =
            parse_source(source).expect("Failed to parse component with conditional style");
        assert_eq!(program.declarations.len(), 1);
    }

    #[test]
    fn test_parse_store_definition() {
        let source = r#"
            Counter | {
                State {
                    count: 0
                    name: "test"
                    items: []
                }

                Actions {
                    Increment
                    Decrement
                    SetCount(value)
                    AddItem(item)
                }

                Reducers {
                    on Increment { count: $count + 1 }
                    on Decrement { count: $count - 1 }
                    on SetCount(value) { count: value }
                }
            }
        "#;

        let program = parse_source(source).expect("Failed to parse store definition");
        assert_eq!(program.declarations.len(), 1);
        assert!(matches!(
            &program.declarations[0],
            topo::ast::Declaration::Store(_)
        ));
    }

    #[test]
    fn test_parse_store_with_effects() {
        let source = r#"
            UserStore | {
                State {
                    user: null
                    isLoading: false
                }

                Actions {
                    FetchUser(id)
                    SetUser(user)
                }

                Effects {
                    on FetchUser(id) {
                        user: await http.get("/api/users/" + id)
                        dispatch: SetUser(user)
                    }
                }
            }
        "#;

        let program = parse_source(source).expect("Failed to parse store with effects");
        assert_eq!(program.declarations.len(), 1);
    }

    #[test]
    fn test_parse_api_service() {
        let source = r#"
            UserApi :: {
                rest: "/api/users"

                getAll: get("/") -> User[]
                getById: get("/:id") -> User
                create: post("/") -> User
                update: put("/:id") -> User
            }
        "#;

        let program = parse_source(source).expect("Failed to parse API service");
        assert_eq!(program.declarations.len(), 1);
        assert!(matches!(
            &program.declarations[0],
            topo::ast::Declaration::ApiService(_)
        ));
    }

    #[test]
    fn test_parse_api_service_with_block_syntax() {
        let source = r#"
            AuthApi :: {
                rest: "/api/auth"

                login: post("/login") {
                    request: LoginRequest
                    response: AuthToken
                    error: AuthError
                }

                logout: post("/logout") -> { success: boolean }
            }
        "#;

        let program =
            parse_source(source).expect("Failed to parse API service with block syntax");
        assert_eq!(program.declarations.len(), 1);
    }

    #[test]
    fn test_parse_import_statement() {
        let source = r#"
            import { Button, Card } from "./components/atoms.tp"
            import { UserStore } from "../stores/user.tp"

            Page -> {
                children: [Button, Card]
            }
        "#;

        let program = parse_source(source).expect("Failed to parse import statement");
        // Count imports and components separately
        let import_count = program
            .declarations
            .iter()
            .filter(|d| matches!(d, topo::ast::Declaration::Import(_)))
            .count();
        let component_count = program
            .declarations
            .iter()
            .filter(|d| matches!(d, topo::ast::Declaration::Component(_)))
            .count();
        assert_eq!(import_count, 2, "Should have 2 imports");
        assert_eq!(component_count, 1, "Should have 1 component");
    }

    #[test]
    fn test_parse_typed_component_params() {
        let source = r#"
            UserCard(user: User, showDetails: boolean) -> {
                type: div
                style: "p-4"
            }
        "#;

        let program = parse_source(source).expect("Failed to parse typed component params");
        if let topo::ast::Declaration::Component(comp) = &program.declarations[0] {
            assert_eq!(comp.params.len(), 2);
            assert!(comp.params[0].type_annotation.is_some());
            assert!(comp.params[1].type_annotation.is_some());
        } else {
            panic!("Expected component declaration");
        }
    }

    #[test]
    fn test_parse_typed_action_params() {
        let source = r#"
            Store | {
                State { value: 0 }
                Actions {
                    SetValue(value: number)
                    SetUser(user: User)
                    SetItems(items: Item[])
                }
            }
        "#;

        let program = parse_source(source).expect("Failed to parse typed action params");
        if let topo::ast::Declaration::Store(store) = &program.declarations[0] {
            let actions = store.actions.as_ref().unwrap();
            assert_eq!(actions.actions.len(), 3);
        } else {
            panic!("Expected store declaration");
        }
    }

    #[test]
    fn test_parse_optional_type() {
        let source = r#"
            Component(value: string?) -> {
                type: div
            }
        "#;

        let program = parse_source(source).expect("Failed to parse optional type");
        if let topo::ast::Declaration::Component(comp) = &program.declarations[0] {
            assert!(matches!(
                &comp.params[0].type_annotation,
                Some(topo::ast::TypeAnnotation::Optional { .. })
            ));
        }
    }

    #[test]
    fn test_parse_array_type() {
        let source = r#"
            List(items: Item[]) -> {
                type: div
            }
        "#;

        let program = parse_source(source).expect("Failed to parse array type");
        if let topo::ast::Declaration::Component(comp) = &program.declarations[0] {
            assert!(matches!(
                &comp.params[0].type_annotation,
                Some(topo::ast::TypeAnnotation::Array { .. })
            ));
        }
    }

    #[test]
    fn test_parse_union_type() {
        let source = r#"
            Component(value: string | number | boolean) -> {
                type: div
            }
        "#;

        let program = parse_source(source).expect("Failed to parse union type");
        if let topo::ast::Declaration::Component(comp) = &program.declarations[0] {
            assert!(matches!(
                &comp.params[0].type_annotation,
                Some(topo::ast::TypeAnnotation::Union { .. })
            ));
        }
    }

    #[test]
    fn test_parse_object_type() {
        let source = r#"
            Component(config: { name: string, value: number }) -> {
                type: div
            }
        "#;

        let program = parse_source(source).expect("Failed to parse object type");
        if let topo::ast::Declaration::Component(comp) = &program.declarations[0] {
            assert!(matches!(
                &comp.params[0].type_annotation,
                Some(topo::ast::TypeAnnotation::Object { .. })
            ));
        }
    }

    #[test]
    fn test_parse_lifecycle_hooks() {
        let source = r#"
            Component -> {
                init: Store.Load
                cleanup: Store.Reset
                type: div
            }
        "#;

        let program = parse_source(source).expect("Failed to parse lifecycle hooks");
        assert_eq!(program.declarations.len(), 1);
    }

    #[test]
    fn test_parse_event_handlers() {
        let source = r#"
            Button -> {
                type: button
                onClick: handleClick
                onMouseEnter: handleHover
                content: "Click me"
            }
        "#;

        let program = parse_source(source).expect("Failed to parse event handlers");
        assert_eq!(program.declarations.len(), 1);
    }

    #[test]
    fn test_parse_for_expression() {
        let source = r#"
            List(items) -> {
                children: items.for(item => {
                    ListItem({ text: item.name })
                })
            }
        "#;

        let program = parse_source(source).expect("Failed to parse for expression");
        assert_eq!(program.declarations.len(), 1);
    }

    #[test]
    fn test_parse_pipe_expression() {
        let source = r#"
            Price(amount) -> {
                type: text
                content: amount | currency("USD")
            }
        "#;

        let program = parse_source(source).expect("Failed to parse pipe expression");
        assert_eq!(program.declarations.len(), 1);
    }

    #[test]
    fn test_parse_store_reference() {
        let source = r#"
            Display -> {
                type: text
                content: $Counter.count
            }
        "#;

        let program = parse_source(source).expect("Failed to parse store reference");
        assert_eq!(program.declarations.len(), 1);
    }

    #[test]
    fn test_parse_multiple_declarations() {
        let source = r#"
            // Store definition
            AppStore | {
                State { count: 0 }
                Actions { Increment }
            }

            // Component using store
            Counter -> {
                type: div
                children: [
                    Display,
                    IncrementButton
                ]
            }

            Display -> {
                type: text
                content: $AppStore.count
            }

            IncrementButton -> {
                type: button
                onClick: AppStore.Increment
                content: "+"
            }
        "#;

        let program = parse_source(source).expect("Failed to parse multiple declarations");
        assert_eq!(program.declarations.len(), 4);
    }
}

// ============================================================================
// Codegen Tests - Verify code generation produces expected output
// ============================================================================

mod codegen_tests {
    use super::*;

    #[test]
    fn test_generate_simple_component() {
        let source = r#"
            Button(label) -> {
                type: button
                content: label
                style: "px-4 py-2"
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate JS");

        // Verify essential parts are present
        assert!(js.contains("Button"), "Should contain component name");
        assert!(
            js.contains("button") || js.contains("createElement"),
            "Should contain element type"
        );
    }

    #[test]
    fn test_generate_store() {
        let source = r#"
            Counter | {
                State {
                    count: 0
                }
                Actions {
                    Increment
                    Decrement
                }
                Reducers {
                    on Increment { count: $count + 1 }
                    on Decrement { count: $count - 1 }
                }
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate JS");

        // Verify store structure
        assert!(js.contains("Counter"), "Should contain store name");
        assert!(js.contains("count"), "Should contain state field");
        assert!(
            js.contains("Increment") || js.contains("increment"),
            "Should contain action"
        );
    }

    #[test]
    fn test_generate_component_with_children() {
        let source = r#"
            Card -> {
                style: "p-4"
                children: [
                    Header,
                    Content,
                    Footer
                ]
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate JS");
        assert!(js.contains("Card"), "Should contain component name");
    }

    #[test]
    fn test_generate_conditional_expression() {
        let source = r#"
            Status(active) -> {
                type: text
                content: active ? "Active" : "Inactive"
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate JS");
        assert!(
            js.contains("?") || js.contains("Active"),
            "Should contain conditional or its values"
        );
    }

    #[test]
    fn test_generate_for_loop() {
        let source = r#"
            List(items) -> {
                children: items.for(item => {
                    Item({ text: item })
                })
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate JS");
        assert!(
            js.contains("map") || js.contains("for"),
            "Should contain iteration"
        );
    }

    #[test]
    fn test_generate_event_handler() {
        let source = r#"
            Button -> {
                type: button
                onClick: handleClick
                content: "Click"
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate JS");
        assert!(
            js.contains("onClick") || js.contains("click") || js.contains("handleClick"),
            "Should contain event handler"
        );
    }

    #[test]
    fn test_generate_store_dispatch() {
        let source = r#"
            IncrementButton -> {
                type: button
                onClick: Counter.Increment
                content: "+"
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate JS");
        assert!(
            js.contains("Counter") || js.contains("Increment") || js.contains("dispatch"),
            "Should contain store dispatch"
        );
    }

    #[test]
    fn test_generate_cross_store_dispatch() {
        let source = r#"
            Home | {
                State {
                    step: "intro"
                }
                Actions {
                    SetStep(step)
                }
                Reducers {
                    on SetStep(step) { step: step }
                }
            }

            Wizard | {
                State {
                    currentPage: 0
                }
                Actions {
                    Start
                }
                Effects {
                    on Start {
                        dispatch: Home.SetStep("questions")
                    }
                }
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate JS");
        // Verify cross-store dispatch generates correct code
        assert!(
            js.contains("dispatch('Home', 'SetStep'"),
            "Should generate cross-store dispatch with target store 'Home': {}",
            js
        );
    }

    #[test]
    fn test_generate_api_service() {
        let source = r#"
            UserApi :: {
                rest: "/api/users"
                getAll: get("/") -> User[]
                create: post("/") -> User
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate JS");
        assert!(js.contains("UserApi"), "Should contain API service name");
        assert!(
            js.contains("/api/users") || js.contains("api"),
            "Should contain API path"
        );
    }

    #[test]
    fn test_generate_pipe_expression() {
        let source = r#"
            Price(amount) -> {
                type: text
                content: amount | currency
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate JS");
        assert!(js.contains("Price"), "Should contain component name");
    }

    #[test]
    fn test_generate_store_reference() {
        let source = r#"
            Display -> {
                type: text
                content: $Store.value
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate JS");
        assert!(
            js.contains("Store") || js.contains("value"),
            "Should contain store reference"
        );
    }
}

// ============================================================================
// TypeCheck Tests - Verify type checking works correctly
// ============================================================================

mod typecheck_tests {
    use super::*;

    #[test]
    fn test_typecheck_valid_store_reference() {
        let source = r#"
            Counter | {
                State { count: 0 }
            }

            Display -> {
                type: text
                content: $Counter.count
            }
        "#;

        let program = parse_source(source).expect("Failed to parse");
        let mut checker = TypeChecker::new();
        let errors = checker.check(&program);
        assert!(errors.is_empty(), "Should have no type errors: {:?}", errors);
    }

    #[test]
    fn test_typecheck_invalid_store_reference() {
        let source = r#"
            Display -> {
                type: text
                content: $NonExistent.value
            }
        "#;

        let program = parse_source(source).expect("Failed to parse");
        let mut checker = TypeChecker::new();
        let errors = checker.check(&program);
        assert!(!errors.is_empty(), "Should have type error for unknown store");
    }

    #[test]
    fn test_typecheck_invalid_property_reference() {
        let source = r#"
            Counter | {
                State { count: 0 }
            }

            Display -> {
                type: text
                content: $Counter.nonexistent
            }
        "#;

        let program = parse_source(source).expect("Failed to parse");
        let mut checker = TypeChecker::new();
        let errors = checker.check(&program);
        assert!(
            !errors.is_empty(),
            "Should have type error for unknown property"
        );
    }
}

// ============================================================================
// End-to-End Tests - Test full pipeline with demo files
// ============================================================================

mod e2e_tests {
    use super::*;

    fn get_demo_dir() -> Option<std::path::PathBuf> {
        let demo_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("demo");
        if demo_dir.exists() {
            Some(demo_dir)
        } else {
            None
        }
    }

    #[test]
    fn test_parse_demo_atoms() {
        let Some(demo_dir) = get_demo_dir() else {
            println!("Demo directory not found, skipping test");
            return;
        };

        let atoms_dir = demo_dir.join("components/atoms");
        if !atoms_dir.exists() {
            println!("Atoms directory not found, skipping test");
            return;
        }

        for entry in fs::read_dir(&atoms_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "tp") {
                let source = fs::read_to_string(&path).unwrap();
                let result = parse_source(&source);
                assert!(
                    result.is_ok(),
                    "Failed to parse {}: {:?}",
                    path.display(),
                    result.err()
                );
            }
        }
    }

    #[test]
    fn test_parse_demo_services() {
        let Some(demo_dir) = get_demo_dir() else {
            println!("Demo directory not found, skipping test");
            return;
        };

        let services_dir = demo_dir.join("services");
        if !services_dir.exists() {
            println!("Services directory not found, skipping test");
            return;
        }

        for entry in fs::read_dir(&services_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "tp") {
                let source = fs::read_to_string(&path).unwrap();
                let result = parse_source(&source);
                assert!(
                    result.is_ok(),
                    "Failed to parse {}: {:?}",
                    path.display(),
                    result.err()
                );
            }
        }
    }

    #[test]
    fn test_parse_demo_pages() {
        let Some(demo_dir) = get_demo_dir() else {
            println!("Demo directory not found, skipping test");
            return;
        };

        let pages_dir = demo_dir.join("pages");
        if !pages_dir.exists() {
            println!("Pages directory not found, skipping test");
            return;
        }

        fn check_tp_files(dir: &Path) {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        check_tp_files(&path);
                    } else if path.extension().map_or(false, |ext| ext == "tp") {
                        let source = fs::read_to_string(&path).unwrap();
                        let result = parse_source(&source);
                        assert!(
                            result.is_ok(),
                            "Failed to parse {}: {:?}",
                            path.display(),
                            result.err()
                        );
                    }
                }
            }
        }

        check_tp_files(&pages_dir);
    }

    #[test]
    fn test_generate_demo_atoms() {
        let Some(demo_dir) = get_demo_dir() else {
            println!("Demo directory not found, skipping test");
            return;
        };

        let atoms_dir = demo_dir.join("components/atoms");
        if !atoms_dir.exists() {
            println!("Atoms directory not found, skipping test");
            return;
        }

        for entry in fs::read_dir(&atoms_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "tp") {
                let source = fs::read_to_string(&path).unwrap();
                let result = parse_and_generate(&source);
                assert!(
                    result.is_ok(),
                    "Failed to generate JS for {}: {:?}",
                    path.display(),
                    result.err()
                );

                // Verify output is non-empty
                let js = result.unwrap();
                assert!(
                    !js.trim().is_empty(),
                    "Generated JS should not be empty for {}",
                    path.display()
                );
            }
        }
    }
}

// ============================================================================
// Error Handling Tests - Verify error cases are handled correctly
// ============================================================================

mod error_tests {
    use super::*;

    #[test]
    fn test_parse_error_missing_arrow() {
        let source = r#"
            Button(label) {
                type: button
            }
        "#;

        let result = parse_source(source);
        assert!(result.is_err(), "Should fail to parse missing arrow");
    }

    #[test]
    fn test_parse_error_unclosed_brace() {
        let source = r#"
            Button -> {
                type: button
        "#;

        let result = parse_source(source);
        assert!(result.is_err(), "Should fail to parse unclosed brace");
    }

    #[test]
    fn test_parse_error_invalid_store_syntax() {
        let source = r#"
            Counter | {
                State {
                    count: 0
                Actions {
                    Increment
                }
            }
        "#;

        let result = parse_source(source);
        assert!(result.is_err(), "Should fail to parse invalid store syntax");
    }

    #[test]
    fn test_parse_empty_source() {
        let source = "";
        let result = parse_source(source);
        // Empty source should parse successfully with no declarations
        assert!(result.is_ok(), "Empty source should parse successfully");
        let program = result.unwrap();
        assert!(
            program.declarations.is_empty(),
            "Empty source should have no declarations"
        );
    }

    #[test]
    fn test_parse_comment_only() {
        let source = r#"
            // This is a comment
            // Another comment
        "#;

        let result = parse_source(source);
        assert!(
            result.is_ok(),
            "Comment-only source should parse successfully"
        );
    }
}

// ============================================================================
// Snapshot Tests - Capture current behavior for regression detection
// ============================================================================

mod snapshot_tests {
    use super::*;

    /// Helper to create a deterministic snapshot of generated code
    fn normalize_js(js: &str) -> String {
        js.lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn test_snapshot_simple_component() {
        let source = r#"
            SimpleButton -> {
                type: button
                content: "Click"
                style: "btn"
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate");
        let normalized = normalize_js(&js);

        // Store a hash or key characteristics instead of exact match
        assert!(
            normalized.contains("SimpleButton"),
            "Component name preserved"
        );
        assert!(
            normalized.contains("button") || normalized.contains("Button"),
            "Element type preserved"
        );
        assert!(normalized.contains("Click"), "Content preserved");
    }

    #[test]
    fn test_snapshot_store_with_actions() {
        let source = r#"
            TestStore | {
                State {
                    value: 0
                    name: "test"
                }
                Actions {
                    SetValue(v)
                    Reset
                }
                Reducers {
                    on SetValue(v) { value: v }
                    on Reset { value: 0, name: "" }
                }
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate");
        let normalized = normalize_js(&js);

        assert!(normalized.contains("TestStore"), "Store name preserved");
        assert!(normalized.contains("value"), "State field preserved");
        assert!(
            normalized.contains("SetValue") || normalized.contains("setValue"),
            "Action preserved"
        );
    }

    #[test]
    fn test_snapshot_api_service() {
        let source = r#"
            TestApi :: {
                rest: "/api/test"

                list: get("/") -> Item[]
                getOne: get("/:id") -> Item
                createItem: post("/") -> Item
            }
        "#;

        let js = parse_and_generate(source).expect("Failed to generate");
        let normalized = normalize_js(&js);

        assert!(normalized.contains("TestApi"), "API name preserved");
        assert!(
            normalized.contains("/api/test") || normalized.contains("api"),
            "Base path preserved"
        );
    }
}
