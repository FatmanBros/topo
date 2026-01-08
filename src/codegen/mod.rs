//! Code generation for topo language
//!
//! Converts AST to JavaScript code.

use crate::ast::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;

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
    /// Component parameter names (for object-style props conversion)
    component_params: HashMap<String, Vec<String>>,
    /// Store names that have same-name components (need internal naming)
    stores_with_components: HashSet<String>,
    /// Store state fields: map from store name to set of state field names
    store_state_fields: HashMap<String, HashSet<String>>,
    /// Current file path being processed
    current_file_path: Option<String>,
    /// Current file's anonymous store name (derived from filename)
    current_file_store_name: Option<String>,
    /// Actions/state fields defined in anonymous store (for unqualified reference resolution)
    current_file_store_actions: HashSet<String>,
    current_file_store_fields: HashSet<String>,
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
            component_params: HashMap::new(),
            stores_with_components: HashSet::new(),
            store_state_fields: HashMap::new(),
            current_file_path: None,
            current_file_store_name: None,
            current_file_store_actions: HashSet::new(),
            current_file_store_fields: HashSet::new(),
        }
    }

    /// Convert filename to PascalCase for anonymous store naming
    /// Supports: kebab-case, snake_case, camelCase, PascalCase
    fn filename_to_pascal_case(filename: &str) -> String {
        let stem = Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(filename);

        let mut result = String::new();
        let mut capitalize_next = true;

        for ch in stem.chars() {
            if ch == '-' || ch == '_' {
                capitalize_next = true;
            } else if capitalize_next {
                result.push(ch.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                result.push(ch);
            }
        }

        result
    }

    /// Check if current property is an event handler
    fn is_event_handler(&self) -> bool {
        if let Some(key) = &self.current_property_key {
            matches!(
                key.as_str(),
                "click" | "onClick" | "submit" | "onSubmit" | "change" | "onChange"
                    | "input" | "onInput" | "focus" | "onFocus" | "blur" | "onBlur"
                    | "keydown" | "onKeydown" | "keyup" | "onKeyup" | "keypress" | "onKeypress"
                    | "mousedown" | "onMousedown" | "mouseup" | "onMouseup"
                    | "mouseover" | "onMouseover" | "mouseout" | "onMouseout"
                    | "mouseenter" | "onMouseenter" | "mouseleave" | "onMouseleave"
            )
        } else {
            false
        }
    }

    /// Check if current property is an input event handler that receives a value
    fn is_input_event_handler(&self) -> bool {
        if let Some(key) = &self.current_property_key {
            matches!(key.as_str(), "onInput" | "input" | "onChange" | "change")
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

    /// Generate responsive style value
    /// Handles both string styles and responsive style objects:
    /// - String: `"flex items-center"` -> `'flex items-center'`
    /// - Object: `{ common: "flex", desktop: "gap-8", tablet: "gap-4", mobile: "gap-2" }`
    ///   -> `'flex lg:gap-8 md:gap-4 gap-2'`
    ///
    /// Responsive prefixes (mobile-first approach):
    /// - common/mobile: no prefix (base styles)
    /// - tablet: md: prefix (≥768px)
    /// - desktop: lg: prefix (≥1024px)
    fn generate_responsive_style(&mut self, expr: &Expression) -> std::string::String {
        match expr {
            Expression::Object { properties } => {
                // Check if this is a responsive style object
                let responsive_keys = ["common", "base", "mobile", "tablet", "desktop"];
                let has_responsive_keys = properties.iter()
                    .any(|p| responsive_keys.contains(&p.key.as_str()));

                if has_responsive_keys {
                    // Collect classes for each breakpoint
                    let mut classes: Vec<std::string::String> = Vec::new();

                    for prop in properties {
                        let class_value = match &prop.value {
                            Expression::String { value } => value.clone(),
                            _ => self.generate_expression(&prop.value)
                                .trim_matches('\'')
                                .trim_matches('"')
                                .to_string(),
                        };

                        if class_value.is_empty() {
                            continue;
                        }

                        let prefix = match prop.key.as_str() {
                            "common" | "base" | "mobile" => "", // No prefix for base/mobile
                            "tablet" => "md:",
                            "desktop" => "lg:",
                            _ => continue, // Skip unknown keys
                        };

                        // Add prefix to each class
                        for class in class_value.split_whitespace() {
                            if prefix.is_empty() {
                                classes.push(class.to_string());
                            } else {
                                classes.push(format!("{}{}", prefix, class));
                            }
                        }
                    }

                    format!("'{}'", classes.join(" "))
                } else {
                    // Not a responsive object, generate as normal
                    self.generate_expression(expr)
                }
            }
            // For non-object expressions, use normal generation
            _ => self.generate_expression(expr),
        }
    }

    /// Get the JavaScript variable name for a store
    /// Returns `_StoreNameStore` if the store has a same-name component, otherwise `store_name`
    fn store_var_name(&self, store_name: &str) -> String {
        if self.stores_with_components.contains(store_name) {
            format!("_{}Store", store_name)
        } else {
            store_name.to_string()
        }
    }

    /// Generate runtime code (call once at the beginning of build)
    pub fn generate_runtime(&mut self) -> String {
        self.emit_runtime_imports();
        self.emit_line("");
        std::mem::take(&mut self.output)
    }

    /// Pre-collect component parameter names from a program
    /// Call this for all files before generating code to enable cross-file param detection
    pub fn collect_component_params(&mut self, program: &Program) {
        for decl in &program.declarations {
            if let Declaration::Component(comp) = decl {
                if !comp.params.is_empty() {
                    let param_names: Vec<String> = comp.params.iter().map(|p| p.name.clone()).collect();
                    self.component_params.insert(comp.name.clone(), param_names);
                }
            }
        }
    }

    /// Pre-collect store state fields from all files for cross-file state access
    pub fn collect_store_state_fields(&mut self, program: &Program, file_path: Option<&str>) {
        for decl in &program.declarations {
            if let Declaration::Store(store) = decl {
                let store_name = self.resolve_store_name(store, file_path);
                if let Some(state_block) = &store.state {
                    let fields: HashSet<String> = state_block.fields.iter()
                        .map(|p| p.key.clone())
                        .collect();
                    self.store_state_fields.insert(store_name, fields);
                }
            }
        }
    }

    /// Resolve store name: use explicit name or derive from filename
    fn resolve_store_name(&self, store: &StoreDef, file_path: Option<&str>) -> String {
        store.name.clone().unwrap_or_else(|| {
            file_path
                .map(|p| Self::filename_to_pascal_case(p))
                .unwrap_or_else(|| "AnonymousStore".to_string())
        })
    }

    /// Resolve page component name: "Page" gets renamed based on file path
    /// e.g., pages/docs/forms/index.tp -> FormsPage
    fn resolve_page_component_name(&self, name: &str) -> String {
        if name != "Page" {
            return name.to_string();
        }

        // If no file path, just return "Page"
        let file_path = match &self.current_file_path {
            Some(p) => p,
            None => return name.to_string(),
        };

        // Extract parent directory name from file path
        // e.g., "/path/to/pages/docs/forms/index.tp" -> "forms"
        let path = std::path::Path::new(file_path);
        let parent_name = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("Page");

        // If parent is "pages" (root index.tp), use "AppPage"
        if parent_name == "pages" {
            return "AppPage".to_string();
        }

        // Convert to PascalCase and append "Page"
        // e.g., "forms" -> "FormsPage", "quick-start" -> "QuickStartPage"
        let pascal_case = parent_name
            .split(|c: char| c == '-' || c == '_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<String>();

        format!("{}Page", pascal_case)
    }

    pub fn generate(&mut self, program: &Program) -> String {
        self.generate_with_file_path(program, None)
    }

    pub fn generate_with_file_path(&mut self, program: &Program, file_path: Option<&str>) -> String {
        // Reset file-specific state
        self.current_file_path = file_path.map(|s| s.to_string());
        self.current_file_store_name = None;
        self.current_file_store_actions.clear();
        self.current_file_store_fields.clear();

        // Check if this is a page file (pages/.../index.tp) that needs IIFE wrapping
        let is_page_file = file_path.map_or(false, |p| {
            p.contains("/pages/") && p.ends_with("/index.tp")
        });

        // Get the Page component's resolved name for this file
        let page_component_name = if is_page_file {
            Some(self.resolve_page_component_name("Page"))
        } else {
            None
        };

        // First pass: collect API service names, collect theme and component params
        // Also detect stores that have same-name components
        let mut store_names: HashSet<String> = HashSet::new();
        let mut component_names: HashSet<String> = HashSet::new();

        for decl in &program.declarations {
            if let Declaration::ApiService(api) = decl {
                self.api_service_names.insert(api.name.clone());
            }
            if let Declaration::Store(store) = decl {
                let store_name = self.resolve_store_name(store, file_path);

                // If anonymous store, track its actions/fields for unqualified reference resolution
                if store.name.is_none() {
                    self.current_file_store_name = Some(store_name.clone());
                    if let Some(actions_block) = &store.actions {
                        for action in &actions_block.actions {
                            self.current_file_store_actions.insert(action.name.clone());
                        }
                    }
                    if let Some(state_block) = &store.state {
                        for field in &state_block.fields {
                            self.current_file_store_fields.insert(field.key.clone());
                        }
                    }
                }

                store_names.insert(store_name.clone());
                // Collect state field names for store state access
                if let Some(state_block) = &store.state {
                    let fields: HashSet<String> = state_block.fields.iter()
                        .map(|p| p.key.clone())
                        .collect();
                    self.store_state_fields.insert(store_name, fields);
                }
            }
            if let Declaration::Component(comp) = decl {
                component_names.insert(comp.name.clone());
                // Collect component parameter names for object-style props conversion
                if !comp.params.is_empty() {
                    let param_names: Vec<String> = comp.params.iter().map(|p| p.name.clone()).collect();
                    self.component_params.insert(comp.name.clone(), param_names);
                }
            }
            // Collect theme colors from Theme definition
            if let Declaration::Theme(theme) = decl {
                self.collect_theme_from_def(theme);
            }
        }

        // Find stores that have same-name components
        for name in store_names.intersection(&component_names) {
            self.stores_with_components.insert(name.clone());
        }

        // Generate theme CSS injection if theme is defined
        if !self.theme_values.is_empty() {
            self.emit_theme_css();
            self.emit_line("");
        }

        // Start IIFE for page files to isolate local components
        if is_page_file {
            self.emit_line("// Page module (IIFE for scope isolation)");
            self.emit_line("(function() {");
            self.indent += 1;
        }

        for decl in &program.declarations {
            // Skip Theme - it's handled separately via CSS injection
            if matches!(decl, Declaration::Theme(_)) {
                continue;
            }
            self.generate_declaration(decl);
            self.emit_line("");
        }

        // Close IIFE and export Page component to global scope
        if is_page_file {
            if let Some(ref name) = page_component_name {
                self.emit_line(&format!("window.{} = {};", name, name));
            }
            self.indent -= 1;
            self.emit_line("})();");
        }

        // Note: mount is now handled by the build system to avoid duplicates
        // when compiling multiple files

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
        self.emit_line("    // Silent dispatch - update state without triggering re-render");
        self.emit_line("    dispatchSilent(action, ...args) {");
        self.emit_line("      const reducer = reducers.get(action);");
        self.emit_line("      if (reducer) {");
        self.emit_line("        Object.assign(state, reducer(state, ...args));");
        self.emit_line("      }");
        self.emit_line("      const effect = effects.get(action);");
        self.emit_line("      if (effect) effect(...args);");
        self.emit_line("      return state;");
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
        self.emit_line("// HTTP client with interceptor support");
        self.emit_line("const http = {");
        self.emit_line("  interceptors: {");
        self.emit_line("    request: [],");
        self.emit_line("    response: [],");
        self.emit_line("    error: []");
        self.emit_line("  },");
        self.emit_line("  defaults: { baseURL: '', headers: {} },");
        self.emit_line("  async _request(url, options = {}) {");
        self.emit_line("    let config = { url: this.defaults.baseURL + url, ...options, headers: { ...this.defaults.headers, ...options.headers } };");
        self.emit_line("    for (const fn of this.interceptors.request) { config = await fn(config) || config; }");
        self.emit_line("    try {");
        self.emit_line("      let res = await fetch(config.url, config);");
        self.emit_line("      if (!res.ok) throw { response: res, status: res.status, message: `HTTP ${res.status}` };");
        self.emit_line("      let data = await res.json();");
        self.emit_line("      for (const fn of this.interceptors.response) { data = await fn(data, res) || data; }");
        self.emit_line("      return data;");
        self.emit_line("    } catch (err) {");
        self.emit_line("      for (const fn of this.interceptors.error) { err = await fn(err) || err; }");
        self.emit_line("      throw err;");
        self.emit_line("    }");
        self.emit_line("  },");
        self.emit_line("  get(url) { return this._request(url); },");
        self.emit_line("  post(url, data) { return this._request(url, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(data) }); },");
        self.emit_line("  put(url, data) { return this._request(url, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(data) }); },");
        self.emit_line("  patch(url, data) { return this._request(url, { method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(data) }); },");
        self.emit_line("  del(url) { return this._request(url, { method: 'DELETE' }); }");
        self.emit_line("};");
        self.emit_line("");
        self.emit_line("// Silent dispatch for form field handlers - no re-render, just update related elements");
        self.emit_line("function dispatchField(storeName, action, value, fieldName) {");
        self.emit_line("  const store = stores.get(storeName);");
        self.emit_line("  if (store) {");
        self.emit_line("    const newState = store.dispatchSilent(action, value);");
        self.emit_line("    // Update error element directly");
        self.emit_line("    const errorEl = document.querySelector(`[data-error=\"${storeName}.${fieldName}Error\"]`);");
        self.emit_line("    if (errorEl) {");
        self.emit_line("      const errorText = newState[fieldName + 'Error'] || '';");
        self.emit_line("      errorEl.textContent = errorText;");
        self.emit_line("      errorEl.style.display = errorText ? '' : 'none';");
        self.emit_line("    }");
        self.emit_line("    // Update any bound text elements");
        self.emit_line("    document.querySelectorAll(`[data-bind=\"${storeName}.${fieldName}\"]`).forEach(el => {");
        self.emit_line("      el.textContent = newState[fieldName] || '';");
        self.emit_line("    });");
        self.emit_line("  }");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function mount(componentFn, container) {");
        self.emit_line("  const el = document.querySelector(container);");
        self.emit_line("  if (!el) return;");
        self.emit_line("  let lastPage = null;");
        self.emit_line("  const render = () => {");
        self.emit_line("    // Save focus state before re-render");
        self.emit_line("    const activeEl = document.activeElement;");
        self.emit_line("    const inputs = el.querySelectorAll('input, textarea, select');");
        self.emit_line("    let focusIndex = -1, selStart = 0, selEnd = 0;");
        self.emit_line("    inputs.forEach((inp, i) => {");
        self.emit_line("      if (inp === activeEl) {");
        self.emit_line("        focusIndex = i;");
        self.emit_line("        // Only save selection for text inputs and textareas");
        self.emit_line("        if (inp.selectionStart !== undefined) {");
        self.emit_line("          selStart = inp.selectionStart || 0;");
        self.emit_line("          selEnd = inp.selectionEnd || 0;");
        self.emit_line("        }");
        self.emit_line("      }");
        self.emit_line("    });");
        self.emit_line("    // Use routed page if available, otherwise use provided component");
        self.emit_line("    const page = currentPage || componentFn;");
        self.emit_line("    const pageChanged = page !== lastPage;");
        self.emit_line("    lastPage = page;");
        self.emit_line("    const vdom = typeof page === 'function' ? page() : page;");
        self.emit_line("    el.innerHTML = renderVdom(vdom);");
        self.emit_line("    bindEvents(el, vdom);");
        self.emit_line("    // Call lifecycle init on page change");
        self.emit_line("    if (pageChanged && vdom && vdom.lifecycle && vdom.lifecycle.init) {");
        self.emit_line("      vdom.lifecycle.init();");
        self.emit_line("    }");
        self.emit_line("    // Restore focus after re-render");
        self.emit_line("    if (focusIndex >= 0) {");
        self.emit_line("      const newInputs = el.querySelectorAll('input, textarea, select');");
        self.emit_line("      if (newInputs[focusIndex]) {");
        self.emit_line("        newInputs[focusIndex].focus();");
        self.emit_line("        // Only restore selection for text inputs and textareas");
        self.emit_line("        if (newInputs[focusIndex].setSelectionRange) {");
        self.emit_line("          try { newInputs[focusIndex].setSelectionRange(selStart, selEnd); } catch(e) {}");
        self.emit_line("        }");
        self.emit_line("      }");
        self.emit_line("    }");
        self.emit_line("  };");
        self.emit_line("  stores.forEach(store => store.subscribe && store.subscribe(render));");
        self.emit_line("  // Re-render on route change");
        self.emit_line("  window.addEventListener('popstate', () => { updateRoute(); render(); });");
        self.emit_line("  // Make render accessible for navigation");
        self.emit_line("  __rerender = render;");
        self.emit_line("  // Initial route setup and render");
        self.emit_line("  updateRoute();");
        self.emit_line("  render();");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("// Resolve { t: 'key' } translation objects");
        self.emit_line("function resolveText(val) {");
        self.emit_line("  if (val && typeof val === 'object' && val.t) {");
        self.emit_line("    return typeof t === 'function' ? t(val.t) : val.t;");
        self.emit_line("  }");
        self.emit_line("  return val;");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("// Escape HTML special characters");
        self.emit_line("function escapeHtml(str) {");
        self.emit_line("  if (str == null) return '';");
        self.emit_line("  return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/\"/g, '&quot;');");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function renderVdom(vdom) {");
        self.emit_line("  if (!vdom) return '';");
        self.emit_line("  const { type, content, value, style, children, align, inputType, placeholder, dataError, dataBind, dataField, options, rows, id } = vdom;");
        self.emit_line("  const resolvedContent = resolveText(content);");
        self.emit_line("  const resolvedPlaceholder = resolveText(placeholder);");
        self.emit_line("  const styleAttr = style ? ` class=\"${style}\"` : '';");
        self.emit_line("  const idAttr = id ? ` id=\"${id}\"` : '';");
        self.emit_line("  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';");
        self.emit_line("  const dataErrorAttr = dataError ? ` data-error=\"${dataError}\"` : '';");
        self.emit_line("  const dataBindAttr = dataBind ? ` data-bind=\"${dataBind}\"` : '';");
        self.emit_line("  const dataFieldAttr = dataField ? ` data-field=\"${dataField}\"` : '';");
        self.emit_line("  ");
        self.emit_line("  if (type === 'text') {");
        self.emit_line("    return `<span${styleAttr}${dataErrorAttr}${dataBindAttr}>${escapeHtml(resolvedContent != null ? resolvedContent : (value != null ? value : ''))}</span>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'button') {");
        self.emit_line("    return `<button${styleAttr} data-click=\"true\">${escapeHtml(resolvedContent || '')}</button>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'submit') {");
        self.emit_line("    return `<button type=\"submit\"${styleAttr} data-click=\"true\">${escapeHtml(resolvedContent || '')}</button>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'link') {");
        self.emit_line("    const rawHref = vdom.href || '#';");
        self.emit_line("    // Sanitize href - block dangerous URL schemes");
        self.emit_line("    const href = /^(javascript|data|vbscript):/i.test(rawHref) ? '#' : escapeHtml(rawHref);");
        self.emit_line("    // Support both content and children for links");
        self.emit_line("    const innerContent = resolvedContent || (children ? renderChildren(children) : '');");
        self.emit_line("    return `<a href=\"${href}\"${styleAttr} data-link=\"true\">${innerContent}</a>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'input') {");
        self.emit_line("    const inputTypeAttr = inputType || 'text';");
        self.emit_line("    const placeholderAttr = resolvedPlaceholder ? ` placeholder=\"${escapeHtml(resolvedPlaceholder)}\"` : '';");
        self.emit_line("    const valueAttr = value !== undefined ? ` value=\"${escapeHtml(value)}\"` : '';");
        self.emit_line("    return `<input type=\"${inputTypeAttr}\"${styleAttr}${placeholderAttr}${valueAttr}${dataFieldAttr} data-input=\"true\" />`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'select') {");
        self.emit_line("    const placeholderOpt = resolvedPlaceholder ? `<option value=\"\" disabled selected>${escapeHtml(resolvedPlaceholder)}</option>` : '';");
        self.emit_line("    const opts = (options || []).map(o => {");
        self.emit_line("      const optVal = typeof o === 'object' ? o.value : o;");
        self.emit_line("      const optLabel = typeof o === 'object' ? resolveText(o.label) : o;");
        self.emit_line("      const selected = optVal === value ? ' selected' : '';");
        self.emit_line("      return `<option value=\"${escapeHtml(optVal)}\"${selected}>${escapeHtml(optLabel)}</option>`;");
        self.emit_line("    }).join('');");
        self.emit_line("    return `<select${styleAttr}${dataFieldAttr} data-input=\"true\">${placeholderOpt}${opts}</select>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'textarea') {");
        self.emit_line("    const placeholderAttr = resolvedPlaceholder ? ` placeholder=\"${escapeHtml(resolvedPlaceholder)}\"` : '';");
        self.emit_line("    const rowsAttr = rows ? ` rows=\"${rows}\"` : '';");
        self.emit_line("    return `<textarea${styleAttr}${placeholderAttr}${rowsAttr}${dataFieldAttr} data-input=\"true\">${escapeHtml(value || '')}</textarea>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'formfield') {");
        self.emit_line("    const { label, inputType, placeholder, value, errorMsg, dataError, dataField } = vdom;");
        self.emit_line("    const labelText = resolveText(label);");
        self.emit_line("    const placeholderText = resolveText(placeholder);");
        self.emit_line("    const inputTypeAttr = inputType || 'text';");
        self.emit_line("    const placeholderAttr = placeholderText ? ` placeholder=\"${escapeHtml(placeholderText)}\"` : '';");
        self.emit_line("    const valueAttr = value !== undefined ? ` value=\"${escapeHtml(value)}\"` : '';");
        self.emit_line("    const dataFieldAttr = dataField ? ` data-field=\"${dataField}\"` : '';");
        self.emit_line("    const dataErrorAttr = dataError ? ` data-error=\"${dataError}\"` : '';");
        self.emit_line("    return `<div class=\"mb-4 flex flex-col\"><label class=\"block text-sm font-medium text-gray-700 mb-2\">${escapeHtml(labelText || '')}</label><input type=\"${inputTypeAttr}\" class=\"w-full px-4 py-3 border border-gray-300 rounded-lg focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500 transition\"${placeholderAttr}${valueAttr}${dataFieldAttr} data-input=\"true\" /><span class=\"text-red-500 text-sm mt-1\"${dataErrorAttr}>${escapeHtml(errorMsg || '')}</span></div>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'form') {");
        self.emit_line("    const inner = (children || []).map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');");
        self.emit_line("    return `<form${styleAttr} class=\"${(style || '') + flexClass}\" data-form=\"true\">${inner}</form>`;");
        self.emit_line("  }");
        self.emit_line("  if (children) {");
        self.emit_line("    const childArr = Array.isArray(children) ? children : [children];");
        self.emit_line("    const inner = childArr.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');");
        self.emit_line("    return `<div${idAttr} class=\"${(style || '') + flexClass}\">${inner}</div>`;");
        self.emit_line("  }");
        self.emit_line("  return `<div${idAttr}${styleAttr}>${escapeHtml(resolvedContent != null ? resolvedContent : (value != null ? value : ''))}</div>`;");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function bindEvents(el, vdom) {");
        self.emit_line("  el.querySelectorAll('[data-click]').forEach((btn, i) => {");
        self.emit_line("    const handler = findClickHandler(vdom, i);");
        self.emit_line("    if (handler) btn.onclick = handler;");
        self.emit_line("  });");
        self.emit_line("  el.querySelectorAll('[data-input]').forEach((input, i) => {");
        self.emit_line("    const handler = findInputHandler(vdom, i);");
        self.emit_line("    if (handler) {");
        self.emit_line("      // Use 'input' event for text inputs and textareas, 'change' for selects");
        self.emit_line("      if (input.tagName === 'SELECT') {");
        self.emit_line("        input.onchange = (e) => handler(e.target.value);");
        self.emit_line("      } else {");
        self.emit_line("        input.oninput = (e) => handler(e.target.value);");
        self.emit_line("      }");
        self.emit_line("    }");
        self.emit_line("  });");
        self.emit_line("  el.querySelectorAll('[data-link]').forEach((link) => {");
        self.emit_line("    link.onclick = (e) => {");
        self.emit_line("      const href = link.getAttribute('href');");
        self.emit_line("      // Block dangerous URL schemes");
        self.emit_line("      if (/^(javascript|data|vbscript):/i.test(href)) { e.preventDefault(); return; }");
        self.emit_line("      if (href.startsWith('#')) {");
        self.emit_line("        e.preventDefault();");
        self.emit_line("        const target = document.querySelector(href) || document.getElementById(href.slice(1));");
        self.emit_line("        if (target) target.scrollIntoView({ behavior: 'smooth' });");
        self.emit_line("      } else if (href.startsWith('http://') || href.startsWith('https://')) {");
        self.emit_line("        window.open(href, '_blank');");
        self.emit_line("        e.preventDefault();");
        self.emit_line("      } else {");
        self.emit_line("        e.preventDefault();");
        self.emit_line("        navigate(href);");
        self.emit_line("      }");
        self.emit_line("    };");
        self.emit_line("  });");
        self.emit_line("  el.querySelectorAll('[data-form]').forEach((form, i) => {");
        self.emit_line("    const handler = findSubmitHandler(vdom, i);");
        self.emit_line("    if (handler) {");
        self.emit_line("      form.onsubmit = (e) => {");
        self.emit_line("        e.preventDefault();");
        self.emit_line("        handler();");
        self.emit_line("      };");
        self.emit_line("    }");
        self.emit_line("  });");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function normalizeChildren(children) {");
        self.emit_line("  if (!children) return [];");
        self.emit_line("  return Array.isArray(children) ? children : [children];");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function findClickHandler(vdom, index, count = { n: 0 }) {");
        self.emit_line("  if (!vdom) return null;");
        self.emit_line("  if (vdom.click && count.n++ === index) return vdom.click;");
        self.emit_line("  for (const c of normalizeChildren(vdom.children)) {");
        self.emit_line("    const child = typeof c === 'function' ? c() : c;");
        self.emit_line("    const h = findClickHandler(child, index, count);");
        self.emit_line("    if (h) return h;");
        self.emit_line("  }");
        self.emit_line("  return null;");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function findInputHandler(vdom, index, count = { n: 0 }) {");
        self.emit_line("  if (!vdom) return null;");
        self.emit_line("  if ((vdom.input || vdom.onInput) && count.n++ === index) return vdom.input || vdom.onInput;");
        self.emit_line("  for (const c of normalizeChildren(vdom.children)) {");
        self.emit_line("    const child = typeof c === 'function' ? c() : c;");
        self.emit_line("    const h = findInputHandler(child, index, count);");
        self.emit_line("    if (h) return h;");
        self.emit_line("  }");
        self.emit_line("  return null;");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function findSubmitHandler(vdom, index, count = { n: 0 }) {");
        self.emit_line("  if (!vdom) return null;");
        self.emit_line("  if (vdom.type === 'form' && vdom.onSubmit && count.n++ === index) return vdom.onSubmit;");
        self.emit_line("  for (const c of normalizeChildren(vdom.children)) {");
        self.emit_line("    const child = typeof c === 'function' ? c() : c;");
        self.emit_line("    const h = findSubmitHandler(child, index, count);");
        self.emit_line("    if (h) return h;");
        self.emit_line("  }");
        self.emit_line("  return null;");
        self.emit_line("}");
        self.emit_line("");

        // Router runtime
        self.emit_router_runtime();
    }

    fn emit_router_runtime(&mut self) {
        self.emit_line("// Router");
        self.emit_line("const routeState = { path: '/', params: {}, query: {} };");
        self.emit_line("const routes = [];");
        self.emit_line("let currentPage = null;");
        self.emit_line("let __rerender = () => {};");
        self.emit_line("");

        // Route registration
        self.emit_line("function registerRoute(pattern, component) {");
        self.emit_line("  const paramNames = [];");
        self.emit_line("  const regexPattern = pattern.replace(/\\[([^\\]]+)\\]/g, (_, name) => {");
        self.emit_line("    if (name.startsWith('...')) {");
        self.emit_line("      paramNames.push(name.slice(3));");
        self.emit_line("      return '(.*)';");
        self.emit_line("    }");
        self.emit_line("    paramNames.push(name);");
        self.emit_line("    return '([^/]+)';");
        self.emit_line("  });");
        self.emit_line("  routes.push({ pattern: new RegExp(`^${regexPattern}$`), paramNames, component });");
        self.emit_line("}");
        self.emit_line("");

        // Route matching
        self.emit_line("function matchRoute(path) {");
        self.emit_line("  for (const route of routes) {");
        self.emit_line("    const match = path.match(route.pattern);");
        self.emit_line("    if (match) {");
        self.emit_line("      const params = {};");
        self.emit_line("      route.paramNames.forEach((name, i) => { params[name] = match[i + 1]; });");
        self.emit_line("      return { component: route.component, params };");
        self.emit_line("    }");
        self.emit_line("  }");
        self.emit_line("  return null;");
        self.emit_line("}");
        self.emit_line("");

        // Parse query string
        self.emit_line("function parseQuery(search) {");
        self.emit_line("  const query = {};");
        self.emit_line("  if (search) {");
        self.emit_line("    new URLSearchParams(search).forEach((v, k) => { query[k] = v; });");
        self.emit_line("  }");
        self.emit_line("  return query;");
        self.emit_line("}");
        self.emit_line("");

        // Guard checking function
        self.emit_line("function checkGuards(path) {");
        self.emit_line("  if (typeof __guardSetup === 'undefined') return true;");
        self.emit_line("");
        self.emit_line("  // Check route-specific guards first (to allow 'none' to skip global guards)");
        self.emit_line("  for (const [pattern, guard] of Object.entries(__guardSetup.routes || {})) {");
        self.emit_line("    const regex = new RegExp('^' + pattern.replace(/\\*/g, '.*') + '$');");
        self.emit_line("    if (regex.test(path)) {");
        self.emit_line("      if (guard === null) return true; // 'none' - skip all guards");
        self.emit_line("      if (typeof guard === 'function' && !guard()) return false;");
        self.emit_line("      return true; // Route-specific guard passed, skip global guards");
        self.emit_line("    }");
        self.emit_line("  }");
        self.emit_line("");
        self.emit_line("  // Check global guards");
        self.emit_line("  for (const guard of __guardSetup.global || []) {");
        self.emit_line("    if (typeof guard === 'function' && !guard()) return false;");
        self.emit_line("  }");
        self.emit_line("");
        self.emit_line("  return true;");
        self.emit_line("}");
        self.emit_line("");

        // Navigate function
        self.emit_line("function navigate(path) {");
        self.emit_line("  if (!checkGuards(path)) return;");
        self.emit_line("  history.pushState(null, '', path);");
        self.emit_line("  updateRoute();");
        self.emit_line("  __rerender();");
        self.emit_line("}");
        self.emit_line("");

        // Update route
        self.emit_line("function updateRoute() {");
        self.emit_line("  const path = location.pathname;");
        self.emit_line("  const matched = matchRoute(path);");
        self.emit_line("  routeState.path = path;");
        self.emit_line("  routeState.query = parseQuery(location.search);");
        self.emit_line("  if (matched) {");
        self.emit_line("    routeState.params = matched.params;");
        self.emit_line("    currentPage = matched.component;");
        self.emit_line("  } else {");
        self.emit_line("    routeState.params = {};");
        self.emit_line("    currentPage = null;");
        self.emit_line("  }");
        self.emit_line("  stores.forEach(store => store.dispatch && store.dispatch('__routeChange'));");
        self.emit_line("}");
        self.emit_line("");

        // Router store
        self.emit_line("const Router = {");
        self.emit_line("  get state() { return routeState; },");
        self.emit_line("  Navigate: navigate,");
        self.emit_line("  subscribe(fn) { /* handled by stores */ }");
        self.emit_line("};");
        self.emit_line("stores.set('Router', Router);");
        self.emit_line("");

        // $route accessor
        self.emit_line("const $route = routeState;");
        self.emit_line("");

        // Initialize router on load
        self.emit_line("window.addEventListener('popstate', updateRoute);");
        self.emit_line("document.addEventListener('DOMContentLoaded', updateRoute);");
    }

    fn emit_runtime_validators(&mut self) {
        self.emit_line("// Validators");
        self.emit_line("const validators = {");
        self.indent += 1;

        // required
        self.emit_line("required(value, _args, field) {");
        self.emit_line("  if (value === null || value === undefined || value === '') {");
        self.emit_line("    const msg = typeof t === 'function' ? t('validation_required', { field }) : `${field} is required`;");
        self.emit_line("    return { valid: false, error: msg };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // min - minimum value for numbers, minimum length for strings
        self.emit_line("min(value, args, field) {");
        self.emit_line("  const min = args[0];");
        self.emit_line("  if (typeof value === 'string' && value.length < min) {");
        self.emit_line("    const msg = typeof t === 'function' ? t('validation_min_length', { field, min }) : `${field} must be at least ${min} characters`;");
        self.emit_line("    return { valid: false, error: msg };");
        self.emit_line("  }");
        self.emit_line("  if (typeof value === 'number' && value < min) {");
        self.emit_line("    const msg = typeof t === 'function' ? t('validation_min_value', { field, min }) : `${field} must be at least ${min}`;");
        self.emit_line("    return { valid: false, error: msg };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // max - maximum value for numbers, maximum length for strings
        self.emit_line("max(value, args, field) {");
        self.emit_line("  const max = args[0];");
        self.emit_line("  if (typeof value === 'string' && value.length > max) {");
        self.emit_line("    const msg = typeof t === 'function' ? t('validation_max_length', { field, max }) : `${field} must be at most ${max} characters`;");
        self.emit_line("    return { valid: false, error: msg };");
        self.emit_line("  }");
        self.emit_line("  if (typeof value === 'number' && value > max) {");
        self.emit_line("    const msg = typeof t === 'function' ? t('validation_max_value', { field, max }) : `${field} must be at most ${max}`;");
        self.emit_line("    return { valid: false, error: msg };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // minLength - minimum length for strings
        self.emit_line("minLength(value, args, field) {");
        self.emit_line("  const min = args[0];");
        self.emit_line("  if (typeof value === 'string' && value.length < min) {");
        self.emit_line("    const msg = typeof t === 'function' ? t('validation_min_length', { field, min }) : `${field} must be at least ${min} characters`;");
        self.emit_line("    return { valid: false, error: msg };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // maxLength - maximum length for strings
        self.emit_line("maxLength(value, args, field) {");
        self.emit_line("  const max = args[0];");
        self.emit_line("  if (typeof value === 'string' && value.length > max) {");
        self.emit_line("    const msg = typeof t === 'function' ? t('validation_max_length', { field, max }) : `${field} must be at most ${max} characters`;");
        self.emit_line("    return { valid: false, error: msg };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // email
        self.emit_line("email(value, _args, field) {");
        self.emit_line("  const emailRegex = /^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/;");
        self.emit_line("  if (typeof value === 'string' && !emailRegex.test(value)) {");
        self.emit_line("    const msg = typeof t === 'function' ? t('validation_email', { field }) : `${field} must be a valid email address`;");
        self.emit_line("    return { valid: false, error: msg };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // pattern - regex pattern
        self.emit_line("pattern(value, args, field) {");
        self.emit_line("  const pattern = new RegExp(args[0]);");
        self.emit_line("  if (typeof value === 'string' && !pattern.test(value)) {");
        self.emit_line("    const msg = typeof t === 'function' ? t('validation_pattern', { field }) : `${field} does not match the required pattern`;");
        self.emit_line("    return { valid: false, error: msg };");
        self.emit_line("  }");
        self.emit_line("  return { valid: true };");
        self.emit_line("},");

        // regex - alias for pattern with custom error message support
        self.emit_line("regex(value, args, field) {");
        self.emit_line("  const pattern = new RegExp(args[0]);");
        self.emit_line("  const customMsg = args[1];");
        self.emit_line("  if (typeof value === 'string' && !pattern.test(value)) {");
        self.emit_line("    return { valid: false, error: customMsg || `${field} does not match the required format` };");
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
            Declaration::Test(_) => {}  // Handled separately for Playwright test generation
            Declaration::BeforeEach(_) => {} // Handled separately for Playwright test generation
            Declaration::AfterEach(_) => {}  // Handled separately for Playwright test generation
            Declaration::BeforeOnce(_) => {} // Handled separately for Playwright test generation
            Declaration::AfterOnce(_) => {}  // Handled separately for Playwright test generation
            Declaration::Guard(guard) => self.generate_guard(guard),
            Declaration::GuardSetup(setup) => self.generate_guard_setup(setup),
        }
    }

    fn generate_guard(&mut self, guard: &GuardDef) {
        // Generate guard function
        let check_expr = self.generate_expression(&guard.check);
        let guard_name = guard.name.clone();
        let redirect = guard.redirect.clone();

        self.emit_line(&format!("function {}Guard() {{", guard_name));
        self.emit_line(&format!("  const allowed = {};", check_expr));
        self.emit_line("  if (!allowed) {");
        self.emit_line(&format!("    window.location.hash = '{}';", redirect));
        self.emit_line("    return false;");
        self.emit_line("  }");
        self.emit_line("  return true;");
        self.emit_line("}");
        self.emit_line("");
    }

    fn generate_guard_setup(&mut self, setup: &GuardSetupDef) {
        // Generate guard setup configuration
        self.emit_line("const __guardSetup = {");

        // Global guards
        if !setup.global.is_empty() {
            let guards = setup.global.iter()
                .map(|g| format!("{}Guard", g))
                .collect::<Vec<_>>()
                .join(", ");
            self.emit_line(&format!("  global: [{}],", guards));
        } else {
            self.emit_line("  global: [],");
        }

        // Route-specific guards
        self.emit_line("  routes: {");
        for route in &setup.routes {
            match &route.guard {
                Some(guard_name) => {
                    self.emit_line(&format!("    '{}': {}Guard,", route.pattern, guard_name));
                }
                None => {
                    self.emit_line(&format!("    '{}': null,", route.pattern));
                }
            }
        }
        self.emit_line("  }");
        self.emit_line("};");
        self.emit_line("");
    }

    fn generate_component(&mut self, comp: &ComponentDef) {
        // Store component params for expression generation
        let old_params = std::mem::take(&mut self.local_params);
        for param in &comp.params {
            self.local_params.insert(param.name.clone());
        }

        // Resolve component name - "Page" gets renamed based on file path to avoid conflicts
        let component_name = self.resolve_page_component_name(&comp.name);

        let params_str = if comp.params.is_empty() {
            String::new()
        } else {
            comp.params.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(", ")
        };

        // Check if this is an alias component: Alias(args) -> Base(args, defaultValue)
        if let Some(alias) = &comp.alias {
            // Get base component's parameter names to detect event handler params
            let base_params = self.component_params.get(&alias.base).cloned();

            let args_str = alias.args.iter().enumerate()
                .map(|(i, arg)| {
                    // Check if this arg corresponds to an event handler param
                    if let Some(ref params) = base_params {
                        if let Some(param_name) = params.get(i) {
                            if matches!(param_name.as_str(), "onClick" | "click" | "onInput" | "input" | "onChange" | "change" | "onSubmit" | "submit") {
                                self.current_property_key = Some(param_name.clone());
                                let val = self.generate_expression(arg);
                                self.current_property_key = None;
                                return val;
                            }
                        }
                    }
                    self.generate_expression(arg)
                })
                .collect::<Vec<_>>()
                .join(", ");
            self.emit_line(&format!("function {}({}) {{", component_name, params_str));
            self.indent += 1;
            self.emit_line(&format!("return {}({});", alias.base, args_str));
            self.indent -= 1;
            self.emit_line("}");
            self.local_params = old_params;
            return;
        }

        self.emit_line(&format!("function {}({}) {{", component_name, params_str));
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

            // Handle responsive style objects: style: { common: '', desktop: '', tablet: '', mobile: '' }
            let value = if prop.key == "style" {
                self.generate_responsive_style(&prop.value)
            } else {
                self.generate_expression(&prop.value)
            };

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
        // Resolve store name (use explicit name or derive from filename)
        let store_name = self.resolve_store_name(store, self.current_file_path.as_deref());

        // Collect fields with validation annotations for auto-validation
        let validated_fields: Vec<String> = store.state.as_ref()
            .map(|s| s.fields.iter()
                .filter(|f| !f.annotations.is_empty())
                .map(|f| f.key.clone())
                .collect())
            .unwrap_or_default();

        // Get variable name (different from registry name if same-name component exists)
        let var_name = self.store_var_name(&store_name);

        // Generate store creation
        self.emit_line(&format!("const {} = createStore('{}', {{", var_name, store_name));
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
                let comma = if i < validated_fields.len() - 1 { "," } else { "" };
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
                    let comma = if i < validation_rules.len() - 1 { "," } else { "" };
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
                self.emit_line(&format!("for (const [field, fieldRules] of Object.entries({}ValidationRules)) {{", var_name));
                self.indent += 1;
                self.emit_line("const value = data[field];");
                self.emit_line(&format!("const label = typeof t === 'function' ? t({}FieldKeys[field]) : ({}FieldKeys[field] || field);", var_name, var_name));
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

            // Auto-generate Set actions with validation for annotated fields
            for field in &state.fields {
                if !field.annotations.is_empty() && !field.annotations.iter().all(|a| a.name == "key" || a.name == "hidden") {
                    let field_name = &field.key;
                    let capitalized = capitalize_first(field_name);

                    // Generate auto-validating setter
                    self.emit_line(&format!("{}.on('Set{}', (state, value) => {{", var_name, capitalized));
                    self.indent += 1;
                    self.emit_line(&format!("const result = validate{}({{ ...state, {}: value }});", var_name, field_name));
                    self.emit_line(&format!("const error = result.errors.{} ? result.errors.{}[0] : '';", field_name, field_name));
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
            let validated_fields: Vec<&str> = state.fields.iter()
                .filter(|f| !f.annotations.is_empty() && !f.annotations.iter().all(|a| a.name == "key" || a.name == "hidden"))
                .map(|f| f.key.as_str())
                .collect();

            if !validated_fields.is_empty() {
                self.emit_line(&format!("{}.on('Submit', (state) => {{", var_name));
                self.indent += 1;
                self.emit_line(&format!("const result = validate{}(state);", var_name));
                self.emit_line("return {");
                self.indent += 1;
                self.emit_line("...state,");
                for (i, field) in validated_fields.iter().enumerate() {
                    let comma = if i < validated_fields.len() - 1 { "," } else { "" };
                    self.emit_line(&format!("{}Error: result.errors.{} ? result.errors.{}[0] : ''{}", field, field, field, comma));
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
                self.emit_line(&format!("Object.defineProperty({}, 'Fields', {{", var_name));
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

    fn generate_reducer(&mut self, store: &StoreDef, store_name: &str, handler: &ReducerHandler) {
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

    fn generate_effect(&mut self, store: &StoreDef, store_name: &str, handler: &EffectHandler) {
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

    fn generate_selector(&mut self, store: &StoreDef, store_name: &str, selector: &SelectorDef) {
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

    fn generate_statement_with_tracking(&mut self, store: &StoreDef, store_name: &str, stmt: &Statement) {
        self.generate_statement_impl(store, store_name, stmt, true)
    }

    fn generate_statement_impl(&mut self, store: &StoreDef, store_name: &str, stmt: &Statement, track_locals: bool) {
        match stmt {
            Statement::Assignment { name, value } => {
                let val = self.generate_expression(value);
                self.emit_line(&format!("const {} = {};", name, val));
                // Track this variable so subsequent statements can reference it
                if track_locals {
                    self.local_params.insert(name.clone());
                }
            }
            Statement::Dispatch { action, args } => {
                let args_str = args
                    .iter()
                    .map(|a| self.generate_expression(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.emit_line(&format!(
                    "dispatch('{}', '{}'{}{});",
                    store_name,
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
            Expression::Object { properties } => {
                let props: Vec<String> = properties
                    .iter()
                    .map(|p| {
                        self.current_property_key = Some(p.key.clone());
                        let value = self.generate_expression(&p.value);
                        self.current_property_key = None;
                        format!("{}: {}", p.key, value)
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
                if self.is_event_handler() {
                    if let Expression::Identifier { name: store } = object.as_ref() {
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
            Expression::Call { callee, args } => {
                let c = self.generate_expression(callee);

                // Check for object-style props: ComponentName({ prop1: val1, prop2: val2 })
                // Convert to positional args: ComponentName(val1, val2)
                if args.len() == 1 {
                    if let Expression::Object { properties } = &args[0] {
                        if let Expression::Identifier { name } = callee.as_ref() {
                            // Clone param_names to avoid borrow conflict
                            let param_names_opt = self.component_params.get(name).cloned();
                            if let Some(param_names) = param_names_opt {
                                // If component uses single "props" param, pass object as-is
                                if param_names.len() == 1 && param_names[0] == "props" {
                                    let props: Vec<String> = properties
                                        .iter()
                                        .map(|p| format!("{}: {}", p.key, self.generate_expression(&p.value)))
                                        .collect();
                                    return format!("{}({{ {} }})", c, props.join(", "));
                                }
                                // Check for Reference props and collect their paths
                                let mut auto_data_error: Option<String> = None;
                                let mut auto_data_field: Option<String> = None;
                                for p in properties {
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
        }
    }

    fn emit_line(&mut self, line: &str) {
        let indent = "  ".repeat(self.indent);
        self.output.push_str(&indent);
        self.output.push_str(line);
        self.output.push('\n');
    }

    /// Collect validation rules from property annotations (excluding @label)
    fn collect_validation_rules(&mut self, fields: &[Property]) -> Vec<(String, Vec<String>)> {
        let mut result = Vec::new();

        for field in fields {
            let validation_annotations: Vec<&Annotation> = field.annotations.iter()
                .filter(|a| a.name != "label")
                .collect();

            if !validation_annotations.is_empty() {
                let rules: Vec<String> = validation_annotations.iter()
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

    /// Collect field keys from @key annotations (for i18n and form generation)
    fn collect_field_keys(&mut self, fields: &[Property]) -> Vec<(String, String)> {
        let mut result = Vec::new();

        for field in fields {
            // Look for @key annotation
            let key = field.annotations.iter()
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
    fn is_hidden_field(&self, field: &Property) -> bool {
        field.annotations.iter().any(|a| a.name == "hidden")
    }

    /// Collect form field info: (field_name, i18n_key, input_type)
    fn collect_form_fields(&mut self, fields: &[Property]) -> Vec<(String, String, String)> {
        let mut result = Vec::new();

        for field in fields {
            // Skip hidden fields
            if self.is_hidden_field(field) {
                continue;
            }

            // Only include fields with @key annotation
            let key = field.annotations.iter()
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
                } else if field.key.contains("password") || field.annotations.iter().any(|a| a.name == "password") {
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

/// Capitalize the first letter of a string
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
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
            component_params: HashMap::new(),
            stores_with_components: HashSet::new(),
            store_state_fields: HashMap::new(),
            current_file_path: None,
            current_file_store_name: None,
            current_file_store_actions: HashSet::new(),
            current_file_store_fields: HashSet::new(),
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
            Declaration::Test(_) => {}   // Tests don't need type exports
            Declaration::BeforeEach(_) => {} // Tests don't need type exports
            Declaration::AfterEach(_) => {}  // Tests don't need type exports
            Declaration::BeforeOnce(_) => {} // Tests don't need type exports
            Declaration::AfterOnce(_) => {}  // Tests don't need type exports
            Declaration::Guard(_) => {}      // Guards don't need type exports
            Declaration::GuardSetup(_) => {} // GuardSetup doesn't need type exports
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
        // For type generation, use explicit name or fallback to AnonymousStore
        let store_name = store.name.clone().unwrap_or_else(|| "AnonymousStore".to_string());

        // Generate State interface
        self.emit_line(&format!("export interface {}State {{", store_name));
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
            self.emit_line(&format!("export type {}Actions =", store_name));
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
        self.emit_line(&format!("export interface {}Store {{", store_name));
        self.indent += 1;
        self.emit_line(&format!("readonly state: {}State;", store_name));
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
        self.emit_line(&format!("export declare const {}: {}Store;", store_name, store_name));
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

    #[test]
    fn test_generate_object_style_props() {
        let source = r#"
            FormField(label, inputType, placeholder) -> {
                type: div
                content: label
            }

            App -> {
                children: [
                    FormField({ label: "Email" inputType: "email" placeholder: "Enter email" })
                ]
            }
        "#;

        let output = generate(source);
        // The object-style call should be converted to positional args
        assert!(output.contains("FormField('Email', 'email', 'Enter email')"));
    }
}
