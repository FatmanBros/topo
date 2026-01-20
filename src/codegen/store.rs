//! Store code generation - handles Store definitions, reducers, effects, selectors

use crate::ast::{Expression, Property, ReducerHandler, SelectorDef, EffectHandler, Statement, StoreDef};
use crate::codegen::JsCodegen;

/// Capitalize the first letter of a string
pub(super) fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

impl JsCodegen {
    pub(super) fn generate_store(&mut self, store: &StoreDef) {
        // Resolve store name (use explicit name or derive from filename)
        let store_name = self.resolve_store_name(store, self.current_file_path.as_deref());

        // Collect fields with validation annotations for auto-validation
        let validated_fields: Vec<String> = store
            .state
            .as_ref()
            .map(|s| {
                s.fields
                    .iter()
                    .filter(|f| !f.annotations.is_empty())
                    .map(|f| f.key.clone())
                    .collect()
            })
            .unwrap_or_default();

        // Get variable name (different from registry name if same-name component exists)
        let var_name = self.store_var_name(&store_name);

        // Generate store creation
        self.emit_line(&format!(
            "const {} = createStore('{}', {{",
            var_name, store_name
        ));
        self.indent += 1;

        if let Some(state) = &store.state {
            // Count total fields including auto-generated error fields
            let total_fields = state.fields.len() + validated_fields.len();
            let mut field_index = 0;

            for field in &state.fields {
                field_index += 1;
                let comma = if field_index < total_fields { "," } else { "" };
                let value = self.generate_expression(&field.value);
                self.emit_line(&format!("{}: {}{}", field.key, value, comma));
            }

            // Auto-generate error fields for validated fields
            for (i, field_name) in validated_fields.iter().enumerate() {
                let comma = if i < validated_fields.len() - 1 {
                    ","
                } else {
                    ""
                };
                self.emit_line(&format!("{}Error: ''{}", field_name, comma));
            }
        }

        self.indent -= 1;
        self.emit_line("});");
        self.emit_line("");

        // Generate validation rules from annotations
        if let Some(state) = &store.state {
            let validation_rules = self.collect_validation_rules(&state.fields);
            let field_keys = self.collect_field_keys(&state.fields);

            if !validation_rules.is_empty() {
                self.emit_line(&format!("const {}ValidationRules = {{", var_name));
                self.indent += 1;
                for (i, (field, rules)) in validation_rules.iter().enumerate() {
                    let comma = if i < validation_rules.len() - 1 {
                        ","
                    } else {
                        ""
                    };
                    self.emit_line(&format!("{}: [{}]{}", field, rules.join(", "), comma));
                }
                self.indent -= 1;
                self.emit_line("};");
                self.emit_line("");

                // Generate field keys map (for i18n)
                self.emit_line(&format!("const {}FieldKeys = {{", var_name));
                self.indent += 1;
                for (i, (field, key)) in field_keys.iter().enumerate() {
                    let comma = if i < field_keys.len() - 1 { "," } else { "" };
                    self.emit_line(&format!("{}: '{}'{}", field, key, comma));
                }
                self.indent -= 1;
                self.emit_line("};");
                self.emit_line("");

                // Attach keys to store object for runtime access ($Store.keys.field)
                self.emit_line(&format!("{}.keys = {}FieldKeys;", var_name, var_name));

                // Generate validate helper for this store with custom labels
                self.emit_line(&format!("function validate{}(data) {{", var_name));
                self.indent += 1;
                self.emit_line("const errors = {};");
                self.emit_line(&format!(
                    "for (const [field, fieldRules] of Object.entries({}ValidationRules)) {{",
                    var_name
                ));
                self.indent += 1;
                self.emit_line("const value = data[field];");
                self.emit_line(&format!(
                    "const label = typeof t === 'function' ? t({}FieldKeys[field]) : ({}FieldKeys[field] || field);",
                    var_name, var_name
                ));
                self.emit_line("for (const rule of fieldRules) {");
                self.indent += 1;
                self.emit_line("const validator = validators[rule.name];");
                self.emit_line("if (validator) {");
                self.indent += 1;
                self.emit_line("const result = validator(value, rule.args || [], label);");
                self.emit_line("if (!result.valid) {");
                self.indent += 1;
                self.emit_line("errors[field] = errors[field] || [];");
                self.emit_line("errors[field].push(result.error);");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
                self.emit_line("return { valid: Object.keys(errors).length === 0, errors };");
                self.indent -= 1;
                self.emit_line("}");
                self.emit_line("");
            }

            // Collect user-defined reducer action names to avoid overwriting
            let user_defined_actions: std::collections::HashSet<String> = store
                .reducers
                .as_ref()
                .map(|r| r.handlers.iter().map(|h| h.action.clone()).collect())
                .unwrap_or_default();

            // Auto-generate Set actions with validation for annotated fields
            // Skip if user has explicitly defined the reducer
            for field in &state.fields {
                if !field.annotations.is_empty()
                    && !field
                        .annotations
                        .iter()
                        .all(|a| a.name == "key" || a.name == "hidden")
                {
                    let field_name = &field.key;
                    let capitalized = capitalize_first(field_name);
                    let action_name = format!("Set{}", capitalized);

                    // Skip if user defined this reducer explicitly
                    if user_defined_actions.contains(&action_name) {
                        continue;
                    }

                    // Generate auto-validating setter
                    self.emit_line(&format!(
                        "{}.on('Set{}', (state, value) => {{",
                        var_name, capitalized
                    ));
                    self.indent += 1;
                    self.emit_line(&format!(
                        "const result = validate{}({{ ...state, {}: value }});",
                        var_name, field_name
                    ));
                    self.emit_line(&format!(
                        "const error = result.errors.{} ? result.errors.{}[0] : '';",
                        field_name, field_name
                    ));
                    self.emit_line("return {");
                    self.indent += 1;
                    self.emit_line("...state,");
                    self.emit_line(&format!("{}: value,", field_name));
                    self.emit_line(&format!("{}Error: error", field_name));
                    self.indent -= 1;
                    self.emit_line("};");
                    self.indent -= 1;
                    self.emit_line("});");
                    self.emit_line("");
                }
            }

            // Auto-generate Submit action handler that validates all fields
            // Skip if user has explicitly defined the Submit reducer
            let validated_fields: Vec<&str> = state
                .fields
                .iter()
                .filter(|f| {
                    !f.annotations.is_empty()
                        && !f
                            .annotations
                            .iter()
                            .all(|a| a.name == "key" || a.name == "hidden")
                })
                .map(|f| f.key.as_str())
                .collect();

            if !validated_fields.is_empty() && !user_defined_actions.contains("Submit") {
                self.emit_line(&format!("{}.on('Submit', (state) => {{", var_name));
                self.indent += 1;
                self.emit_line(&format!("const result = validate{}(state);", var_name));
                self.emit_line("return {");
                self.indent += 1;
                self.emit_line("...state,");
                for (i, field) in validated_fields.iter().enumerate() {
                    let comma = if i < validated_fields.len() - 1 {
                        ","
                    } else {
                        ""
                    };
                    self.emit_line(&format!(
                        "{}Error: result.errors.{} ? result.errors.{}[0] : ''{}",
                        field, field, field, comma
                    ));
                }
                self.indent -= 1;
                self.emit_line("};");
                self.indent -= 1;
                self.emit_line("});");
                self.emit_line("");
            }
        }

        // Generate reducers
        if let Some(reducers) = &store.reducers {
            for handler in &reducers.handlers {
                self.generate_reducer(store, &store_name, handler);
            }
        }

        // Generate effects
        if let Some(effects) = &store.effects {
            for handler in &effects.handlers {
                self.generate_effect(store, &store_name, handler);
            }
        }

        // Generate selectors
        if let Some(selectors) = &store.selectors {
            for selector in &selectors.selectors {
                self.generate_selector(store, &store_name, selector);
            }
        }

        // Generate .Fields getter for automatic form generation
        if let Some(state) = &store.state {
            let form_fields = self.collect_form_fields(&state.fields);
            if !form_fields.is_empty() {
                self.emit_line(&format!(
                    "Object.defineProperty({}, 'Fields', {{",
                    var_name
                ));
                self.indent += 1;
                self.emit_line("get() {");
                self.indent += 1;
                self.emit_line("return [");
                self.indent += 1;
                for (field_name, i18n_key, input_type) in &form_fields {
                    let capitalized = capitalize_first(field_name);
                    self.emit_line(&format!(
                        "{{ type: 'formfield', label: {{ t: '{}' }}, inputType: '{}', placeholder: {{ t: '{}_placeholder' }}, value: {}.state.{}, onInput: (v) => dispatchField('{}', 'Set{}', v, '{}'), errorMsg: {}.state.{}Error, dataError: '{}.{}Error', dataField: '{}.{}' }},",
                        i18n_key, input_type, i18n_key,
                        var_name, field_name,
                        store_name, capitalized, field_name,
                        var_name, field_name,
                        store_name, field_name,
                        store_name, field_name
                    ));
                }
                self.indent -= 1;
                self.emit_line("];");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("});");
                self.emit_line("");
            }
        }
    }

    pub(super) fn generate_reducer(
        &mut self,
        store: &StoreDef,
        store_name: &str,
        handler: &ReducerHandler,
    ) {
        // Collect state field names for context-aware expression generation
        self.state_fields.clear();
        self.local_params.clear();

        if let Some(state) = &store.state {
            for field in &state.fields {
                self.state_fields.insert(field.key.clone());
            }
        }

        // Collect reducer parameters (these take precedence over state fields)
        for param in &handler.params {
            self.local_params.insert(param.clone());
        }

        let params = if handler.params.is_empty() {
            String::new()
        } else {
            format!(", {}", handler.params.join(", "))
        };

        let var_name = self.store_var_name(store_name);
        self.emit_line(&format!(
            "{}.on('{}', (state{}) => ({{",
            var_name, handler.action, params
        ));
        self.indent += 1;

        self.emit_line("...state,");
        for (i, prop) in handler.body.iter().enumerate() {
            let comma = if i < handler.body.len() - 1 { "," } else { "" };
            let value = self.generate_expression(&prop.value);
            self.emit_line(&format!("{}: {}{}", prop.key, value, comma));
        }

        self.indent -= 1;
        self.emit_line("}));");
        self.emit_line("");

        // Clear context after reducer generation
        self.state_fields.clear();
        self.local_params.clear();
    }

    pub(super) fn generate_effect(
        &mut self,
        store: &StoreDef,
        store_name: &str,
        handler: &EffectHandler,
    ) {
        // Save old local_params and clear for effect scope
        let old_local_params = std::mem::take(&mut self.local_params);

        // Add effect parameters to local_params
        for param in &handler.params {
            self.local_params.insert(param.clone());
        }

        let params = if handler.params.is_empty() {
            String::new()
        } else {
            handler.params.join(", ")
        };

        let var_name = self.store_var_name(store_name);
        self.emit_line(&format!(
            "{}.effect('{}', async ({}) => {{",
            var_name, handler.action, params
        ));
        self.indent += 1;

        for stmt in &handler.body {
            self.generate_statement_with_tracking(store, store_name, stmt);
        }

        self.indent -= 1;
        self.emit_line("});");
        self.emit_line("");

        // Restore old local_params
        self.local_params = old_local_params;
    }

    pub(super) fn generate_selector(
        &mut self,
        store: &StoreDef,
        store_name: &str,
        selector: &SelectorDef,
    ) {
        // Collect state field names for context-aware expression generation
        self.state_fields.clear();
        if let Some(state) = &store.state {
            for field in &state.fields {
                self.state_fields.insert(field.key.clone());
            }
        }

        let body = self.generate_expression(&selector.body);
        let var_name = self.store_var_name(store_name);
        self.emit_line(&format!(
            "{}.selector('{}', (state) => {});",
            var_name, selector.name, body
        ));

        self.state_fields.clear();
    }

    fn generate_statement_with_tracking(
        &mut self,
        store: &StoreDef,
        store_name: &str,
        stmt: &Statement,
    ) {
        self.generate_statement_impl(store, store_name, stmt, true)
    }

    #[allow(clippy::only_used_in_recursion)]
    fn generate_statement_impl(
        &mut self,
        store: &StoreDef,
        store_name: &str,
        stmt: &Statement,
        track_locals: bool,
    ) {
        match stmt {
            Statement::Assignment { name, value } => {
                let val = self.generate_expression(value);
                self.emit_line(&format!("const {} = {};", name, val));
                // Track this variable so subsequent statements can reference it
                if track_locals {
                    self.local_params.insert(name.clone());
                }
            }
            Statement::Dispatch { store, action, args } => {
                let args_str = args
                    .iter()
                    .map(|a| self.generate_expression(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                let target_store = store.as_deref().unwrap_or(store_name);
                self.emit_line(&format!(
                    "dispatch('{}', '{}'{}{});",
                    target_store,
                    action,
                    if args_str.is_empty() { "" } else { ", " },
                    args_str
                ));
            }
            Statement::Navigate { path } => {
                // Check if it's a route reference (.home, .docs.installation)
                if let Expression::RouteRef { .. } = path {
                    let route_expr = self.generate_expression(path);
                    self.emit_line(&format!("navigateWithGuards({});", route_expr));
                } else {
                    let path_expr = self.generate_expression(path);
                    self.emit_line(&format!("navigate({});", path_expr));
                }
            }
            Statement::TryCatch {
                try_block,
                catch_param,
                catch_block,
            } => {
                self.emit_line("try {");
                self.indent += 1;
                for s in try_block {
                    self.generate_statement_impl(store, store_name, s, track_locals);
                }
                self.indent -= 1;
                // Add catch parameter to local_params for catch block
                if track_locals {
                    self.local_params.insert(catch_param.clone());
                }
                self.emit_line(&format!("}} catch ({}) {{", catch_param));
                self.indent += 1;
                for s in catch_block {
                    self.generate_statement_impl(store, store_name, s, track_locals);
                }
                self.indent -= 1;
                self.emit_line("}");
            }
            Statement::If {
                condition,
                then_block,
                else_block,
            } => {
                let cond = self.generate_expression(condition);
                self.emit_line(&format!("if ({}) {{", cond));
                self.indent += 1;
                for s in then_block {
                    self.generate_statement_impl(store, store_name, s, track_locals);
                }
                self.indent -= 1;
                if let Some(else_stmts) = else_block {
                    self.emit_line("} else {");
                    self.indent += 1;
                    for s in else_stmts {
                        self.generate_statement_impl(store, store_name, s, track_locals);
                    }
                    self.indent -= 1;
                }
                self.emit_line("}");
            }
            Statement::Await { expr } => {
                let e = self.generate_expression(expr);
                self.emit_line(&format!("await {};", e));
            }
            Statement::Expression(expr) => {
                let e = self.generate_expression(expr);
                self.emit_line(&format!("{};", e));
            }
        }
    }

    /// Collect validation rules from property annotations (excluding @label)
    pub(super) fn collect_validation_rules(
        &mut self,
        fields: &[Property],
    ) -> Vec<(String, Vec<String>)> {
        let mut result = Vec::new();

        for field in fields {
            let validation_annotations: Vec<_> = field
                .annotations
                .iter()
                .filter(|a| a.name != "label")
                .collect();

            if !validation_annotations.is_empty() {
                let rules: Vec<String> = validation_annotations
                    .iter()
                    .map(|ann| {
                        if ann.args.is_empty() {
                            format!("{{ name: '{}' }}", ann.name)
                        } else {
                            let args: Vec<String> = ann
                                .args
                                .iter()
                                .map(|arg| self.generate_expression(arg))
                                .collect();
                            format!("{{ name: '{}', args: [{}] }}", ann.name, args.join(", "))
                        }
                    })
                    .collect();
                result.push((field.key.clone(), rules));
            }
        }

        result
    }

    /// Collect field keys from @key annotations (for i18n and form generation)
    pub(super) fn collect_field_keys(&mut self, fields: &[Property]) -> Vec<(String, String)> {
        let mut result = Vec::new();

        for field in fields {
            // Look for @key annotation
            let key = field
                .annotations
                .iter()
                .find(|a| a.name == "key")
                .and_then(|a| a.args.first())
                .map(|arg| {
                    if let Expression::String { value } = arg {
                        value.clone()
                    } else {
                        field.key.clone()
                    }
                })
                .unwrap_or_else(|| field.key.clone());

            result.push((field.key.clone(), key));
        }

        result
    }

    /// Check if a field has @hidden annotation
    pub(super) fn is_hidden_field(&self, field: &Property) -> bool {
        field.annotations.iter().any(|a| a.name == "hidden")
    }

    /// Collect form field info: (field_name, i18n_key, input_type)
    pub(super) fn collect_form_fields(
        &mut self,
        fields: &[Property],
    ) -> Vec<(String, String, String)> {
        let mut result = Vec::new();

        for field in fields {
            // Skip hidden fields
            if self.is_hidden_field(field) {
                continue;
            }

            // Only include fields with @key annotation
            let key = field
                .annotations
                .iter()
                .find(|a| a.name == "key")
                .and_then(|a| a.args.first())
                .and_then(|arg| {
                    if let Expression::String { value } = arg {
                        Some(value.clone())
                    } else {
                        None
                    }
                });

            if let Some(i18n_key) = key {
                // Determine input type from annotations
                let input_type = if field.annotations.iter().any(|a| a.name == "email") {
                    "email".to_string()
                } else if field.key.contains("password")
                    || field.annotations.iter().any(|a| a.name == "password")
                {
                    "password".to_string()
                } else {
                    "text".to_string()
                };

                result.push((field.key.clone(), i18n_key, input_type));
            }
        }

        result
    }
}
