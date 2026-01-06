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

    pub fn generate(&mut self, program: &Program) -> String {
        // First pass: collect API service names and find App component
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
        }

        self.emit_runtime_imports();
        self.emit_line("");

        for decl in &program.declarations {
            self.generate_declaration(decl);
            self.emit_line("");
        }

        // Auto-mount App component if it exists
        if has_app {
            self.emit_line("// Mount app");
            self.emit_line("mount(App, '#app');");
        }

        self.output.clone()
    }

    fn emit_runtime_imports(&mut self) {
        // Inline minimal runtime
        self.emit_line("// topo runtime");
        self.emit_line("const stores = new Map();");
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
        self.emit_line("  const { type, content, value, style, children, align } = vdom;");
        self.emit_line("  const styleAttr = style ? ` class=\"${style}\"` : '';");
        self.emit_line("  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';");
        self.emit_line("  ");
        self.emit_line("  if (type === 'text') {");
        self.emit_line("    return `<span${styleAttr}>${content || value || ''}</span>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'button') {");
        self.emit_line("    return `<button${styleAttr} data-click=\"true\">${content || ''}</button>`;");
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
    }

    fn generate_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Component(comp) => self.generate_component(comp),
            Declaration::Store(store) => self.generate_store(store),
            Declaration::ApiService(api) => self.generate_api_service(api),
            Declaration::Method(method) => self.generate_method(method),
        }
    }

    fn generate_component(&mut self, comp: &ComponentDef) {
        self.emit_line(&format!("function {}() {{", comp.name));
        self.indent += 1;

        self.emit_line("return {");
        self.indent += 1;

        for (i, prop) in comp.properties.iter().enumerate() {
            let comma = if i < comp.properties.len() - 1 { "," } else { "" };
            // Track current property key for event handler detection
            self.current_property_key = Some(prop.key.clone());
            let value = self.generate_expression(&prop.value);
            self.current_property_key = None;
            self.emit_line(&format!("{}: {}{}", prop.key, value, comma));
        }

        self.indent -= 1;
        self.emit_line("};");

        self.indent -= 1;
        self.emit_line("}");
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

        self.indent -= 1;
        self.emit_line("};");
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

    fn generate_expression(&self, expr: &Expression) -> String {
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
        }
    }
}

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
}
