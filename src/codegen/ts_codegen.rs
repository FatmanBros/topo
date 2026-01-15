//! TypeScript type generation
//!
//! Generates TypeScript type definitions from topo AST.

use crate::ast::*;

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
            Declaration::Method(_) => {}      // Methods don't need type exports
            Declaration::Test(_) => {}        // Tests don't need type exports
            Declaration::BeforeEach(_) => {}  // Tests don't need type exports
            Declaration::AfterEach(_) => {}   // Tests don't need type exports
            Declaration::BeforeOnce(_) => {}  // Tests don't need type exports
            Declaration::AfterOnce(_) => {}   // Tests don't need type exports
            Declaration::Guard(_) => {}       // Guards don't need type exports
            Declaration::GuardSetup(_) => {}  // GuardSetup doesn't need type exports
            Declaration::Resolver(_) => {}    // Resolvers don't need type exports
            Declaration::Directive(_) => {}   // Directives don't need type exports
            Declaration::Routes(_) => {}      // Routes types are handled separately
            Declaration::Function(_) => {}    // Functions don't need type exports
            Declaration::Schema(_) => {}      // Schema is used for type inference, not type exports
            Declaration::Repository(_) => {}  // Repository types are generated from schema
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
        let store_name = store
            .name
            .clone()
            .unwrap_or_else(|| "AnonymousStore".to_string());

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

                let separator = if i < actions.actions.len() - 1 {
                    " |"
                } else {
                    ";"
                };
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
        self.emit_line(&format!(
            "export declare const {}: {}Store;",
            store_name, store_name
        ));
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

        self.emit_line(&format!(
            "export declare const {}Api: {}Api;",
            api.name, api.name
        ));
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

    #[allow(clippy::only_used_in_recursion)]
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

    #[allow(clippy::only_used_in_recursion)]
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
            Expression::Object { members } => {
                let fields: Vec<String> = members
                    .iter()
                    .filter_map(|m| match m {
                        ObjectMember::Property(p) => {
                            Some(format!("{}: {}", p.key, self.infer_type_from_expr(&p.value)))
                        }
                        ObjectMember::Spread { .. } => None, // Skip spreads in type inference
                    })
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
