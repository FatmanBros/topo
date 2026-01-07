//! AST (Abstract Syntax Tree) definitions for topo language
//!
//! The topo language has 4 types of definitions:
//! - Component: `Name -> { }` - UI components
//! - Method: `Name { }` - Logic/functions
//! - ApiService: `Name :: { }` - API service definitions
//! - Store: `Name | { }` - State management

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
}

/// Component alias definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentAlias {
    /// Base component name
    pub base: String,
    /// Arguments to pass to the base component
    pub args: Vec<Expression>,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Endpoint {
    pub name: String,
    pub method: HttpMethod,
    pub path: String,
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
    pub name: String,
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

    /// Dispatch action: `dispatch: ActionName`
    Dispatch {
        action: String,
        args: Vec<Expression>,
    },

    /// Try-catch block
    TryCatch {
        try_block: Vec<Statement>,
        catch_param: String,
        catch_block: Vec<Statement>,
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

    /// For loop: `for item in items { Component(item) }`
    ForIn {
        item: String,
        items: Box<Expression>,
        body: Box<Expression>,
    },

    /// Object: `{ key: value }`
    Object { properties: Vec<Property> },

    /// Identifier: `foo`
    Identifier { name: String },

    /// Store reference: `$Store.path`
    Reference { store: String, path: Vec<String> },

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

    /// Function call: `fn(args)`
    Call {
        callee: Box<Expression>,
        args: Vec<Expression>,
    },

    /// Await expression: `await expr`
    Await { expr: Box<Expression> },

    /// Spread expression: `...arr`
    Spread { expr: Box<Expression> },
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
