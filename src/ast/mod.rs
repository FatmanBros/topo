//! AST (Abstract Syntax Tree) definitions for topo language
//!
//! The topo language has 7 types of definitions:
//! - Component: `Name -> { }` - UI components
//! - Method: `Name { }` - Logic/functions
//! - ApiService: `Name :: { }` - API service definitions
//! - Store: `Name | { }` - State management
//! - Guard: `Name ? { }` - Route guards
//! - Resolver: `Name ! { }` - Data pre-fetching before route navigation
//! - Directive: `Name @ { }` - Custom DOM element behaviors

use serde::{Deserialize, Serialize};

/// Root node of the AST
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub declarations: Vec<Declaration>,
}

/// Top-level declarations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Declaration {
    /// Import statement: `import "path/to/file.tp"` or `import { Name } from "path"`
    Import(ImportDef),

    /// Component definition: `Name -> { ... }`
    Component(ComponentDef),

    /// Method definition: `Name { ... }`
    Method(MethodDef),

    /// API Service definition: `Name :: { ... }`
    ApiService(ApiServiceDef),

    /// Store definition: `Name | { ... }`
    Store(StoreDef),

    /// Theme definition: `Name * { ... }`
    Theme(ThemeDef),

    /// Test definition: `Test("name") { ... }`
    Test(TestDef),

    /// BeforeEach hook: `BeforeEach { ... }`
    BeforeEach(TestHookDef),

    /// AfterEach hook: `AfterEach { ... }`
    AfterEach(TestHookDef),

    /// BeforeOnce hook (runs once before all tests): `BeforeOnce { ... }`
    BeforeOnce(TestHookDef),

    /// AfterOnce hook (runs once after all tests): `AfterOnce { ... }`
    AfterOnce(TestHookDef),

    /// Guard definition: `Name ? { ... }` or `Name | activate { }` / `Name | deactivate { }`
    Guard(GuardDef),

    /// Guard setup: `GuardSetup { ... }`
    GuardSetup(GuardSetupDef),

    /// Routes definition: `Routes { ... }` or `RouteName { ... }` for subroutes
    Routes(RoutesDef),

    /// Pure function definition: `Name(params) -> expression`
    Function(FunctionDef),

    /// Resolver definition: `Name ! { fetch: expr, fallback: value }`
    Resolver(ResolverDef),

    /// Directive definition: `Name @ { onMount, onDestroy }`
    Directive(DirectiveDef),

    /// Schema definition: `Schema { tables... }`
    Schema(SchemaDef),

    /// Repository definition: `Name :: tableName { methods... }`
    Repository(RepositoryDef),
}

// ============================================================================
// Import Definition
// ============================================================================

/// Import statement
/// - `import "path/to/file.tp"` - import all exports
/// - `import { Name, Other } from "path/to/file.tp"` - named imports
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportDef {
    /// The path to the file to import (relative or absolute)
    pub path: String,
    /// Specific names to import (empty = import all)
    pub names: Vec<String>,
}

// ============================================================================
// Component Definition (->)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentDef {
    pub name: String,
    pub params: Vec<TypedParam>,
    pub properties: Vec<Property>,
    /// Lifecycle hook: called when component is mounted
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init: Option<Expression>,
    /// Lifecycle hook: called when component is destroyed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destroy: Option<Expression>,
    /// Component alias: `Alias(args) -> Base(args, defaultValue)`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<ComponentAlias>,
    /// Guards applied to this component: `Name -> Guard1, Guard2 { }`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guards: Vec<String>,
    /// Directives applied to this component: `@Focus`, `@Tooltip("text")`
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directives: Vec<DirectiveUsage>,
}

/// Component alias definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentAlias {
    /// Base component name
    pub base: String,
    /// Arguments to pass to the base component
    pub args: Vec<Expression>,
}

// ============================================================================
// Function Definition (->)
// ============================================================================

/// Pure function definition: `Name(params) -> expression`
/// Unlike components, functions return a value directly without creating a DOM element.
/// Functions can be imported and used across files.
///
/// Example:
/// ```topo
/// // Define a helper function
/// getInitials(name) -> name ? name[0] : "?"
///
/// // Define with type annotations
/// formatPrice(value: number) -> "$" + value.toFixed(2)
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionDef {
    /// Function name
    pub name: String,
    /// Parameters with optional type annotations
    pub params: Vec<TypedParam>,
    /// The expression that computes the return value
    pub body: Expression,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedParam {
    pub name: String,
    pub type_annotation: Option<TypeAnnotation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Property {
    pub key: String,
    pub value: Expression,
    /// Type annotation for this property (e.g., name: string = "")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<TypeAnnotation>,
    /// Validation annotations for this property
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
}

/// Object member: either a property or a spread expression
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ObjectMember {
    /// Regular property: `key: value`
    Property(Property),
    /// Spread expression: `...expr`
    Spread { expr: Expression },
}

// ============================================================================
// Annotations (for validation)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub name: String,
    pub args: Vec<Expression>,
}

// ============================================================================
// Theme Definition (*)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeDef {
    pub name: String,
    pub properties: Vec<Property>,
}

// ============================================================================
// Guard Definition (?)
// ============================================================================

/// Guard definition: `Name ? { check: expr, redirect: "/path" }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuardDef {
    pub name: String,
    /// Guard type: activate (entering route) or deactivate (leaving route)
    pub guard_type: GuardType,
    /// Body statements (for new style guards with Effects-like syntax)
    pub body: Vec<Statement>,
    /// Expression that returns boolean - true allows navigation, false blocks it (legacy)
    pub check: Option<Expression>,
    /// Path to redirect to when guard blocks navigation (legacy)
    pub redirect: Option<String>,
}

/// Guard setup: configures which guards apply to which routes
/// ```text
/// GuardSetup {
///     global: [AuthGuard]
///     routes: {
///         "/admin/*": AdminGuard
///         "/public/*": none
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuardSetupDef {
    /// Guards that apply to all routes
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub global: Vec<String>,
    /// Route-specific guard configurations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<RouteGuard>,
}

/// Route-specific guard configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteGuard {
    /// Route pattern (supports wildcards like "/admin/*")
    pub pattern: String,
    /// Guard name or "none" to disable guards for this route
    pub guard: Option<String>,
}

// ============================================================================
// Resolver Definition (!)
// ============================================================================

/// Resolver definition: `Name ! { fetch: expr, fallback: value, cache?: ms }`
/// Resolvers pre-fetch data before route navigation completes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolverDef {
    /// Resolver name
    pub name: String,
    /// Parameters (e.g., `UserResolver(id)` -> ["id"])
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<String>,
    /// Fetch expression - the async data fetching logic
    pub fetch: Expression,
    /// Fallback value when fetch fails
    pub fallback: Expression,
    /// Optional cache duration in milliseconds
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<u64>,
}

/// Resolver reference in routes with optional arguments
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolverRef {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

// ============================================================================
// Directive Definition (@)
// ============================================================================

/// Directive definition: `Name @ { onMount: fn, onDestroy: fn }`
/// Directives attach custom behavior to DOM elements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirectiveDef {
    /// Directive name
    pub name: String,
    /// Parameters (e.g., `Tooltip(text)` -> ["text"])
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<String>,
    /// Called when element is mounted to DOM
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_mount: Option<Expression>,
    /// Called when element is removed from DOM
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_destroy: Option<Expression>,
    /// Called when directive value changes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_update: Option<Expression>,
}

/// Directive usage in a component: `@DirectiveName` or `@DirectiveName(args)`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirectiveUsage {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<Expression>,
}

// ============================================================================
// Method Definition ({})
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodDef {
    pub name: String,
    pub body: Expression,
}

// ============================================================================
// API Service Definition (::)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiServiceDef {
    pub name: String,
    pub rest: Option<String>,
    pub endpoints: Vec<Endpoint>,
    pub headers: Option<Vec<Property>>,
    pub auth: Option<Expression>,
    pub timeout: Option<u32>,
    /// WebSocket/SSE subscription URL
    pub subscribe: Option<String>,
    /// Event handlers for subscription
    pub event_handlers: Vec<EventHandler>,
    /// Server-side implementation block
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerBlock>,
}

// ============================================================================
// Server-side Implementation (server block)
// ============================================================================

/// Server-side implementation block for API services
/// ```topo
/// UserApi :: {
///     rest: "/api/users"
///     getById: get("/:id") -> User
///
///     server {
///         on getById(id, ctx) {
///             user: await ctx.db.query("SELECT * FROM users WHERE id = ?", id)
///             return: user
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerBlock {
    /// Context type definition (DB, Auth, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<TypeAnnotation>,
    /// Middleware chain
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub middleware: Vec<Expression>,
    /// Endpoint handlers
    pub handlers: Vec<ServerHandler>,
}

/// Server-side handler for an endpoint
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerHandler {
    /// Endpoint name (must match an endpoint definition)
    pub endpoint: String,
    /// Handler parameters
    pub params: Vec<TypedParam>,
    /// Context parameter name (e.g., "ctx")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctx_param: Option<String>,
    /// Handler body statements
    pub body: Vec<ServerStatement>,
}

/// Statements allowed in server handlers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerStatement {
    /// Variable assignment: `name: value`
    Assignment {
        name: String,
        value: Expression,
    },
    /// Return statement: `return: expr`
    Return {
        value: Expression,
    },
    /// Throw error: `throw: ErrorType("message")`
    Throw {
        error_type: String,
        message: Expression,
    },
    /// If statement
    If {
        condition: Expression,
        then_block: Vec<ServerStatement>,
        else_block: Option<Vec<ServerStatement>>,
    },
    /// Try-catch block
    TryCatch {
        try_block: Vec<ServerStatement>,
        catch_param: String,
        catch_block: Vec<ServerStatement>,
    },
    /// Expression statement
    Expression(Expression),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Endpoint {
    pub name: String,
    pub method: HttpMethod,
    pub path: String,
    /// Request body type (for POST, PUT, PATCH)
    pub request_type: Option<TypeAnnotation>,
    /// Response type
    pub response_type: Option<TypeAnnotation>,
    /// Error response type
    pub error_type: Option<TypeAnnotation>,
    /// URL parameters type (e.g., { id: number } for /users/:id)
    pub params_type: Option<TypeAnnotation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

/// Event handler for WebSocket/SSE subscriptions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventHandler {
    /// Event type: message, error, open, close
    pub event: EventType,
    /// Action to dispatch when event occurs
    pub action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventType {
    Message,
    Error,
    Open,
    Close,
}

// ============================================================================
// Store Definition (|)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoreDef {
    /// Store name. None for anonymous stores (derived from filename)
    pub name: Option<String>,
    pub state: Option<StateBlock>,
    pub actions: Option<ActionsBlock>,
    /// Commands are public actions accessible from outside (e.g., from Templates)
    pub commands: Option<CommandsBlock>,
    pub reducers: Option<ReducersBlock>,
    pub effects: Option<EffectsBlock>,
    pub selectors: Option<SelectorsBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateBlock {
    pub fields: Vec<Property>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionsBlock {
    pub actions: Vec<ActionDef>,
}

/// Commands block - public actions accessible from outside the store
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandsBlock {
    pub commands: Vec<ActionDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionDef {
    pub name: String,
    pub params: Vec<Param>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub type_annotation: Option<TypeAnnotation>,
}

// ============================================================================
// Type Annotations
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TypeAnnotation {
    /// Primitive types: string, number, boolean
    Primitive { name: String },

    /// Array type: string[], number[]
    Array { element_type: Box<TypeAnnotation> },

    /// Optional type: string?
    Optional { inner_type: Box<TypeAnnotation> },

    /// Object type: { name: string, age: number }
    Object { fields: Vec<TypedField> },

    /// Union type: string | number
    Union { types: Vec<TypeAnnotation> },

    /// Reference to another type/store: User, LoginForm
    Reference { name: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedField {
    pub name: String,
    pub type_annotation: TypeAnnotation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReducersBlock {
    pub handlers: Vec<ReducerHandler>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReducerHandler {
    pub action: String,
    pub params: Vec<String>,
    pub body: Vec<Property>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectsBlock {
    pub handlers: Vec<EffectHandler>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectHandler {
    pub action: String,
    pub params: Vec<String>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectorsBlock {
    pub selectors: Vec<SelectorDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectorDef {
    pub name: String,
    pub body: Expression,
}

// ============================================================================
// Routes Definition
// ============================================================================

/// Routes definition: `Routes { ... }` or named subroute `DocsRoute { ... }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutesDef {
    /// Name of the routes block (e.g., "Routes", "DocsRoute")
    pub name: String,
    /// Route entries
    pub routes: Vec<RouteEntry>,
    /// Guards configuration
    pub guards: Option<RoutesGuardsConfig>,
}

/// A single route entry
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteEntry {
    /// Route name (e.g., "home", "userDetail")
    pub name: String,
    /// Route parameters (e.g., ["id"] for userDetail(id))
    pub params: Vec<String>,
    /// Route configuration
    pub config: RouteConfig,
    /// Route metadata (title, description)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<RouteMeta>,
}

/// Route configuration - path only, path with guards, resolvers, or subroute reference
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RouteConfig {
    /// Simple path: `"/dashboard"`
    Path { path: String },
    /// Path with guards: `"/dashboard", [guard1, guard2]` (! prefix for canDeactivate)
    PathWithGuards {
        path: String,
        guards: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        can_deactivate: Vec<String>,
    },
    /// Path with resolvers: `"/dashboard", {resolver1, resolver2}`
    PathWithResolvers { path: String, resolvers: Vec<ResolverRef> },
    /// Path with guards and resolvers: `"/users/{id}", [isAuth, !unsaved], {UserResolver(id)}`
    PathWithGuardsAndResolvers {
        path: String,
        guards: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        can_deactivate: Vec<String>,
        resolvers: Vec<ResolverRef>,
    },
    /// Subroute reference: `"/docs" -> DocsRoute`
    SubRoute { path: String, route_ref: String },
}

/// Route modifiers - guards, resolvers, and canDeactivate
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RouteModifiers {
    /// Activate guards (executed when entering route)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guards: Vec<String>,
    /// Resolvers (data pre-fetching before navigation)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolvers: Vec<ResolverRef>,
    /// Deactivate guards (executed when leaving route)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub can_deactivate: Vec<String>,
}

/// Guards configuration for a Routes block
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutesGuardsConfig {
    /// Global guards applied to all routes in this block
    pub global: Vec<String>,
    /// Routes that skip global guards
    pub skip: Vec<String>,
}

/// Route metadata - title, description, etc.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RouteMeta {
    /// Page title (document.title)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Page description (meta tag)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ============================================================================
// Guard Definition (with activate/deactivate)
// ============================================================================

/// Guard type: when the guard is executed
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GuardType {
    /// Execute when entering a route (default)
    Activate,
    /// Execute when leaving a route
    Deactivate,
}

// ============================================================================
// Statements (for Effects)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Statement {
    /// Variable assignment: `name: value`
    Assignment {
        name: String,
        value: Expression,
    },

    /// Dispatch action: `dispatch: ActionName` or `dispatch: StoreName.ActionName`
    Dispatch {
        store: Option<String>,
        action: String,
        args: Vec<Expression>,
    },

    /// Navigate to path: `navigate: "/path"`
    Navigate {
        path: Expression,
    },

    /// Try-catch block
    TryCatch {
        try_block: Vec<Statement>,
        catch_param: String,
        catch_block: Vec<Statement>,
    },

    /// If statement: `if (condition) { ... } else { ... }`
    If {
        condition: Expression,
        then_block: Vec<Statement>,
        else_block: Option<Vec<Statement>>,
    },

    /// Await expression as statement
    Await {
        expr: Expression,
    },

    /// Expression statement
    Expression(Expression),
}

// ============================================================================
// Expressions
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Expression {
    /// String literal: `"hello"`
    String { value: String },

    /// Number literal: `42`, `3.14`
    Number { value: f64 },

    /// Boolean literal: `true`, `false`
    Boolean { value: bool },

    /// Null literal
    Null,

    /// Array: `[a, b, c]`
    Array { elements: Vec<Expression> },

    /// For loop: `items.for(item => { ... })` or `items.for((item, index) => { ... })`
    ForIn {
        item: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<String>,
        items: Box<Expression>,
        body: Box<Expression>,
    },

    /// Object: `{ key: value }` or `{ ...spread, key: value }`
    Object { members: Vec<ObjectMember> },

    /// Identifier: `foo`
    Identifier { name: String },

    /// Store reference: `$Store.path`
    Reference { store: String, path: Vec<String> },

    /// Route reference for navigation: `.home`, `.docs.installation`
    RouteRef { path: Vec<String> },

    /// Action reference: `Store.Action` or `Store.Action(args)`
    ActionRef {
        store: String,
        action: String,
        args: Vec<Expression>,
    },

    /// API call: `getAll()`, `getById(id)`
    ApiCall {
        method: String,
        args: Vec<Expression>,
    },

    /// Binary operation: `a + b`, `a == b`
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },

    /// Unary operation: `!a`, `-a`
    UnaryOp {
        op: UnaryOperator,
        operand: Box<Expression>,
    },

    /// Member access: `obj.prop`
    MemberAccess {
        object: Box<Expression>,
        property: String,
    },

    /// Index access: `obj[key]` or `arr[0]`
    IndexAccess {
        object: Box<Expression>,
        index: Box<Expression>,
    },

    /// Function call: `fn(args)`
    Call {
        callee: Box<Expression>,
        args: Vec<Expression>,
    },

    /// Await expression: `await expr`
    Await { expr: Box<Expression> },

    /// Spread expression: `...arr`
    Spread { expr: Box<Expression> },

    /// Pipe expression: `value | pipeName` or `value | pipeName(arg1, arg2)`
    Pipe {
        value: Box<Expression>,
        pipe_name: String,
        args: Vec<Expression>,
    },

    /// Conditional (ternary) expression: `condition ? then : else`
    Conditional {
        condition: Box<Expression>,
        then_branch: Box<Expression>,
        else_branch: Box<Expression>,
    },

    /// SQL template literal: ctx.sql`SELECT * FROM users WHERE id = ${id}`
    SqlTemplate {
        parts: Vec<String>,
        expressions: Vec<Expression>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOperator {
    // Arithmetic
    Add,      // +
    Sub,      // -
    Mul,      // *
    Div,      // /
    Mod,      // %

    // Comparison
    Eq,       // ==
    Ne,       // !=
    Lt,       // <
    Le,       // <=
    Gt,       // >
    Ge,       // >=

    // Logical
    And,      // &&
    Or,       // ||
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Not,      // !
    Neg,      // -
}

// ============================================================================
// Test Definition
// ============================================================================

/// Test definition: `Test("test name") { ... }` or `xTest("test name") { ... }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestDef {
    pub name: String,
    pub statements: Vec<TestStatement>,
    /// If true, this test is skipped (xTest)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub skip: bool,
}

/// Test hook definition: `BeforeEach { ... }` or `AfterEach { ... }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestHookDef {
    pub statements: Vec<TestStatement>,
}

/// Test statement types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TestStatement {
    /// Navigate to a URL: `goto "/path"`
    Goto { path: String },

    /// Click an element: `click submit`, `click text "Login"`, `click $Store.field`
    Click { target: TestTarget },

    /// Fill an input: `fill $Store.field "value"`
    Fill { target: TestTarget, value: Expression },

    /// Type text: `type $Store.field "value"`
    Type { target: TestTarget, value: Expression },

    /// Expect assertion: `expect $Store.field visible`, `expect url "/path"`
    Expect { target: TestTarget, assertion: TestAssertion },

    /// Mock an API: `mock Service.method -> data`
    Mock {
        service: String,
        method: String,
        response: Expression,
    },

    /// Wait for time: `wait 1000`
    Wait { ms: u32 },

    /// Capture screenshot: `capture()` or `capture("filename")`
    Capture { filename: Option<String> },
}

/// Target for test operations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TestTarget {
    /// Store field reference: `$LoginForm.email`
    Field { store: String, field: String },

    /// Text content: `text "Login"`
    Text { content: String },

    /// Submit button
    Submit,

    /// Button with text
    Button { content: String },

    /// URL (legacy)
    Url,

    /// Page property: `page.url`
    PageProperty { property: String },

    /// CSS selector
    Selector { selector: String },
}

/// Test assertions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TestAssertion {
    /// Element is visible: { visible }
    Visible,

    /// Element is hidden: { hidden }
    Hidden,

    /// Element is disabled: { disabled }
    Disabled,

    /// Element is empty: { empty }
    Empty,

    /// Element has text value
    HasText { value: String },

    /// Value equals (string): expect(target, "value")
    Value { value: String },

    /// URL equals
    Equals { value: String },

    /// Element contains text
    Contains { value: String },
}

// ============================================================================
// Source Location (for error reporting)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

impl Default for Span {
    fn default() -> Self {
        Self {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        }
    }
}

// ============================================================================
// Schema Definition (Database Schema)
// ============================================================================

/// Schema definition: `Schema { tables... }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaDef {
    pub tables: Vec<TableDef>,
}

/// Table definition within a schema
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableDef {
    pub name: String,
    pub columns: Vec<ColumnDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<Relation>,
}

/// Table relation definitions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Relation {
    /// One-to-many: @hasMany(posts)
    HasMany {
        target: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        foreign_key: Option<String>,
    },
    /// One-to-one: @hasOne(profile)
    HasOne {
        target: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        foreign_key: Option<String>,
    },
    /// Many-to-one: @belongsTo(users)
    BelongsTo {
        target: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        foreign_key: Option<String>,
    },
    /// Many-to-many: @manyToMany(tags, post_tags)
    ManyToMany {
        target: String,
        through: String,
    },
}

/// Column definition within a table
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnDef {
    pub name: String,
    pub column_type: ColumnType,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub nullable: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<ColumnConstraint>,
}

/// Column data types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColumnType {
    String,
    Number,
    Boolean,
    Datetime,
    Json,
    Blob,
}

/// Column constraints
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ColumnConstraint {
    /// Primary key: @primary
    Primary,
    /// Unique constraint: @unique
    Unique,
    /// Foreign key reference: @references(table.column)
    References { table: String, column: String },
    /// Default value: @default(value)
    Default { value: Expression },
    /// Auto-increment: @autoincrement
    AutoIncrement,
}

// ============================================================================
// SQL Template Expression
// ============================================================================

/// SQL template literal: ctx.sql`SELECT * FROM users WHERE id = ${id}`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SqlTemplate {
    /// Static parts of the SQL string
    pub parts: Vec<String>,
    /// Interpolated expressions (${expr})
    pub expressions: Vec<Expression>,
}

/// Inferred type from SQL query based on schema
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SqlResultType {
    /// Selected columns and their types
    pub columns: Vec<SqlColumnType>,
    /// Whether result can be null (for .first())
    pub nullable: bool,
    /// Whether result is array (for .all())
    pub is_array: bool,
}

/// Column type in SQL result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SqlColumnType {
    pub name: String,
    pub column_type: ColumnType,
    pub nullable: bool,
}

// ============================================================================
// Repository Definition
// ============================================================================

/// Repository definition: `UserRepository :: users { methods... }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepositoryDef {
    /// Repository name (e.g., "UserRepository")
    pub name: String,
    /// Target table name (e.g., "users")
    pub table: String,
    /// Custom methods
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<RepositoryMethod>,
}

/// Repository method definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepositoryMethod {
    /// Method name
    pub name: String,
    /// Parameters
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<String>,
    /// Method body (SQL template or expression)
    pub body: Expression,
}
