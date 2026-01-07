//! Type checker for topo language
//!
//! Performs static type checking on the AST to catch type errors at compile time.

use std::collections::HashMap;
use thiserror::Error;

use crate::ast::*;

#[derive(Error, Debug, Clone)]
pub enum TypeError {
    #[error("Unknown type '{name}' at {location}")]
    UnknownType { name: String, location: String },

    #[error("Type mismatch: expected '{expected}', found '{found}' at {location}")]
    TypeMismatch {
        expected: String,
        found: String,
        location: String,
    },

    #[error("Unknown store '{name}' at {location}")]
    UnknownStore { name: String, location: String },

    #[error("Unknown property '{property}' on store '{store}' at {location}")]
    UnknownProperty {
        store: String,
        property: String,
        location: String,
    },

    #[error("Unknown action '{action}' on store '{store}' at {location}")]
    UnknownAction {
        store: String,
        action: String,
        location: String,
    },

    #[error("Missing required field '{field}' of type '{expected_type}' at {location}")]
    MissingField {
        field: String,
        expected_type: String,
        location: String,
    },

    #[error("Argument count mismatch: expected {expected}, found {found} at {location}")]
    ArgumentCountMismatch {
        expected: usize,
        found: usize,
        location: String,
    },
}

/// Type environment for type checking
#[derive(Debug, Default)]
pub struct TypeEnv {
    /// Store definitions: store_name -> (state_fields, actions)
    stores: HashMap<String, StoreType>,
    /// Component definitions: component_name -> params
    components: HashMap<String, Vec<TypedParam>>,
    /// Local scope variables: var_name -> type
    locals: HashMap<String, TypeAnnotation>,
}

#[derive(Debug, Clone)]
pub struct StoreType {
    pub state_fields: HashMap<String, TypeAnnotation>,
    /// Internal actions (private - only accessible within the same component/store)
    pub actions: HashMap<String, Vec<Param>>,
    /// Commands (public - accessible from Templates and external components)
    pub commands: HashMap<String, Vec<Param>>,
}

/// Type checker
pub struct TypeChecker {
    env: TypeEnv,
    errors: Vec<TypeError>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            env: TypeEnv::default(),
            errors: Vec::new(),
        }
    }

    /// Check a program and return any type errors
    pub fn check(&mut self, program: &Program) -> Vec<TypeError> {
        // First pass: collect all type definitions (stores, components)
        self.collect_types(program);

        // Second pass: check all expressions and statements
        self.check_declarations(program);

        std::mem::take(&mut self.errors)
    }

    /// First pass: collect type definitions
    fn collect_types(&mut self, program: &Program) {
        for decl in &program.declarations {
            match decl {
                Declaration::Store(store) => {
                    self.collect_store_type(store);
                }
                Declaration::Component(comp) => {
                    self.env
                        .components
                        .insert(comp.name.clone(), comp.params.clone());
                }
                _ => {}
            }
        }
    }

    fn collect_store_type(&mut self, store: &StoreDef) {
        let mut state_fields = HashMap::new();
        let mut actions = HashMap::new();
        let mut commands = HashMap::new();

        // Collect state fields
        if let Some(state) = &store.state {
            for field in &state.fields {
                let field_type = field
                    .type_annotation
                    .clone()
                    .unwrap_or_else(|| self.infer_type(&field.value));
                state_fields.insert(field.key.clone(), field_type);
            }
        }

        // Collect actions (internal/private)
        if let Some(actions_block) = &store.actions {
            for action in &actions_block.actions {
                actions.insert(action.name.clone(), action.params.clone());
            }
        }

        // Collect commands (external/public)
        if let Some(commands_block) = &store.commands {
            for command in &commands_block.commands {
                commands.insert(command.name.clone(), command.params.clone());
            }
        }

        self.env.stores.insert(
            store.name.clone(),
            StoreType {
                state_fields,
                actions,
                commands,
            },
        );
    }

    /// Infer type from an expression
    fn infer_type(&self, expr: &Expression) -> TypeAnnotation {
        match expr {
            Expression::String { .. } => TypeAnnotation::Primitive {
                name: "string".to_string(),
            },
            Expression::Number { .. } => TypeAnnotation::Primitive {
                name: "number".to_string(),
            },
            Expression::Boolean { .. } => TypeAnnotation::Primitive {
                name: "boolean".to_string(),
            },
            Expression::Null => TypeAnnotation::Primitive {
                name: "null".to_string(),
            },
            Expression::Array { elements } => {
                if elements.is_empty() {
                    TypeAnnotation::Array {
                        element_type: Box::new(TypeAnnotation::Primitive {
                            name: "any".to_string(),
                        }),
                    }
                } else {
                    TypeAnnotation::Array {
                        element_type: Box::new(self.infer_type(&elements[0])),
                    }
                }
            }
            Expression::Object { properties } => {
                let fields = properties
                    .iter()
                    .map(|p| TypedField {
                        name: p.key.clone(),
                        type_annotation: p
                            .type_annotation
                            .clone()
                            .unwrap_or_else(|| self.infer_type(&p.value)),
                    })
                    .collect();
                TypeAnnotation::Object { fields }
            }
            Expression::Reference { store, path } => {
                // Look up the store type
                if let Some(store_type) = self.env.stores.get(store) {
                    if let Some(first) = path.first() {
                        if let Some(field_type) = store_type.state_fields.get(first) {
                            return field_type.clone();
                        }
                    }
                }
                TypeAnnotation::Primitive {
                    name: "any".to_string(),
                }
            }
            Expression::Identifier { name } => {
                // Check locals first
                if let Some(t) = self.env.locals.get(name) {
                    return t.clone();
                }
                TypeAnnotation::Primitive {
                    name: "any".to_string(),
                }
            }
            Expression::BinaryOp { left, op, right: _ } => {
                match op {
                    // Comparison operators return boolean
                    BinaryOperator::Eq
                    | BinaryOperator::Ne
                    | BinaryOperator::Lt
                    | BinaryOperator::Le
                    | BinaryOperator::Gt
                    | BinaryOperator::Ge
                    | BinaryOperator::And
                    | BinaryOperator::Or => TypeAnnotation::Primitive {
                        name: "boolean".to_string(),
                    },
                    // Arithmetic operators return the type of operands
                    _ => self.infer_type(left),
                }
            }
            Expression::UnaryOp { op, operand } => match op {
                UnaryOperator::Not => TypeAnnotation::Primitive {
                    name: "boolean".to_string(),
                },
                UnaryOperator::Neg => self.infer_type(operand),
            },
            _ => TypeAnnotation::Primitive {
                name: "any".to_string(),
            },
        }
    }

    /// Second pass: check declarations
    fn check_declarations(&mut self, program: &Program) {
        for decl in &program.declarations {
            match decl {
                Declaration::Component(comp) => self.check_component(comp),
                Declaration::Store(store) => self.check_store(store),
                _ => {}
            }
        }
    }

    fn check_component(&mut self, comp: &ComponentDef) {
        // Add params to local scope
        let old_locals = std::mem::take(&mut self.env.locals);
        for param in &comp.params {
            if let Some(type_ann) = &param.type_annotation {
                self.env.locals.insert(param.name.clone(), type_ann.clone());
            }
        }

        // Check properties
        for prop in &comp.properties {
            self.check_property(prop, &comp.name);
        }

        self.env.locals = old_locals;
    }

    fn check_property(&mut self, prop: &Property, context: &str) {
        // If there's a type annotation, check the value matches
        if let Some(expected_type) = &prop.type_annotation {
            let actual_type = self.infer_type(&prop.value);
            if !self.types_compatible(expected_type, &actual_type) {
                self.errors.push(TypeError::TypeMismatch {
                    expected: self.type_to_string(expected_type),
                    found: self.type_to_string(&actual_type),
                    location: format!("property '{}' in {}", prop.key, context),
                });
            }
        }

        // Check expression for references to unknown stores/properties
        self.check_expression(&prop.value, context);
    }

    fn check_expression(&mut self, expr: &Expression, context: &str) {
        match expr {
            Expression::Reference { store, path } => {
                if let Some(store_type) = self.env.stores.get(store) {
                    if let Some(first) = path.first() {
                        if !store_type.state_fields.contains_key(first) {
                            self.errors.push(TypeError::UnknownProperty {
                                store: store.clone(),
                                property: first.clone(),
                                location: context.to_string(),
                            });
                        }
                    }
                } else {
                    self.errors.push(TypeError::UnknownStore {
                        name: store.clone(),
                        location: context.to_string(),
                    });
                }
            }
            Expression::ActionRef {
                store,
                action,
                args,
            } => {
                if let Some(store_type) = self.env.stores.get(store) {
                    // Check both actions (internal) and commands (public)
                    let action_params = store_type
                        .actions
                        .get(action)
                        .or_else(|| store_type.commands.get(action));

                    if let Some(params) = action_params {
                        // Check argument count
                        if args.len() != params.len() {
                            self.errors.push(TypeError::ArgumentCountMismatch {
                                expected: params.len(),
                                found: args.len(),
                                location: format!("{}.{} in {}", store, action, context),
                            });
                        }
                        // Check argument types
                        for (i, (arg, param)) in args.iter().zip(params.iter()).enumerate() {
                            if let Some(expected_type) = &param.type_annotation {
                                let actual_type = self.infer_type(arg);
                                if !self.types_compatible(expected_type, &actual_type) {
                                    self.errors.push(TypeError::TypeMismatch {
                                        expected: self.type_to_string(expected_type),
                                        found: self.type_to_string(&actual_type),
                                        location: format!(
                                            "argument {} of {}.{} in {}",
                                            i + 1,
                                            store,
                                            action,
                                            context
                                        ),
                                    });
                                }
                            }
                        }
                    } else {
                        self.errors.push(TypeError::UnknownAction {
                            store: store.clone(),
                            action: action.clone(),
                            location: context.to_string(),
                        });
                    }
                } else {
                    self.errors.push(TypeError::UnknownStore {
                        name: store.clone(),
                        location: context.to_string(),
                    });
                }

                // Recursively check args
                for arg in args {
                    self.check_expression(arg, context);
                }
            }
            Expression::BinaryOp { left, right, .. } => {
                self.check_expression(left, context);
                self.check_expression(right, context);
            }
            Expression::UnaryOp { operand, .. } => {
                self.check_expression(operand, context);
            }
            Expression::Array { elements } => {
                for elem in elements {
                    self.check_expression(elem, context);
                }
            }
            Expression::Object { properties } => {
                for prop in properties {
                    self.check_property(prop, context);
                }
            }
            Expression::Call { callee, args } => {
                self.check_expression(callee, context);
                for arg in args {
                    self.check_expression(arg, context);
                }
            }
            Expression::MemberAccess { object, .. } => {
                self.check_expression(object, context);
            }
            Expression::ForIn { items, body, .. } => {
                self.check_expression(items, context);
                self.check_expression(body, context);
            }
            Expression::Await { expr } => {
                self.check_expression(expr, context);
            }
            _ => {}
        }
    }

    fn check_store(&mut self, store: &StoreDef) {
        // Check state fields
        if let Some(state) = &store.state {
            for field in &state.fields {
                self.check_property(field, &format!("{}.State", store.name));
            }
        }

        // Check reducers reference valid actions or commands
        if let Some(reducers) = &store.reducers {
            let store_type = self.env.stores.get(&store.name);
            for handler in &reducers.handlers {
                if let Some(st) = store_type {
                    let action_exists = st.actions.contains_key(&handler.action)
                        || st.commands.contains_key(&handler.action);
                    if !action_exists {
                        self.errors.push(TypeError::UnknownAction {
                            store: store.name.clone(),
                            action: handler.action.clone(),
                            location: format!("{}.Reducers", store.name),
                        });
                    }
                }
            }
        }

        // Check effects reference valid actions or commands
        if let Some(effects) = &store.effects {
            let store_type = self.env.stores.get(&store.name);
            for handler in &effects.handlers {
                if let Some(st) = store_type {
                    let action_exists = st.actions.contains_key(&handler.action)
                        || st.commands.contains_key(&handler.action);
                    if !action_exists {
                        self.errors.push(TypeError::UnknownAction {
                            store: store.name.clone(),
                            action: handler.action.clone(),
                            location: format!("{}.Effects", store.name),
                        });
                    }
                }
            }
        }
    }

    /// Check if two types are compatible
    fn types_compatible(&self, expected: &TypeAnnotation, actual: &TypeAnnotation) -> bool {
        // 'any' is compatible with everything
        if matches!(expected, TypeAnnotation::Primitive { name } if name == "any") {
            return true;
        }
        if matches!(actual, TypeAnnotation::Primitive { name } if name == "any") {
            return true;
        }

        // null is compatible with optional types
        if matches!(actual, TypeAnnotation::Primitive { name } if name == "null") {
            return matches!(expected, TypeAnnotation::Optional { .. });
        }

        match (expected, actual) {
            (TypeAnnotation::Primitive { name: e }, TypeAnnotation::Primitive { name: a }) => {
                e == a
            }
            (
                TypeAnnotation::Array { element_type: e },
                TypeAnnotation::Array { element_type: a },
            ) => self.types_compatible(e, a),
            (
                TypeAnnotation::Optional { inner_type: e },
                TypeAnnotation::Optional { inner_type: a },
            ) => self.types_compatible(e, a),
            // Optional type accepts the inner type
            (TypeAnnotation::Optional { inner_type }, actual) => {
                self.types_compatible(inner_type, actual)
            }
            (TypeAnnotation::Object { fields: e }, TypeAnnotation::Object { fields: a }) => {
                // All expected fields must be present in actual with compatible types
                e.iter().all(|ef| {
                    a.iter().any(|af| {
                        af.name == ef.name
                            && self.types_compatible(&ef.type_annotation, &af.type_annotation)
                    })
                })
            }
            (TypeAnnotation::Union { types }, actual) => {
                types.iter().any(|t| self.types_compatible(t, actual))
            }
            (expected, TypeAnnotation::Union { types }) => {
                types.iter().all(|t| self.types_compatible(expected, t))
            }
            (TypeAnnotation::Reference { name: e }, TypeAnnotation::Reference { name: a }) => {
                e == a
            }
            _ => false,
        }
    }

    /// Convert type to string for error messages
    fn type_to_string(&self, ty: &TypeAnnotation) -> String {
        match ty {
            TypeAnnotation::Primitive { name } => name.clone(),
            TypeAnnotation::Array { element_type } => {
                format!("{}[]", self.type_to_string(element_type))
            }
            TypeAnnotation::Optional { inner_type } => {
                format!("{}?", self.type_to_string(inner_type))
            }
            TypeAnnotation::Object { fields } => {
                let fields_str: Vec<String> = fields
                    .iter()
                    .map(|f| format!("{}: {}", f.name, self.type_to_string(&f.type_annotation)))
                    .collect();
                format!("{{ {} }}", fields_str.join(", "))
            }
            TypeAnnotation::Union { types } => {
                let types_str: Vec<String> = types.iter().map(|t| self.type_to_string(t)).collect();
                types_str.join(" | ")
            }
            TypeAnnotation::Reference { name } => name.clone(),
        }
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Lexer;
    use crate::Parser;

    fn check_source(source: &str) -> Vec<TypeError> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse().unwrap();
        let mut checker = TypeChecker::new();
        checker.check(&program)
    }

    #[test]
    fn test_valid_store_reference() {
        let source = r#"
            Counter | {
                State {
                    count: number = 0
                }
            }

            Display -> {
                type: div
                content: $Counter.count
            }
        "#;
        let errors = check_source(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_unknown_store_reference() {
        let source = r#"
            Display -> {
                type: div
                content: $Unknown.value
            }
        "#;
        let errors = check_source(source);
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], TypeError::UnknownStore { .. }));
    }

    #[test]
    fn test_unknown_property_reference() {
        let source = r#"
            Counter | {
                State {
                    count: number = 0
                }
            }

            Display -> {
                type: div
                content: $Counter.unknown
            }
        "#;
        let errors = check_source(source);
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], TypeError::UnknownProperty { .. }));
    }

    #[test]
    fn test_type_mismatch() {
        let source = r#"
            Form -> {
                name: string = 42
            }
        "#;
        let errors = check_source(source);
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], TypeError::TypeMismatch { .. }));
    }

    #[test]
    fn test_optional_type() {
        let source = r#"
            Form -> {
                name: string? = null
            }
        "#;
        let errors = check_source(source);
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }
}
