//! Code generation for topo language
//!
//! Converts AST to JavaScript code.

mod api;
mod expression;
mod routes;
mod runtime;
mod server_js;
mod server_rust;
mod store;
mod ts_codegen;

pub use server_js::WorkersCodegen;
pub use server_rust::{AxumCodegen, generate_cargo_toml};
pub use ts_codegen::{TsCodegen, VNODE_TYPE_DEF};

use crate::ast::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Default)]
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
    /// Generated Routes names (for avoiding duplicate declarations)
    generated_routes_names: HashMap<String, usize>,
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
            generated_routes_names: HashMap::new(),
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

    /// Get unique name for Routes definition, avoiding duplicate declarations
    /// If the name was already used, append a suffix (_1, _2, etc.)
    fn get_unique_routes_name(&mut self, name: &str) -> String {
        let count = self.generated_routes_names.entry(name.to_string()).or_insert(0);
        *count += 1;
        if *count == 1 {
            name.to_string()
        } else {
            format!("{}_{}", name, count)
        }
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
            Expression::Object { members } => {
                // Check if this is a responsive style object
                let responsive_keys = ["common", "base", "mobile", "tablet", "desktop"];
                let has_responsive_keys = members.iter()
                    .any(|m| matches!(m, ObjectMember::Property(p) if responsive_keys.contains(&p.key.as_str())));

                if has_responsive_keys {
                    // Collect classes for each breakpoint
                    let mut classes: Vec<std::string::String> = Vec::new();

                    for member in members {
                        let prop = match member {
                            ObjectMember::Property(p) => p,
                            ObjectMember::Spread { .. } => continue, // Skip spreads in responsive style
                        };
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
                .map(Self::filename_to_pascal_case)
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
            .split(['-', '_'])
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
        let is_page_file = file_path.is_some_and(|p| {
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

        // For same-name stores (named store with same-name component),
        // also set up current_file_store context if not already set by anonymous store
        if self.current_file_store_name.is_none() && !self.stores_with_components.is_empty() {
            // Use the first same-name store as the file's "anonymous" store context
            for decl in &program.declarations {
                if let Declaration::Store(store) = decl {
                    if let Some(name) = &store.name {
                        if self.stores_with_components.contains(name) {
                            self.current_file_store_name = Some(name.clone());
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
                            break;
                        }
                    }
                }
            }
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
            Declaration::Resolver(resolver) => self.generate_resolver(resolver),
            Declaration::Directive(directive) => self.generate_directive(directive),
            Declaration::Routes(routes) => self.generate_routes(routes),
            Declaration::Function(func) => self.generate_function(func),
            Declaration::Schema(_) => {
                // Schema definitions are used for type inference, not code generation
            }
            Declaration::Repository(_) => {
                // Repository definitions are handled separately in server codegen
            }
            Declaration::Animation(anim) => self.generate_animation(anim),
        }
    }

    fn generate_animation(&mut self, anim: &AnimationDef) {
        let name = &anim.name;
        let duration = &anim.duration;

        self.emit_line(&format!("const {} = {{", name));
        self.indent += 1;

        self.emit_line(&format!("name: '{}',", name));
        self.emit_line(&format!("duration: '{}',", duration));

        if let Some(ref easing) = anim.easing {
            self.emit_line(&format!("easing: '{}',", easing));
        }

        if let Some(ref fill) = anim.fill {
            self.emit_line(&format!("fill: '{}',", fill));
        }

        // Generate keyframes
        self.emit_line("keyframes: [");
        self.indent += 1;

        match &anim.animation_type {
            AnimationType::FromTo { from, to } => {
                // Generate from keyframe
                self.emit_line("{");
                self.indent += 1;
                for prop in from {
                    let value = self.generate_expression(&prop.value);
                    self.emit_line(&format!("{}: {},", prop.property, value));
                }
                self.indent -= 1;
                self.emit_line("},");

                // Generate to keyframe
                self.emit_line("{");
                self.indent += 1;
                for prop in to {
                    let value = self.generate_expression(&prop.value);
                    self.emit_line(&format!("{}: {},", prop.property, value));
                }
                self.indent -= 1;
                self.emit_line("},");
            }
            AnimationType::Keyframes { keyframes } => {
                for kf in keyframes {
                    self.emit_line("{");
                    self.indent += 1;

                    // Add offset for keyframe percentage
                    let offset = kf.percent as f32 / 100.0;
                    self.emit_line(&format!("offset: {},", offset));

                    // Add easing if specified
                    if let Some(ref easing) = kf.easing {
                        self.emit_line(&format!("easing: '{}',", easing));
                    }

                    // Add properties
                    for prop in &kf.properties {
                        let value = self.generate_expression(&prop.value);
                        self.emit_line(&format!("{}: {},", prop.property, value));
                    }

                    self.indent -= 1;
                    self.emit_line("},");
                }
            }
        }

        self.indent -= 1;
        self.emit_line("],");

        self.indent -= 1;
        self.emit_line("};");
        self.emit_line(&format!("__animations.set('{}', {});", name, name));
    }

    /// Generate a pure function definition
    fn generate_function(&mut self, func: &FunctionDef) {
        let params: Vec<String> = func.params.iter().map(|p| p.name.clone()).collect();
        let params_str = params.join(", ");

        self.emit_line(&format!("function {}({}) {{", func.name, params_str));
        self.indent += 1;

        let body_js = self.generate_expression(&func.body);
        self.emit_line(&format!("return {};", body_js));

        self.indent -= 1;
        self.emit_line("}");
    }

    fn generate_guard(&mut self, guard: &GuardDef) {
        // Generate guard function
        let guard_name = guard.name.clone();

        // Determine guard type string for runtime
        let guard_type_str = match guard.guard_type {
            GuardType::Activate => "activate",
            GuardType::Deactivate => "deactivate",
        };

        // Handle guards with check/redirect
        if let Some(ref check) = guard.check {
            let check_expr = self.generate_expression(check);
            let redirect = guard.redirect.as_deref().unwrap_or("/");

            self.emit_line(&format!("const {}Guard = {{", guard_name));
            self.emit_line(&format!("  type: '{}',", guard_type_str));
            self.emit_line(&format!("  check: () => {},", check_expr));
            self.emit_line(&format!("  redirect: '{}',", redirect));
            self.emit_line("  execute() {");
            self.emit_line("    const allowed = this.check();");
            self.emit_line("    if (!allowed) {");
            self.emit_line("      window.location.hash = this.redirect;");
            self.emit_line("      return false;");
            self.emit_line("    }");
            self.emit_line("    return true;");
            self.emit_line("  }");
            self.emit_line("};");
            self.emit_line("");
        }
        // TODO: Handle new style guards with body statements
    }

    fn generate_resolver(&mut self, resolver: &ResolverDef) {
        let resolver_name = &resolver.name;
        let params_str = resolver.params.join(", ");
        let fetch_expr = self.generate_expression(&resolver.fetch);
        let fallback_expr = self.generate_expression(&resolver.fallback);

        self.emit_line(&format!("const {}Resolver = {{", resolver_name));
        self.indent += 1;

        // Parameters for route param mapping
        if !resolver.params.is_empty() {
            self.emit_line(&format!(
                "params: [{}],",
                resolver
                    .params
                    .iter()
                    .map(|p| format!("'{}'", p))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Cache configuration
        if let Some(cache_ms) = resolver.cache {
            self.emit_line(&format!("cacheMs: {},", cache_ms));
            self.emit_line("_cache: null,");
            self.emit_line("_cacheTime: 0,");
        }

        // Resolve function
        self.emit_line(&format!("async resolve({}) {{", params_str));
        self.indent += 1;

        // Cache check
        if resolver.cache.is_some() {
            self.emit_line("const now = Date.now();");
            self.emit_line("if (this._cache && (now - this._cacheTime) < this.cacheMs) {");
            self.emit_line("  return this._cache;");
            self.emit_line("}");
        }

        self.emit_line("try {");
        self.indent += 1;
        self.emit_line(&format!("const data = await {};", fetch_expr));

        // Cache storage
        if resolver.cache.is_some() {
            self.emit_line("this._cache = data;");
            self.emit_line("this._cacheTime = Date.now();");
        }

        self.emit_line("return data;");
        self.indent -= 1;
        self.emit_line("} catch (e) {");
        self.indent += 1;
        self.emit_line("console.error('Resolver error:', e);");
        self.emit_line(&format!("return {};", fallback_expr));
        self.indent -= 1;
        self.emit_line("}");

        self.indent -= 1;
        self.emit_line("},");

        // Clear cache method
        if resolver.cache.is_some() {
            self.emit_line("clearCache() {");
            self.emit_line("  this._cache = null;");
            self.emit_line("  this._cacheTime = 0;");
            self.emit_line("},");
        }

        self.indent -= 1;
        self.emit_line("};");
        self.emit_line("");
    }

    fn generate_directive(&mut self, directive: &DirectiveDef) {
        let directive_name = &directive.name;
        let params_str = directive.params.join(", ");

        self.emit_line(&format!("const {}Directive = {{", directive_name));
        self.indent += 1;

        // Name for identification
        self.emit_line(&format!("name: '{}',", directive_name));

        // Parameters
        if !directive.params.is_empty() {
            self.emit_line(&format!(
                "params: [{}],",
                directive
                    .params
                    .iter()
                    .map(|p| format!("'{}'", p))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // onMount handler
        if let Some(ref on_mount) = directive.on_mount {
            let mount_expr = self.generate_expression(on_mount);
            if directive.params.is_empty() {
                self.emit_line(&format!("onMount: (el) => ({})(el),", mount_expr));
            } else {
                self.emit_line(&format!("onMount: (el, {}) => ({})(el, {}),", params_str, mount_expr, params_str));
            }
        }

        // onDestroy handler
        if let Some(ref on_destroy) = directive.on_destroy {
            let destroy_expr = self.generate_expression(on_destroy);
            if directive.params.is_empty() {
                self.emit_line(&format!("onDestroy: (el) => ({})(el),", destroy_expr));
            } else {
                self.emit_line(&format!("onDestroy: (el, {}) => ({})(el, {}),", params_str, destroy_expr, params_str));
            }
        }

        // onUpdate handler
        if let Some(ref on_update) = directive.on_update {
            let update_expr = self.generate_expression(on_update);
            if directive.params.is_empty() {
                self.emit_line(&format!("onUpdate: (el) => ({})(el),", update_expr));
            } else {
                self.emit_line(&format!("onUpdate: (el, {}) => ({})(el, {}),", params_str, update_expr, params_str));
            }
        }

        self.indent -= 1;
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

        // Check if this is a data-only component (no params, no type property, no guards, no lifecycle)
        // These should be generated as const objects instead of functions
        // IMPORTANT: Components with "children" or "content" must be functions because:
        // 1. They may reference other components that are defined later (no hoisting for const)
        // 2. Function declarations are hoisted, avoiding "Cannot access X before initialization" errors
        let is_data_component = comp.params.is_empty()
            && comp.alias.is_none()
            && comp.guards.is_empty()
            && comp.init.is_none()
            && comp.destroy.is_none()
            && !comp.properties.iter().any(|p| p.key == "type" || p.key == "children" || p.key == "content");

        if is_data_component {
            self.emit_line(&format!("const {} = {{", component_name));
            self.indent += 1;

            let total_props = comp.properties.len();
            for (i, prop) in comp.properties.iter().enumerate() {
                let comma = if i < total_props - 1 { "," } else { "" };
                // Set current_property_key for proper quoting (e.g., align: vertical -> 'vertical')
                self.current_property_key = Some(prop.key.clone());
                let value = self.generate_expression(&prop.value);
                self.current_property_key = None;
                self.emit_line(&format!("{}: {}{}", prop.key, value, comma));
            }

            self.indent -= 1;
            self.emit_line("};");
            self.local_params = old_params;
            return;
        }

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

        // Add guard check if component has guards
        if !comp.guards.is_empty() {
            let guard_checks = comp.guards.iter()
                .map(|g| format!("!{}Guard.check()", g))
                .collect::<Vec<_>>()
                .join(" || ");
            self.emit_line(&format!("if ({}) return null;", guard_checks));
        }

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

    fn emit_line(&mut self, line: &str) {
        let indent = "  ".repeat(self.indent);
        self.output.push_str(&indent);
        self.output.push_str(line);
        self.output.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn generate(source: &str) -> String {
        let mut lexer = Lexer::new(source).unwrap();
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
        let mut lexer = Lexer::new(source).unwrap();
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

    #[test]
    fn test_generate_from_to_animation() {
        let source = r#"
            Fade >> {
                duration: 300ms
                easing: ease-out
                from: { opacity: 0 }
                to: { opacity: 1 }
            }
        "#;

        let output = generate(source);
        assert!(output.contains("const Fade = {"));
        assert!(output.contains("name: 'Fade'"));
        assert!(output.contains("duration: '300ms'"));
        assert!(output.contains("easing: 'ease-out'"));
        assert!(output.contains("keyframes: ["));
        assert!(output.contains("opacity: 0"));
        assert!(output.contains("opacity: 1"));
        assert!(output.contains("__animations.set('Fade', Fade)"));
    }

    #[test]
    fn test_generate_keyframe_animation() {
        let source = r#"
            Bounce >> {
                duration: 500ms
                0%: { transform: "translateY(0)" }
                50%: { transform: "translateY(-20px)" }
                100%: { transform: "translateY(0)" }
            }
        "#;

        let output = generate(source);
        assert!(output.contains("const Bounce = {"));
        assert!(output.contains("name: 'Bounce'"));
        assert!(output.contains("duration: '500ms'"));
        assert!(output.contains("offset: 0"));
        assert!(output.contains("offset: 0.5"));
        assert!(output.contains("offset: 1"));
        assert!(output.contains("__animations.set('Bounce', Bounce)"));
    }
}
