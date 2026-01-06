//! Code generation for topo language
//!
//! Converts AST to JavaScript code.

use crate::ast::*;
use std::collections::HashSet;

pub struct JsCodegen {
    output: String,
    indent: usize,
    /// State field names for current reducer context (for prefixing with "state.")
    state_fields: HashSet<String>,
    /// Reducer/effect parameter names (take precedence over state fields)
    local_params: HashSet<String>,
    /// Current property key being generated (for event handler detection)
    current_property_key: Option<String>,
    /// Known API service names (for adding Api suffix to references)
    api_service_names: HashSet<String>,
    /// Theme color names (for color: primary -> var(--primary))
    theme_colors: HashSet<String>,
    /// Theme color values for CSS generation
    theme_values: Vec<(String, String)>,
}

impl JsCodegen {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            state_fields: HashSet::new(),
            local_params: HashSet::new(),
            current_property_key: None,
            api_service_names: HashSet::new(),
            theme_colors: HashSet::new(),
            theme_values: Vec::new(),
        }
    }

    /// Check if current property is an event handler
    fn is_event_handler(&self) -> bool {
        if let Some(key) = &self.current_property_key {
            matches!(
                key.as_str(),
                "click" | "submit" | "change" | "input" | "focus" | "blur"
                    | "keydown" | "keyup" | "keypress" | "mousedown" | "mouseup"
                    | "mouseover" | "mouseout" | "mouseenter" | "mouseleave"
                    | "onInput"
            )
        } else {
            false
        }
    }

    /// Check if current property should have identifier values quoted
    fn should_quote_identifier(&self) -> bool {
        if let Some(key) = &self.current_property_key {
            matches!(key.as_str(), "type" | "align")
        } else {
            false
        }
    }

    /// Check if current property is a style/color property where theme vars apply
    fn is_style_property(&self) -> bool {
        if let Some(key) = &self.current_property_key {
            matches!(
                key.as_str(),
                "color" | "bg" | "background" | "backgroundColor" | "borderColor"
                    | "textColor" | "fill" | "stroke"
            )
        } else {
            false
        }
    }

    /// Check if identifier is a theme color
    fn is_theme_color(&self, name: &str) -> bool {
        self.theme_colors.contains(name)
    }

    /// Generate runtime code (call once at the beginning of build)
    pub fn generate_runtime(&mut self) -> String {
        self.emit_runtime_imports();
        self.emit_line("");
        std::mem::take(&mut self.output)
    }

    pub fn generate(&mut self, program: &Program) -> String {
        // First pass: collect API service names, find App component, and collect theme
        let mut has_app = false;
        for decl in &program.declarations {
            if let Declaration::ApiService(api) = decl {
                self.api_service_names.insert(api.name.clone());
            }
            if let Declaration::Component(comp) = decl {
                if comp.name == "App" {
                    has_app = true;
                }
            }
            // Collect theme colors from Theme definition
            if let Declaration::Theme(theme) = decl {
                self.collect_theme_from_def(theme);
            }
        }

        // Generate theme CSS injection if theme is defined
        if !self.theme_values.is_empty() {
            self.emit_theme_css();
            self.emit_line("");
        }

        for decl in &program.declarations {
            // Skip Theme - it's handled separately via CSS injection
            if matches!(decl, Declaration::Theme(_)) {
                continue;
            }
            self.generate_declaration(decl);
            self.emit_line("");
        }

        // Auto-mount App component if it exists
        if has_app {
            self.emit_line("// Mount app");
            self.emit_line("mount(App, '#app');");
        }

        std::mem::take(&mut self.output)
    }

    fn collect_theme_from_def(&mut self, theme: &ThemeDef) {
        for prop in &theme.properties {
            self.theme_colors.insert(prop.key.clone());
            // Extract color value
            let value = match &prop.value {
                Expression::String { value } => value.clone(),
                Expression::Identifier { name } => name.clone(),
                _ => self.generate_expression(&prop.value),
            };
            self.theme_values.push((prop.key.clone(), value));
        }
    }

    fn emit_theme_css(&mut self) {
        self.emit_line("// Theme CSS variables");
        self.emit_line("(function() {");
        self.indent += 1;
        self.emit_line("const style = document.createElement('style');");
        self.emit_line("style.textContent = `");

        // Generate :root CSS variables
        self.emit_line(":root {");
        let theme_values = self.theme_values.clone();
        for (name, value) in &theme_values {
            self.emit_line(&format!("  --{}: {};", name, value));
        }
        self.emit_line("}");

        // Generate body background if defined
        let has_background = theme_values.iter().any(|(k, _)| k == "background");
        if has_background {
            self.emit_line("body {");
            self.emit_line("  background-color: var(--background);");
            self.emit_line("}");
        }

        self.emit_line("`;");
        self.emit_line("document.head.appendChild(style);");
        self.indent -= 1;
        self.emit_line("})();");
    }

    fn emit_runtime_imports(&mut self) {
        // Inline minimal runtime
        self.emit_line("// topo runtime");
        self.emit_line("const stores = new Map();");
        self.emit_line("");
        self.emit_runtime_validators();
        self.emit_line("");
        self.emit_line("function createStore(name, initialState) {");
        self.emit_line("  const state = { ...initialState };");
        self.emit_line("  const listeners = [];");
        self.emit_line("  const reducers = new Map();");
        self.emit_line("  const effects = new Map();");
        self.emit_line("  const selectors = new Map();");
        self.emit_line("");
        self.emit_line("  const store = {");
        self.emit_line("    get state() { return state; },");
        self.emit_line("    on(action, reducer) { reducers.set(action, reducer); },");
        self.emit_line("    effect(action, handler) { effects.set(action, handler); },");
        self.emit_line("    selector(name, fn) { selectors.set(name, fn); },");
        self.emit_line("    subscribe(fn) { listeners.push(fn); },");
        self.emit_line("    dispatch(action, ...args) {");
        self.emit_line("      const reducer = reducers.get(action);");
        self.emit_line("      if (reducer) {");
        self.emit_line("        Object.assign(state, reducer(state, ...args));");
        self.emit_line("        listeners.forEach(fn => fn(state));");
        self.emit_line("      }");
        self.emit_line("      const effect = effects.get(action);");
        self.emit_line("      if (effect) effect(...args);");
        self.emit_line("    },");
        self.emit_line("    select(name) {");
        self.emit_line("      const selector = selectors.get(name);");
        self.emit_line("      return selector ? selector(state) : undefined;");
        self.emit_line("    }");
        self.emit_line("  };");
        self.emit_line("  stores.set(name, store);");
        self.emit_line("  return store;");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function dispatch(storeName, action, ...args) {");
        self.emit_line("  const store = stores.get(storeName);");
        self.emit_line("  if (store) store.dispatch(action, ...args);");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function mount(componentFn, container) {");
        self.emit_line("  const el = document.querySelector(container);");
        self.emit_line("  if (!el) return;");
        self.emit_line("  const render = () => {");
        self.emit_line("    const vdom = componentFn();");
        self.emit_line("    el.innerHTML = renderVdom(vdom);");
        self.emit_line("    bindEvents(el, vdom);");
        self.emit_line("  };");
        self.emit_line("  stores.forEach(store => store.subscribe(render));");
        self.emit_line("  render();");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function renderVdom(vdom) {");
        self.emit_line("  if (!vdom) return '';");
        self.emit_line("  const { type, content, value, style, children, align, inputType, placeholder } = vdom;");
        self.emit_line("  const styleAttr = style ? ` class=\"${style}\"` : '';");
        self.emit_line("  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';");
        self.emit_line("  ");
        self.emit_line("  if (type === 'text') {");
        self.emit_line("    return `<span${styleAttr}>${content || value || ''}</span>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'button') {");
        self.emit_line("    return `<button${styleAttr} data-click=\"true\">${content || ''}</button>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'input') {");
        self.emit_line("    const inputTypeAttr = inputType || 'text';");
        self.emit_line("    const placeholderAttr = placeholder ? ` placeholder=\"${placeholder}\"` : '';");
        self.emit_line("    const valueAttr = value !== undefined ? ` value=\"${value}\"` : '';");
        self.emit_line("    return `<input type=\"${inputTypeAttr}\"${styleAttr}${placeholderAttr}${valueAttr} data-input=\"true\" />`;");
        self.emit_line("  }");
        self.emit_line("  if (children) {");
        self.emit_line("    const inner = children.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');");
        self.emit_line("    return `<div class=\"${(style || '') + flexClass}\">${inner}</div>`;");
        self.emit_line("  }");
        self.emit_line("  return `<div${styleAttr}>${content || value || ''}</div>`;");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function bindEvents(el, vdom) {");
        self.emit_line("  el.querySelectorAll('[data-click]').forEach((btn, i) => {");
        self.emit_line("    const handler = findClickHandler(vdom, i);");
        self.emit_line("    if (handler) btn.onclick = handler;");
        self.emit_line("  });");
        self.emit_line("  el.querySelectorAll('[data-input]').forEach((input, i) => {");
        self.emit_line("    const handler = findInputHandler(vdom, i);");
        self.emit_line("    if (handler) input.oninput = (e) => handler(e.target.value);");
        self.emit_line("  });");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function findClickHandler(vdom, index, count = { n: 0 }) {");
        self.emit_line("  if (!vdom) return null;");
        self.emit_line("  if (vdom.click && count.n++ === index) return vdom.click;");
        self.emit_line("  if (vdom.children) {");
        self.emit_line("    for (const c of vdom.children) {");
        self.emit_line("      const child = typeof c === 'function' ? c() : c;");
        self.emit_line("      const h = findClickHandler(child, index, count);");
        self.emit_line("      if (h) return h;");
        self.emit_line("    }");
        self.emit_line("  }");
        self.emit_line("  return null;");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function findInputHandler(vdom, index, count = { n: 0 }) {");
        self.emit_line("  if (!vdom) return null;");
        self.emit_line("  if (vdom.input && count.n++ === index) return vdom.input;");
        self.emit_line("  if (vdom.children) {");
        self.emit_line("    for (const c of vdom.children) {");
        self.emit_line("      const child = typeof c === 'function' ? c() : c;");
        self.emit_line("      const h = findInputHandler(child, index, count);");
        self.emit_line("      if (h) return h;");
        self.emit_line("    }");
        self.emit_line("  }");
        self.emit_line("  return null;");
        self.emit_line("}");
    }

    fn emit_runtime_validators(&mut self) {
        self.emit_line("// Validators");
        self.emit_line("const validators = {");
        self.indent += 1;

        // required
        self.emit_line("required(value, _args, field) {");
        self.emit_line("  if (value === null || value === undefined || value === '') {");
        self.emit_line("    return { valid: false, error: `${field} is required` };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // min - minimum value for numbers, minimum length for strings
        self.emit_line("min(value, args, field) {");
        self.emit_line("  const min = args[0];");
        self.emit_line("  if (typeof value === 'string' && value.length < min) {");
        self.emit_line("    return { valid: false, error: `${field} must be at least ${min} characters` };");
        self.emit_line("  }");
        self.emit_line("  if (typeof value === 'number' && value < min) {");
        self.emit_line("    return { valid: false, error: `${field} must be at least ${min}` };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // max - maximum value for numbers, maximum length for strings
        self.emit_line("max(value, args, field) {");
        self.emit_line("  const max = args[0];");
        self.emit_line("  if (typeof value === 'string' && value.length > max) {");
        self.emit_line("    return { valid: false, error: `${field} must be at most ${max} characters` };");
        self.emit_line("  }");
        self.emit_line("  if (typeof value === 'number' && value > max) {");
        self.emit_line("    return { valid: false, error: `${field} must be at most ${max}` };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // minLength - minimum length for strings
        self.emit_line("minLength(value, args, field) {");
        self.emit_line("  const min = args[0];");
        self.emit_line("  if (typeof value === 'string' && value.length < min) {");
        self.emit_line("    return { valid: false, error: `${field} must be at least ${min} characters` };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // maxLength - maximum length for strings
        self.emit_line("maxLength(value, args, field) {");
        self.emit_line("  const max = args[0];");
        self.emit_line("  if (typeof value === 'string' && value.length > max) {");
        self.emit_line("    return { valid: false, error: `${field} must be at most ${max} characters` };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // email
        self.emit_line("email(value, _args, field) {");
        self.emit_line("  const emailRegex = /^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/;");
        self.emit_line("  if (typeof value === 'string' && !emailRegex.test(value)) {");
        self.emit_line("    return { valid: false, error: `${field} must be a valid email address` };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // pattern - regex pattern
        self.emit_line("pattern(value, args, field) {");
        self.emit_line("  const pattern = new RegExp(args[0]);");
        self.emit_line("  if (typeof value === 'string' && !pattern.test(value)) {");
        self.emit_line("    return { valid: false, error: `${field} does not match the required pattern` };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // url
        self.emit_line("url(value, _args, field) {");
        self.emit_line("  try {");
        self.emit_line("    new URL(value);");
        self.emit_line("    return { valid: true };");
        self.emit_line("  } catch {");
        self.emit_line("    return { valid: false, error: `${field} must be a valid URL` };");
        self.emit_line("  }");
        self.emit_line("},");

        // alphanumeric
        self.emit_line("alphanumeric(value, _args, field) {");
        self.emit_line("  if (typeof value === 'string' && !/^[a-zA-Z0-9]+$/.test(value)) {");
        self.emit_line("    return { valid: false, error: `${field} must contain only letters and numbers` };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // range - between min and max
        self.emit_line("range(value, args, field) {");
        self.emit_line("  const [min, max] = args;");
        self.emit_line("  if (typeof value === 'number' && (value < min || value > max)) {");
        self.emit_line("    return { valid: false, error: `${field} must be between ${min} and ${max}` };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        self.indent -= 1;
        self.emit_line("};");
        self.emit_line("");

        // validate function
        self.emit_line("function validate(data, rules) {");
        self.emit_line("  const errors = {};");
        self.emit_line("  for (const [field, fieldRules] of Object.entries(rules)) {");
        self.emit_line("    const value = data[field];");
        self.emit_line("    for (const rule of fieldRules) {");
        self.emit_line("      const validator = validators[rule.name];");
        self.emit_line("      if (validator) {");
        self.emit_line("        const result = validator(value, rule.args || [], field);");
        self.emit_line("        if (!result.valid) {");
        self.emit_line("          errors[field] = errors[field] || [];");
        self.emit_line("          errors[field].push(result.error);");
        self.emit_line("        }");
        self.emit_line("      }");
        self.emit_line("    }");
        self.emit_line("  }");
        self.emit_line("  return { valid: Object.keys(errors).length === 0, errors };");
        self.emit_line("}");
    }

    fn generate_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Import(_) => {} // Imports are resolved at build time
            Declaration::Component(comp) => self.generate_component(comp),
            Declaration::Store(store) => self.generate_store(store),
            Declaration::ApiService(api) => self.generate_api_service(api),
            Declaration::Method(method) => self.generate_method(method),
            Declaration::Theme(_) => {} // Handled separately via CSS injection
        }
    }

    fn generate_component(&mut self, comp: &ComponentDef) {
        // Store component params for expression generation
        let old_params = std::mem::take(&mut self.local_params);
        for param in &comp.params {
            self.local_params.insert(param.name.clone());
        }

        let params_str = if comp.params.is_empty() {
            String::new()
        } else {
            comp.params.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(", ")
        };

        self.emit_line(&format!("function {}({}) {{", comp.name, params_str));
        self.indent += 1;

        self.emit_line("return {");
        self.indent += 1;

        // Count total properties including lifecycle hooks
        let has_lifecycle = comp.init.is_some() || comp.destroy.is_some();
        let total_props = comp.properties.len() + if has_lifecycle { 1 } else { 0 };
        let mut prop_index = 0;

        for prop in &comp.properties {
            prop_index += 1;
            let comma = if prop_index < total_props { "," } else { "" };
            // Track current property key for event handler detection
            self.current_property_key = Some(prop.key.clone());
            let value = self.generate_expression(&prop.value);
            self.current_property_key = None;
            self.emit_line(&format!("{}: {}{}", prop.key, value, comma));
        }

        // Generate lifecycle hooks
        if comp.init.is_some() || comp.destroy.is_some() {
            let init_code = comp.init.as_ref()
                .map(|e| self.generate_expression(e))
                .unwrap_or_else(|| "null".to_string());
            let destroy_code = comp.destroy.as_ref()
                .map(|e| self.generate_expression(e))
                .unwrap_or_else(|| "null".to_string());
            self.emit_line(&format!("lifecycle: {{ init: {}, destroy: {} }}", init_code, destroy_code));
        }

        self.indent -= 1;
        self.emit_line("};");

        self.indent -= 1;
        self.emit_line("}");

        // Restore old params
        self.local_params = old_params;
    }

    fn generate_store(&mut self, store: &StoreDef) {
        // Generate store creation
        self.emit_line(&format!("const {} = createStore('{}', {{", store.name, store.name));
        self.indent += 1;

        if let Some(state) = &store.state {
            for (i, field) in state.fields.iter().enumerate() {
                let comma = if i < state.fields.len() - 1 { "," } else { "" };
                let value = self.generate_expression(&field.value);
                self.emit_line(&format!("{}: {}{}", field.key, value, comma));
            }
        }

        self.indent -= 1;
        self.emit_line("});");
        self.emit_line("");

        // Generate validation rules from annotations
        if let Some(state) = &store.state {
            let validation_rules = self.collect_validation_rules(&state.fields);
            if !validation_rules.is_empty() {
                self.emit_line(&format!("const {}ValidationRules = {{", store.name));
                self.indent += 1;
                for (i, (field, rules)) in validation_rules.iter().enumerate() {
                    let comma = if i < validation_rules.len() - 1 { "," } else { "" };
                    self.emit_line(&format!("{}: [{}]{}", field, rules.join(", "), comma));
                }
                self.indent -= 1;
                self.emit_line("};");
                self.emit_line("");

                // Generate validate helper for this store
                self.emit_line(&format!("function validate{}(data) {{", store.name));
                self.indent += 1;
                self.emit_line(&format!("return validate(data, {}ValidationRules);", store.name));
                self.indent -= 1;
                self.emit_line("}");
                self.emit_line("");
            }
        }

        // Generate reducers
        if let Some(reducers) = &store.reducers {
            for handler in &reducers.handlers {
                self.generate_reducer(store, handler);
            }
        }

        // Generate effects
        if let Some(effects) = &store.effects {
            for handler in &effects.handlers {
                self.generate_effect(store, handler);
            }
        }

        // Generate selectors
        if let Some(selectors) = &store.selectors {
            for selector in &selectors.selectors {
                self.generate_selector(store, selector);
            }
        }
    }

    fn generate_reducer(&mut self, store: &StoreDef, handler: &ReducerHandler) {
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

        self.emit_line(&format!(
            "{}.on('{}', (state{}) => ({{",
            store.name, handler.action, params
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

    fn generate_effect(&mut self, store: &StoreDef, handler: &EffectHandler) {
        let params = if handler.params.is_empty() {
            String::new()
        } else {
            handler.params.join(", ")
        };

        self.emit_line(&format!(
            "{}.effect('{}', async ({}) => {{",
            store.name, handler.action, params
        ));
        self.indent += 1;

        for stmt in &handler.body {
            self.generate_statement(store, stmt);
        }

        self.indent -= 1;
        self.emit_line("});");
        self.emit_line("");
    }

    fn generate_selector(&mut self, store: &StoreDef, selector: &SelectorDef) {
        // Collect state field names for context-aware expression generation
        self.state_fields.clear();
        if let Some(state) = &store.state {
            for field in &state.fields {
                self.state_fields.insert(field.key.clone());
            }
        }

        let body = self.generate_expression(&selector.body);
        self.emit_line(&format!(
            "{}.selector('{}', (state) => {});",
            store.name, selector.name, body
        ));

        self.state_fields.clear();
    }

    fn generate_statement(&mut self, store: &StoreDef, stmt: &Statement) {
        match stmt {
            Statement::Assignment { name, value } => {
                let val = self.generate_expression(value);
                self.emit_line(&format!("const {} = {};", name, val));
            }
            Statement::Dispatch { action, args } => {
                let args_str = args
                    .iter()
                    .map(|a| self.generate_expression(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.emit_line(&format!(
                    "dispatch('{}', '{}'{}{});",
                    store.name,
                    action,
                    if args_str.is_empty() { "" } else { ", " },
                    args_str
                ));
            }
            Statement::TryCatch {
                try_block,
                catch_param,
                catch_block,
            } => {
                self.emit_line("try {");
                self.indent += 1;
                for s in try_block {
                    self.generate_statement(store, s);
                }
                self.indent -= 1;
                self.emit_line(&format!("}} catch ({}) {{", catch_param));
                self.indent += 1;
                for s in catch_block {
                    self.generate_statement(store, s);
                }
                self.indent -= 1;
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

    fn generate_api_service(&mut self, api: &ApiServiceDef) {
        // Check if this is a subscriber-only service (no rest endpoints)
        let is_subscriber_only = api.rest.is_none() && api.endpoints.is_empty() && api.subscribe.is_some();

        if is_subscriber_only {
            // Generate subscriber-only service
            self.generate_subscriber(api);
            return;
        }

        // Use Api suffix to avoid name collision with same-name Store
        self.emit_line(&format!("const {}Api = {{", api.name));
        self.indent += 1;

        if let Some(base_url) = &api.rest {
            // Generate CRUD methods
            self.emit_line(&format!("baseUrl: '{}',", base_url));
            self.emit_line("");

            self.emit_line("async getAll() {");
            self.indent += 1;
            self.emit_line(&format!("return fetch('{}').then(r => r.json());", base_url));
            self.indent -= 1;
            self.emit_line("},");
            self.emit_line("");

            self.emit_line("async getById(id) {");
            self.indent += 1;
            self.emit_line(&format!("return fetch(`{}/${{id}}`).then(r => r.json());", base_url));
            self.indent -= 1;
            self.emit_line("},");
            self.emit_line("");

            self.emit_line("async create(data) {");
            self.indent += 1;
            self.emit_line(&format!(
                "return fetch('{}', {{ method: 'POST', body: JSON.stringify(data), headers: {{ 'Content-Type': 'application/json' }} }}).then(r => r.json());",
                base_url
            ));
            self.indent -= 1;
            self.emit_line("},");
            self.emit_line("");

            self.emit_line("async update(id, data) {");
            self.indent += 1;
            self.emit_line(&format!(
                "return fetch(`{}/${{id}}`, {{ method: 'PUT', body: JSON.stringify(data), headers: {{ 'Content-Type': 'application/json' }} }}).then(r => r.json());",
                base_url
            ));
            self.indent -= 1;
            self.emit_line("},");
            self.emit_line("");

            self.emit_line("async delete(id) {");
            self.indent += 1;
            self.emit_line(&format!(
                "return fetch(`{}/${{id}}`, {{ method: 'DELETE' }});",
                base_url
            ));
            self.indent -= 1;
            self.emit_line("},");
        }

        // Generate custom endpoints
        for endpoint in &api.endpoints {
            self.generate_endpoint(api, endpoint);
        }

        // Generate subscriber methods if subscribe URL is present
        if api.subscribe.is_some() {
            self.emit_line("");
            self.generate_subscriber_methods(api);
        }

        self.indent -= 1;
        self.emit_line("};");
    }

    fn generate_subscriber(&mut self, api: &ApiServiceDef) {
        let subscribe_url = api.subscribe.as_ref().unwrap();
        let is_websocket = subscribe_url.starts_with("ws://") || subscribe_url.starts_with("wss://");

        self.emit_line(&format!("const {}Subscriber = {{", api.name));
        self.indent += 1;

        self.emit_line("connection: null,");
        self.emit_line("");

        // connect() method
        self.emit_line("connect() {");
        self.indent += 1;

        if is_websocket {
            // WebSocket connection
            self.emit_line(&format!("this.connection = new WebSocket('{}');", subscribe_url));
            self.emit_line("");

            // Generate event handlers
            for handler in &api.event_handlers {
                let event_name = match handler.event {
                    EventType::Message => "onmessage",
                    EventType::Error => "onerror",
                    EventType::Open => "onopen",
                    EventType::Close => "onclose",
                };

                if handler.event == EventType::Message {
                    self.emit_line(&format!("this.connection.{} = (event) => {{", event_name));
                    self.indent += 1;
                    self.emit_line("const data = JSON.parse(event.data);");
                    self.emit_line(&format!("dispatch('{}', '{}', data);", api.name, handler.action));
                    self.indent -= 1;
                    self.emit_line("};");
                } else {
                    self.emit_line(&format!("this.connection.{} = (event) => {{", event_name));
                    self.indent += 1;
                    self.emit_line(&format!("dispatch('{}', '{}', event);", api.name, handler.action));
                    self.indent -= 1;
                    self.emit_line("};");
                }
            }
        } else {
            // EventSource (SSE) connection
            self.emit_line(&format!("this.connection = new EventSource('{}');", subscribe_url));
            self.emit_line("");

            // Generate event handlers
            for handler in &api.event_handlers {
                let event_name = match handler.event {
                    EventType::Message => "onmessage",
                    EventType::Error => "onerror",
                    EventType::Open => "onopen",
                    EventType::Close => "onclose", // Note: EventSource doesn't have onclose, but we handle it for consistency
                };

                if handler.event == EventType::Close {
                    // EventSource doesn't have onclose, skip it
                    continue;
                }

                if handler.event == EventType::Message {
                    self.emit_line(&format!("this.connection.{} = (event) => {{", event_name));
                    self.indent += 1;
                    self.emit_line("const data = JSON.parse(event.data);");
                    self.emit_line(&format!("dispatch('{}', '{}', data);", api.name, handler.action));
                    self.indent -= 1;
                    self.emit_line("};");
                } else {
                    self.emit_line(&format!("this.connection.{} = (event) => {{", event_name));
                    self.indent += 1;
                    self.emit_line(&format!("dispatch('{}', '{}', event);", api.name, handler.action));
                    self.indent -= 1;
                    self.emit_line("};");
                }
            }
        }

        self.indent -= 1;
        self.emit_line("},");
        self.emit_line("");

        // disconnect() method
        self.emit_line("disconnect() {");
        self.indent += 1;
        self.emit_line("if (this.connection) {");
        self.indent += 1;
        self.emit_line("this.connection.close();");
        self.emit_line("this.connection = null;");
        self.indent -= 1;
        self.emit_line("}");
        self.indent -= 1;
        self.emit_line("},");
        self.emit_line("");

        // send() method (WebSocket only)
        if is_websocket {
            self.emit_line("send(data) {");
            self.indent += 1;
            self.emit_line("if (this.connection && this.connection.readyState === WebSocket.OPEN) {");
            self.indent += 1;
            self.emit_line("this.connection.send(JSON.stringify(data));");
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("},");
        }

        self.indent -= 1;
        self.emit_line("};");
    }

    fn generate_subscriber_methods(&mut self, api: &ApiServiceDef) {
        let subscribe_url = api.subscribe.as_ref().unwrap();
        let is_websocket = subscribe_url.starts_with("ws://") || subscribe_url.starts_with("wss://");

        self.emit_line("_subscription: null,");
        self.emit_line("");

        // subscribe() method
        self.emit_line("subscribe() {");
        self.indent += 1;

        if is_websocket {
            self.emit_line(&format!("this._subscription = new WebSocket('{}');", subscribe_url));

            for handler in &api.event_handlers {
                let event_name = match handler.event {
                    EventType::Message => "onmessage",
                    EventType::Error => "onerror",
                    EventType::Open => "onopen",
                    EventType::Close => "onclose",
                };

                if handler.event == EventType::Message {
                    self.emit_line(&format!("this._subscription.{} = (event) => {{", event_name));
                    self.indent += 1;
                    self.emit_line("const data = JSON.parse(event.data);");
                    self.emit_line(&format!("dispatch('{}', '{}', data);", api.name, handler.action));
                    self.indent -= 1;
                    self.emit_line("};");
                } else {
                    self.emit_line(&format!("this._subscription.{} = (event) => {{", event_name));
                    self.indent += 1;
                    self.emit_line(&format!("dispatch('{}', '{}', event);", api.name, handler.action));
                    self.indent -= 1;
                    self.emit_line("};");
                }
            }
        } else {
            self.emit_line(&format!("this._subscription = new EventSource('{}');", subscribe_url));

            for handler in &api.event_handlers {
                if handler.event == EventType::Close {
                    continue;
                }

                let event_name = match handler.event {
                    EventType::Message => "onmessage",
                    EventType::Error => "onerror",
                    EventType::Open => "onopen",
                    EventType::Close => continue,
                };

                if handler.event == EventType::Message {
                    self.emit_line(&format!("this._subscription.{} = (event) => {{", event_name));
                    self.indent += 1;
                    self.emit_line("const data = JSON.parse(event.data);");
                    self.emit_line(&format!("dispatch('{}', '{}', data);", api.name, handler.action));
                    self.indent -= 1;
                    self.emit_line("};");
                } else {
                    self.emit_line(&format!("this._subscription.{} = (event) => {{", event_name));
                    self.indent += 1;
                    self.emit_line(&format!("dispatch('{}', '{}', event);", api.name, handler.action));
                    self.indent -= 1;
                    self.emit_line("};");
                }
            }
        }

        self.indent -= 1;
        self.emit_line("},");
        self.emit_line("");

        // unsubscribe() method
        self.emit_line("unsubscribe() {");
        self.indent += 1;
        self.emit_line("if (this._subscription) {");
        self.indent += 1;
        self.emit_line("this._subscription.close();");
        self.emit_line("this._subscription = null;");
        self.indent -= 1;
        self.emit_line("}");
        self.indent -= 1;
        self.emit_line("},");
    }

    fn generate_endpoint(&mut self, api: &ApiServiceDef, endpoint: &Endpoint) {
        let method = match endpoint.method {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
        };

        let base = api.rest.as_deref().unwrap_or("");
        let full_path = format!("{}{}", base, endpoint.path);

        self.emit_line(&format!("async {}(params) {{", endpoint.name));
        self.indent += 1;
        self.emit_line(&format!(
            "return fetch('{}', {{ method: '{}' }}).then(r => r.json());",
            full_path, method
        ));
        self.indent -= 1;
        self.emit_line("},");
    }

    fn generate_method(&mut self, method: &MethodDef) {
        let body = self.generate_expression(&method.body);
        self.emit_line(&format!("function {}() {{ return {}; }}", method.name, body));
    }

    fn generate_expression(&mut self, expr: &Expression) -> String {
        match expr {
            Expression::String { value } => format!("'{}'", value.replace('\'', "\\'")),
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
                } else {
                    name.clone()
                }
            }
            Expression::Array { elements } => {
                let elems: Vec<String> = elements.iter().map(|e| self.generate_expression(e)).collect();
                format!("[{}]", elems.join(", "))
            }
            Expression::ForIn { item, items, body } => {
                let items_str = self.generate_expression(items);
                // Add item to local params for body generation
                self.local_params.insert(item.clone());
                let body_str = self.generate_expression(body);
                self.local_params.remove(item);
                format!("{}.map({} => {})", items_str, item, body_str)
            }
            Expression::Object { properties } => {
                let props: Vec<String> = properties
                    .iter()
                    .map(|p| format!("{}: {}", p.key, self.generate_expression(&p.value)))
                    .collect();
                format!("{{ {} }}", props.join(", "))
            }
            Expression::Reference { store, path } => {
                if path.is_empty() {
                    format!("{}.state", store)
                } else {
                    format!("{}.state.{}", store, path.join("."))
                }
            }
            Expression::ActionRef { store, action, args } => {
                let args_str: Vec<String> = args.iter().map(|a| self.generate_expression(a)).collect();
                if args_str.is_empty() {
                    format!("() => dispatch('{}', '{}')", store, action)
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
                if self.is_event_handler() {
                    if let Expression::Identifier { name: store } = object.as_ref() {
                        // Generate as action dispatch: () => dispatch('Store', 'Action')
                        return format!("() => dispatch('{}', '{}')", store, property);
                    }
                }
                // Check if object is an API service name - add Api suffix
                if let Expression::Identifier { name } = object.as_ref() {
                    if self.api_service_names.contains(name) {
                        return format!("{}Api.{}", name, property);
                    }
                }
                let obj = self.generate_expression(object);
                format!("{}.{}", obj, property)
            }
            Expression::Call { callee, args } => {
                let c = self.generate_expression(callee);
                let args_str: Vec<String> = args.iter().map(|a| self.generate_expression(a)).collect();
                format!("{}({})", c, args_str.join(", "))
            }
            Expression::Await { expr } => {
                let e = self.generate_expression(expr);
                format!("await {}", e)
            }
        }
    }

    fn emit_line(&mut self, line: &str) {
        let indent = "  ".repeat(self.indent);
        self.output.push_str(&indent);
        self.output.push_str(line);
        self.output.push('\n');
    }

    /// Collect validation rules from property annotations
    fn collect_validation_rules(&mut self, fields: &[Property]) -> Vec<(String, Vec<String>)> {
        let mut result = Vec::new();

        for field in fields {
            if !field.annotations.is_empty() {
                let rules: Vec<String> = field.annotations.iter()
                    .map(|ann| {
                        if ann.args.is_empty() {
                            format!("{{ name: '{}' }}", ann.name)
                        } else {
                            let args: Vec<String> = ann.args.iter()
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
}

impl Default for JsCodegen {
    fn default() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            state_fields: HashSet::new(),
            local_params: HashSet::new(),
            current_property_key: None,
            api_service_names: HashSet::new(),
            theme_colors: HashSet::new(),
            theme_values: Vec::new(),
        }
    }
}

// ============================================================================
// TypeScript Type Generation
// ============================================================================

/// Generates TypeScript type definitions from the AST
pub struct TsCodegen {
    output: String,
    indent: usize,
}

impl TsCodegen {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    /// Generate TypeScript type definitions
    pub fn generate(&mut self, program: &Program) -> String {
        self.emit_line("// Generated TypeScript definitions for topo");
        self.emit_line("// Do not edit manually");
        self.emit_line("");

        for decl in &program.declarations {
            self.generate_declaration(decl);
            self.emit_line("");
        }

        self.output.clone()
    }

    fn generate_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Import(_) => {} // Imports are resolved at build time
            Declaration::Component(comp) => self.generate_component_types(comp),
            Declaration::Store(store) => self.generate_store_types(store),
            Declaration::ApiService(api) => self.generate_api_types(api),
            Declaration::Theme(theme) => self.generate_theme_types(theme),
            Declaration::Method(_) => {} // Methods don't need type exports
        }
    }

    fn generate_component_types(&mut self, comp: &ComponentDef) {
        // Generate props interface if component has params
        if !comp.params.is_empty() {
            self.emit_line(&format!("export interface {}Props {{", comp.name));
            self.indent += 1;

            for param in &comp.params {
                let type_str = param
                    .type_annotation
                    .as_ref()
                    .map(|t| self.type_to_string(t))
                    .unwrap_or_else(|| "any".to_string());
                self.emit_line(&format!("{}: {};", param.name, type_str));
            }

            self.indent -= 1;
            self.emit_line("}");
            self.emit_line("");
        }

        // Generate component function type
        if comp.params.is_empty() {
            self.emit_line(&format!("export declare function {}(): VNode;", comp.name));
        } else {
            self.emit_line(&format!(
                "export declare function {}(props: {}Props): VNode;",
                comp.name, comp.name
            ));
        }
    }

    fn generate_store_types(&mut self, store: &StoreDef) {
        // Generate State interface
        self.emit_line(&format!("export interface {}State {{", store.name));
        self.indent += 1;

        if let Some(state) = &store.state {
            for field in &state.fields {
                let type_str = field
                    .type_annotation
                    .as_ref()
                    .map(|t| self.type_to_string(t))
                    .unwrap_or_else(|| self.infer_type_from_expr(&field.value));
                self.emit_line(&format!("{}: {};", field.key, type_str));
            }
        }

        self.indent -= 1;
        self.emit_line("}");
        self.emit_line("");

        // Generate Actions type
        if let Some(actions) = &store.actions {
            self.emit_line(&format!("export type {}Actions =", store.name));
            self.indent += 1;

            for (i, action) in actions.actions.iter().enumerate() {
                let params_str = if action.params.is_empty() {
                    String::new()
                } else {
                    action
                        .params
                        .iter()
                        .map(|p| {
                            let type_str = p
                                .type_annotation
                                .as_ref()
                                .map(|t| self.type_to_string(t))
                                .unwrap_or_else(|| "any".to_string());
                            format!("{}: {}", p.name, type_str)
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                let separator = if i < actions.actions.len() - 1 { " |" } else { ";" };
                self.emit_line(&format!(
                    "| {{ type: '{}'; payload: {{ {} }} }}{}",
                    action.name, params_str, separator
                ));
            }

            self.indent -= 1;
            self.emit_line("");
        }

        // Generate Store interface
        self.emit_line(&format!("export interface {}Store {{", store.name));
        self.indent += 1;
        self.emit_line(&format!("readonly state: {}State;", store.name));
        self.emit_line("subscribe(listener: (state: {}State) => void): void;");
        self.emit_line("dispatch(action: string, ...args: any[]): void;");

        // Add selectors if defined
        if let Some(selectors) = &store.selectors {
            for selector in &selectors.selectors {
                self.emit_line(&format!("select(name: '{}'): any;", selector.name));
            }
        }

        self.indent -= 1;
        self.emit_line("}");
        self.emit_line("");

        // Export store constant
        self.emit_line(&format!("export declare const {}: {}Store;", store.name, store.name));
    }

    fn generate_api_types(&mut self, api: &ApiServiceDef) {
        self.emit_line(&format!("export interface {}Api {{", api.name));
        self.indent += 1;

        if api.rest.is_some() {
            self.emit_line("getAll(): Promise<any[]>;");
            self.emit_line("getById(id: string | number): Promise<any>;");
            self.emit_line("create(data: any): Promise<any>;");
            self.emit_line("update(id: string | number, data: any): Promise<any>;");
            self.emit_line("delete(id: string | number): Promise<void>;");
        }

        for endpoint in &api.endpoints {
            self.emit_line(&format!("{}(params?: any): Promise<any>;", endpoint.name));
        }

        if api.subscribe.is_some() {
            self.emit_line("subscribe(): void;");
            self.emit_line("unsubscribe(): void;");
        }

        self.indent -= 1;
        self.emit_line("}");
        self.emit_line("");

        self.emit_line(&format!("export declare const {}Api: {}Api;", api.name, api.name));
    }

    fn generate_theme_types(&mut self, theme: &ThemeDef) {
        self.emit_line(&format!("export interface {}Theme {{", theme.name));
        self.indent += 1;

        for prop in &theme.properties {
            self.emit_line(&format!("{}: string;", prop.key));
        }

        self.indent -= 1;
        self.emit_line("}");
    }

    fn type_to_string(&self, ty: &TypeAnnotation) -> String {
        match ty {
            TypeAnnotation::Primitive { name } => {
                // Map topo primitives to TypeScript
                match name.as_str() {
                    "string" => "string".to_string(),
                    "number" => "number".to_string(),
                    "boolean" => "boolean".to_string(),
                    "null" => "null".to_string(),
                    "undefined" => "undefined".to_string(),
                    "void" => "void".to_string(),
                    "any" => "any".to_string(),
                    other => other.to_string(),
                }
            }
            TypeAnnotation::Array { element_type } => {
                format!("{}[]", self.type_to_string(element_type))
            }
            TypeAnnotation::Optional { inner_type } => {
                format!("{} | null | undefined", self.type_to_string(inner_type))
            }
            TypeAnnotation::Object { fields } => {
                let fields_str: Vec<String> = fields
                    .iter()
                    .map(|f| format!("{}: {}", f.name, self.type_to_string(&f.type_annotation)))
                    .collect();
                format!("{{ {} }}", fields_str.join("; "))
            }
            TypeAnnotation::Union { types } => {
                let types_str: Vec<String> = types.iter().map(|t| self.type_to_string(t)).collect();
                types_str.join(" | ")
            }
            TypeAnnotation::Reference { name } => name.clone(),
        }
    }

    fn infer_type_from_expr(&self, expr: &Expression) -> String {
        match expr {
            Expression::String { .. } => "string".to_string(),
            Expression::Number { .. } => "number".to_string(),
            Expression::Boolean { .. } => "boolean".to_string(),
            Expression::Null => "null".to_string(),
            Expression::Array { elements } => {
                if elements.is_empty() {
                    "any[]".to_string()
                } else {
                    format!("{}[]", self.infer_type_from_expr(&elements[0]))
                }
            }
            Expression::Object { properties } => {
                let fields: Vec<String> = properties
                    .iter()
                    .map(|p| format!("{}: {}", p.key, self.infer_type_from_expr(&p.value)))
                    .collect();
                format!("{{ {} }}", fields.join("; "))
            }
            _ => "any".to_string(),
        }
    }

    fn emit_line(&mut self, line: &str) {
        let indent = "  ".repeat(self.indent);
        self.output.push_str(&indent);
        self.output.push_str(line);
        self.output.push('\n');
    }
}

impl Default for TsCodegen {
    fn default() -> Self {
        Self::new()
    }
}

/// VNode type definition for runtime
pub const VNODE_TYPE_DEF: &str = r#"
export interface VNode {
  type?: string;
  content?: string;
  value?: any;
  style?: string;
  children?: (VNode | (() => VNode))[];
  click?: () => void;
  input?: (value: string) => void;
  align?: 'horizontal' | 'vertical';
  inputType?: string;
  placeholder?: string;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn generate(source: &str) -> String {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse().unwrap();
        let mut codegen = JsCodegen::new();
        codegen.generate(&program)
    }

    #[test]
    fn test_generate_component() {
        let source = r#"
            Button -> {
                type: button
                content: "Click"
            }
        "#;

        let output = generate(source);
        assert!(output.contains("function Button()"));
        assert!(output.contains("type: 'button'"));
    }

    #[test]
    fn test_generate_store() {
        let source = r#"
            Counter | {
                State {
                    count: 0
                }
            }
        "#;

        let output = generate(source);
        assert!(output.contains("createStore('Counter'"));
        assert!(output.contains("count: 0"));
    }

    fn generate_ts(source: &str) -> String {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse().unwrap();
        let mut codegen = TsCodegen::new();
        codegen.generate(&program)
    }

    #[test]
    fn test_generate_typescript_types() {
        let source = r#"
            Counter | {
                State {
                    count: number = 0
                    name: string = ""
                }
                Actions {
                    Increment
                    Add(amount: number)
                }
            }
        "#;

        let output = generate_ts(source);
        assert!(output.contains("export interface CounterState"));
        assert!(output.contains("count: number"));
        assert!(output.contains("name: string"));
        assert!(output.contains("export interface CounterStore"));
    }

    #[test]
    fn test_generate_component_props_types() {
        let source = r#"
            UserCard(name: string, age: number) -> {
                type: div
                content: name
            }
        "#;

        let output = generate_ts(source);
        assert!(output.contains("export interface UserCardProps"));
        assert!(output.contains("name: string"));
        assert!(output.contains("age: number"));
        assert!(output.contains("export declare function UserCard(props: UserCardProps): VNode"));
    }

    #[test]
    fn test_generate_lifecycle_hooks() {
        let source = r#"
            Timer -> {
                type: div
                content: "Timer"
                init: console.log("mounted")
                destroy: console.log("destroyed")
            }
        "#;

        let output = generate(source);
        assert!(output.contains("lifecycle:"));
        assert!(output.contains("init:"));
        assert!(output.contains("destroy:"));
    }
}
