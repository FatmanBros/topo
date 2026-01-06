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
    /// Component definition: `Name -> { ... }`
    Component(ComponentDef),

    /// Method definition: `Name { ... }`
    Method(MethodDef),

    /// API Service definition: `Name :: { ... }`
    ApiService(ApiServiceDef),

    /// Store definition: `Name | { ... }`
    Store(StoreDef),
}

// ============================================================================
// Component Definition (->)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentDef {
    pub name: String,
    pub properties: Vec<Property>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Property {
    pub key: String,
    pub value: Expression,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionDef {
    pub name: String,
    pub params: Vec<Param>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub type_annotation: Option<String>,
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
