//! Expression code generation
//!
//! Generates JavaScript code from topo expressions.

use crate::ast::*;
use super::JsCodegen;

impl JsCodegen {
    pub(super) fn generate_method(&mut self, method: &MethodDef) {
        let body = self.generate_expression(&method.body);
        self.emit_line(&format!("function {}() {{ return {}; }}", method.name, body));
    }

    pub(super) fn generate_expression(&mut self, expr: &Expression) -> String {
        match expr {
            Expression::String { value } => {
                // Escape special characters for JavaScript string literals
                let escaped = value
                    .replace('\\', "\\\\")  // Backslash first
                    .replace('\'', "\\'")   // Single quotes
                    .replace('\n', "\\n")   // Newlines
                    .replace('\r', "\\r")   // Carriage returns
                    .replace('\t', "\\t");  // Tabs
                format!("'{}'", escaped)
            }
            Expression::Number { value } => value.to_string(),
            Expression::Boolean { value } => value.to_string(),
            Expression::Null => "null".to_string(),
            Expression::Identifier { name } => {
                // Local parameters (reducer/effect params) take precedence over state fields
                if self.local_params.contains(name) {
                    name.clone()
                } else if self.state_fields.contains(name) {
                    // In reducer context, prefix state field names with "state."
                    format!("state.{}", name)
                } else if self.is_style_property() && self.is_theme_color(name) {
                    // Convert theme color to CSS variable: primary -> 'var(--primary)'
                    format!("'var(--{})'", name)
                } else if self.should_quote_identifier() {
                    // Quote identifier values for properties like type, align
                    format!("'{}'", name)
                } else if let Some(store_name) = &self.current_file_store_name {
                    // Check for unqualified action reference from anonymous store
                    if self.current_file_store_actions.contains(name) {
                        // Generate as action reference: () => dispatch('StoreName', 'Action')
                        format!("() => dispatch('{}', '{}')", store_name, name)
                    } else if self.current_file_store_fields.contains(name) {
                        // Generate as state field reference: StoreName.state.field
                        let var_name = self.store_var_name(store_name);
                        format!("{}.state.{}", var_name, name)
                    } else {
                        name.clone()
                    }
                } else {
                    name.clone()
                }
            }
            Expression::Array { elements } => {
                let elems: Vec<String> = elements.iter().map(|e| self.generate_expression(e)).collect();
                format!("[{}]", elems.join(", "))
            }
            Expression::Spread { expr } => {
                format!("...{}", self.generate_expression(expr))
            }
            Expression::Conditional { condition, then_branch, else_branch } => {
                let cond = self.generate_expression(condition);
                let then_expr = self.generate_expression(then_branch);
                let else_expr = self.generate_expression(else_branch);
                format!("({} ? {} : {})", cond, then_expr, else_expr)
            }
            Expression::ForIn { item, index, items, body } => {
                let items_str = self.generate_expression(items);
                // Add item and index to local params for body generation
                self.local_params.insert(item.clone());
                if let Some(ref idx) = index {
                    self.local_params.insert(idx.clone());
                }
                let body_str = self.generate_expression(body);
                self.local_params.remove(item);
                if let Some(ref idx) = index {
                    self.local_params.remove(idx);
                }
                // Generate .map() with or without index
                match index {
                    Some(idx) => format!("{}.map(({}, {}) => {})", items_str, item, idx, body_str),
                    None => format!("{}.map({} => {})", items_str, item, body_str),
                }
            }
            Expression::Object { members } => {
                let props: Vec<String> = members
                    .iter()
                    .map(|m| match m {
                        ObjectMember::Property(p) => {
                            self.current_property_key = Some(p.key.clone());
                            let value = self.generate_expression(&p.value);
                            self.current_property_key = None;
                            format!("{}: {}", p.key, value)
                        }
                        ObjectMember::Spread { expr } => {
                            format!("...{}", self.generate_expression(expr))
                        }
                    })
                    .collect();
                format!("{{ {} }}", props.join(", "))
            }
            Expression::Reference { store, path } => {
                // Special handling for $route
                if store == "route" {
                    if path.is_empty() {
                        return "$route".to_string();
                    }
                    return format!("$route.{}", path.join("."));
                }

                // Special handling for $i18n
                if store == "i18n" {
                    if path.is_empty() {
                        return "$i18n".to_string();
                    }
                    return format!("$i18n.{}", path.join("."));
                }

                // Use store variable name (may differ from store name if same-name component exists)
                let var_name = self.store_var_name(store);

                if path.is_empty() {
                    format!("{}.state", var_name)
                } else if path.last().map(|s| s.as_str()) == Some("label") {
                    // $Store.field.label -> Store.labels.field (virtual property)
                    let field_path: Vec<&str> = path.iter()
                        .take(path.len() - 1)
                        .map(|s| s.as_str())
                        .collect();
                    format!("{}.labels.{}", var_name, field_path.join("."))
                } else {
                    format!("{}.state.{}", var_name, path.join("."))
                }
            }
            Expression::RouteRef { path } => {
                // Route reference: .home -> __router.home, .docs.installation -> __router.docs.installation
                format!("__router.{}", path.join("."))
            }
            Expression::ActionRef { store, action, args } => {
                let args_str: Vec<String> = args.iter().map(|a| self.generate_expression(a)).collect();
                if args_str.is_empty() {
                    // For input handlers, use dispatchField for efficient updates
                    if self.is_input_event_handler() {
                        // Extract field name from action (e.g., SetEmail -> email)
                        let field_name = if action.starts_with("Set") && action.len() > 3 {
                            let name = &action[3..];
                            // Convert to camelCase (first letter lowercase)
                            let mut chars = name.chars();
                            match chars.next() {
                                Some(c) => format!("{}{}", c.to_lowercase(), chars.collect::<String>()),
                                None => name.to_string(),
                            }
                        } else {
                            action.clone()
                        };
                        format!("(value) => dispatchField('{}', '{}', value, '{}')", store, action, field_name)
                    } else {
                        format!("() => dispatch('{}', '{}')", store, action)
                    }
                } else {
                    format!(
                        "() => dispatch('{}', '{}', {})",
                        store,
                        action,
                        args_str.join(", ")
                    )
                }
            }
            Expression::ApiCall { method, args } => {
                let args_str: Vec<String> = args.iter().map(|a| self.generate_expression(a)).collect();
                format!("{}({})", method, args_str.join(", "))
            }
            Expression::BinaryOp { left, op, right } => {
                let l = self.generate_expression(left);
                let r = self.generate_expression(right);
                let op_str = match op {
                    BinaryOperator::Add => "+",
                    BinaryOperator::Sub => "-",
                    BinaryOperator::Mul => "*",
                    BinaryOperator::Div => "/",
                    BinaryOperator::Mod => "%",
                    BinaryOperator::Eq => "===",
                    BinaryOperator::Ne => "!==",
                    BinaryOperator::Lt => "<",
                    BinaryOperator::Le => "<=",
                    BinaryOperator::Gt => ">",
                    BinaryOperator::Ge => ">=",
                    BinaryOperator::And => "&&",
                    BinaryOperator::Or => "||",
                };
                format!("({} {} {})", l, op_str, r)
            }
            Expression::UnaryOp { op, operand } => {
                let o = self.generate_expression(operand);
                match op {
                    UnaryOperator::Not => format!("!{}", o),
                    UnaryOperator::Neg => format!("-{}", o),
                }
            }
            Expression::MemberAccess { object, property } => {
                // In event handler context, treat Store.Action as ActionRef
                // Only apply to simple Store.Action patterns (not nested member accesses like ColorStyles.neutral.borderLight)
                if self.is_event_handler() {
                    if let Expression::Identifier { name: store } = object.as_ref() {
                        // Check if this is a known store - don't wrap arbitrary identifiers
                        if self.store_state_fields.contains_key(store) || self.stores_with_components.contains(store) {
                            // For input handlers, use dispatchField for efficient updates
                            if self.is_input_event_handler() {
                                // Extract field name from action (e.g., SetEmail -> email)
                                let field_name = if property.starts_with("Set") && property.len() > 3 {
                                    let name = &property[3..];
                                    let mut chars = name.chars();
                                    match chars.next() {
                                        Some(c) => format!("{}{}", c.to_lowercase(), chars.collect::<String>()),
                                        None => name.to_string(),
                                    }
                                } else {
                                    property.clone()
                                };
                                return format!("(value) => dispatchField('{}', '{}', value, '{}')", store, property, field_name);
                            } else {
                                return format!("() => dispatch('{}', '{}')", store, property);
                            }
                        }
                    }
                }
                // Check if object is an API service name - add Api suffix
                if let Expression::Identifier { name } = object.as_ref() {
                    if self.api_service_names.contains(name) {
                        return format!("{}Api.{}", name, property);
                    }
                    // Check if accessing .Fields on a store with same-name component
                    if property == "Fields" && self.stores_with_components.contains(name) {
                        return format!("_{}Store.Fields", name);
                    }
                    // Check if accessing a store state field: Store.field -> Store.state.field
                    if let Some(state_fields) = self.store_state_fields.get(name) {
                        if state_fields.contains(property) {
                            return format!("{}.state.{}", name, property);
                        }
                    }
                }
                let obj = self.generate_expression(object);
                format!("{}.{}", obj, property)
            }
            Expression::IndexAccess { object, index } => {
                let obj = self.generate_expression(object);
                let idx = self.generate_expression(index);
                format!("{}[{}]", obj, idx)
            }
            Expression::Call { callee, args } => {
                let c = self.generate_expression(callee);

                // Check for object-style props: ComponentName({ prop1: val1, prop2: val2 })
                // Convert to positional args: ComponentName(val1, val2)
                if args.len() == 1 {
                    if let Expression::Object { members } = &args[0] {
                        // Extract properties (skip spreads for positional arg conversion)
                        let properties: Vec<&Property> = members.iter()
                            .filter_map(|m| match m {
                                ObjectMember::Property(p) => Some(p),
                                ObjectMember::Spread { .. } => None,
                            })
                            .collect();
                        let has_spread = members.iter().any(|m| matches!(m, ObjectMember::Spread { .. }));

                        if let Expression::Identifier { name } = callee.as_ref() {
                            // Clone param_names to avoid borrow conflict
                            let param_names_opt = self.component_params.get(name).cloned();
                            if let Some(param_names) = param_names_opt {
                                // If component uses single "props" param or has spread, pass object as-is
                                if (param_names.len() == 1 && param_names[0] == "props") || has_spread {
                                    let props: Vec<String> = members
                                        .iter()
                                        .map(|m| match m {
                                            ObjectMember::Property(p) => format!("{}: {}", p.key, self.generate_expression(&p.value)),
                                            ObjectMember::Spread { expr } => format!("...{}", self.generate_expression(expr)),
                                        })
                                        .collect();
                                    return format!("{}({{ {} }})", c, props.join(", "));
                                }
                                // Check for Reference props and collect their paths
                                let mut auto_data_error: Option<String> = None;
                                let mut auto_data_field: Option<String> = None;
                                for p in &properties {
                                    if let Expression::Reference { store, path } = &p.value {
                                        if let Some(last) = path.last() {
                                            if last.ends_with("Error") {
                                                // Build the data-error path: StoreName.fieldNameError
                                                let error_path = format!("{}.{}", store, path.join("."));
                                                auto_data_error = Some(error_path);
                                            } else if p.key == "value" {
                                                // Build the data-field path: StoreName.fieldName
                                                let field_path = format!("{}.{}", store, path.join("."));
                                                auto_data_field = Some(field_path);
                                            }
                                        }
                                    }
                                }

                                // Convert object props to positional args in param order
                                let mut positional_args: Vec<String> = Vec::new();
                                for param_name in &param_names {
                                    // Check if this param should be auto-filled
                                    let explicit_value = properties
                                        .iter()
                                        .find(|p| &p.key == param_name);

                                    let value = if let Some(p) = explicit_value {
                                        // Set property key context for event handler detection
                                        self.current_property_key = Some(p.key.clone());
                                        let val = self.generate_expression(&p.value);
                                        self.current_property_key = None;
                                        val
                                    } else if param_name == "dataError" {
                                        // Auto-fill dataError if we detected an error Reference
                                        auto_data_error.as_ref()
                                            .map(|path| format!("'{}'", path))
                                            .unwrap_or_else(|| "undefined".to_string())
                                    } else if param_name == "dataField" {
                                        // Auto-fill dataField if we detected a value Reference
                                        auto_data_field.as_ref()
                                            .map(|path| format!("'{}'", path))
                                            .unwrap_or_else(|| "undefined".to_string())
                                    } else {
                                        "undefined".to_string()
                                    };
                                    positional_args.push(value);
                                }

                                return format!("{}({})", c, positional_args.join(", "));
                            }
                        }
                    }
                }

                // Get component params if this is a component call
                let param_names = if let Expression::Identifier { name } = callee.as_ref() {
                    self.component_params.get(name).cloned()
                } else {
                    None
                };

                let args_str: Vec<String> = args.iter().enumerate().map(|(i, a)| {
                    // Check if this arg corresponds to an event handler param
                    if let Some(ref params) = param_names {
                        if let Some(param_name) = params.get(i) {
                            if matches!(param_name.as_str(), "onClick" | "click" | "onInput" | "input" | "onChange" | "change" | "onSubmit" | "submit") {
                                self.current_property_key = Some(param_name.clone());
                                let val = self.generate_expression(a);
                                self.current_property_key = None;
                                return val;
                            }
                        }
                    }
                    self.generate_expression(a)
                }).collect();
                let call = format!("{}({})", c, args_str.join(", "));

                // In event handler context (click, etc.), wrap method calls in arrow function
                if self.is_event_handler() && !self.is_input_event_handler() {
                    // Check if it's a method call on $i18n or similar
                    if let Expression::MemberAccess { .. } = callee.as_ref() {
                        return format!("() => {}", call);
                    }
                    if let Expression::Reference { .. } = callee.as_ref() {
                        return format!("() => {}", call);
                    }
                }
                call
            }
            Expression::Await { expr } => {
                let e = self.generate_expression(expr);
                format!("await {}", e)
            }
            Expression::Pipe { value, pipe_name, args } => {
                let value_str = self.generate_expression(value);
                let args_str: Vec<String> = args.iter().map(|a| self.generate_expression(a)).collect();
                if args_str.is_empty() {
                    format!("__pipes.{}({})", pipe_name, value_str)
                } else {
                    format!("__pipes.{}({}, {})", pipe_name, value_str, args_str.join(", "))
                }
            }
        }
    }
}
