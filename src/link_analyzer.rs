//! Link analyzer for extracting page navigation relationships
//!
//! This module analyzes .tp files to extract:
//! - `href` attributes in link components
//! - `router.push()` programmatic navigation calls
//! - Shared component relationships

use crate::ast::{BinaryOperator, ComponentDef, Declaration, Expression};
use crate::config::Config;
use crate::lexer::Lexer;
use crate::parser::Parser as TopoParser;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Link type classification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkType {
    /// Declarative link using href attribute
    Declarative,
    /// Programmatic navigation using router.push()
    Programmatic,
    /// Component import relationship
    ComponentLink,
}

/// A detected link from one page to another
#[derive(Debug, Clone)]
pub struct PageLink {
    /// Source page route (e.g., "/dashboard")
    pub source: String,
    /// Target page route (e.g., "/users")
    pub target: Option<String>,
    /// Link type
    pub link_type: LinkType,
    /// Whether this is a dynamic link (uses variables)
    pub is_dynamic: bool,
    /// Source file path
    pub source_file: String,
}

/// Node in the page graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageNode {
    /// Route path as ID (e.g., "/dashboard" or "@components/navbar")
    pub id: String,
    /// Display label (e.g., "Dashboard" or "Navbar")
    pub label: String,
    /// Source file path
    pub file: String,
    /// Whether this is a dynamic route (e.g., /users/[id])
    pub is_dynamic: bool,
}

/// Edge in the page graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageEdge {
    /// Source node ID (route path)
    pub source: String,
    /// Target node ID (route path)
    pub target: String,
    /// Link type
    pub link_type: String,
    /// Whether the link target is dynamic
    pub is_dynamic: bool,
}

/// Complete page navigation graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageGraph {
    pub nodes: Vec<PageNode>,
    pub edges: Vec<PageEdge>,
}

/// Shared component info
#[derive(Debug, Clone)]
struct SharedComponent {
    #[allow(dead_code)]
    id: String,
    label: String,
    file: PathBuf,
    links: Vec<PageLink>,
}

/// Link analyzer for .tp files
pub struct LinkAnalyzer {
    pages_dir: PathBuf,
    components_dir: PathBuf,
}

impl LinkAnalyzer {
    /// Create a new link analyzer
    pub fn new() -> Result<Self> {
        let config = Config::load_or_default();
        let paths_config = config.paths_config();
        let pages_dir = PathBuf::from(&paths_config.pages);
        let components_dir = PathBuf::from(&paths_config.components);

        Ok(Self {
            pages_dir,
            components_dir,
        })
    }

    /// Build the complete page graph
    pub fn build_graph(&self) -> Result<PageGraph> {
        let page_files = self.find_page_files()?;
        let component_files = self.find_component_files()?;

        let mut nodes = Vec::new();
        let mut links = Vec::new();

        // Track which components have links
        let mut components_with_links: HashMap<String, SharedComponent> = HashMap::new();

        // First pass: analyze components for links
        for file in &component_files {
            let component_id = self.file_to_component_id(file);
            let label = self.component_to_label(file);

            if let Ok(component_links) = self.extract_links_from_file(file, &component_id) {
                if !component_links.is_empty() {
                    components_with_links.insert(
                        component_id.clone(),
                        SharedComponent {
                            id: component_id,
                            label,
                            file: file.clone(),
                            links: component_links,
                        },
                    );
                }
            }
        }

        // Build nodes from page files
        let mut page_imports: HashMap<String, Vec<String>> = HashMap::new();

        for file in &page_files {
            if let Some(route) = self.file_to_route(file) {
                let label = self.route_to_label(&route, file);
                let is_dynamic = route.contains('[');

                nodes.push(PageNode {
                    id: route.clone(),
                    label,
                    file: file
                        .strip_prefix(&self.pages_dir)
                        .unwrap_or(file)
                        .to_string_lossy()
                        .to_string(),
                    is_dynamic,
                });

                // Extract links and imports from this file
                if let Ok(page_links) = self.extract_links_from_file(file, &route) {
                    links.extend(page_links);
                }

                // Track component imports
                if let Ok(imports) = self.extract_component_imports(file) {
                    for import_path in imports {
                        let component_id = self.import_path_to_component_id(&import_path);
                        if components_with_links.contains_key(&component_id) {
                            page_imports
                                .entry(route.clone())
                                .or_default()
                                .push(component_id);
                        }
                    }
                }
            }
        }

        // Add component nodes (only those with links)
        for (component_id, component) in &components_with_links {
            nodes.push(PageNode {
                id: component_id.clone(),
                label: component.label.clone(),
                file: component
                    .file
                    .strip_prefix(&self.components_dir)
                    .unwrap_or(&component.file)
                    .to_string_lossy()
                    .to_string(),
                is_dynamic: false,
            });

            // Add links from this component
            links.extend(component.links.clone());
        }

        // Add page -> component edges
        for (page_route, component_ids) in page_imports {
            for component_id in component_ids {
                links.push(PageLink {
                    source: page_route.clone(),
                    target: Some(component_id),
                    link_type: LinkType::ComponentLink,
                    is_dynamic: false,
                    source_file: String::new(),
                });
            }
        }

        // Build edges from links
        let route_set: HashSet<_> = nodes.iter().map(|n| n.id.clone()).collect();
        let mut edges = Vec::new();
        let mut seen_edges: HashSet<(String, String)> = HashSet::new();

        for link in links {
            if let Some(target) = &link.target {
                // Skip external links and anchors (except for component IDs)
                if !target.starts_with('@') {
                    if target.starts_with("http://")
                        || target.starts_with("https://")
                        || target.starts_with('#')
                    {
                        continue;
                    }
                }

                // Normalize the target path
                let normalized_target = if target.starts_with('@') {
                    target.clone()
                } else {
                    self.normalize_route(target)
                };

                // Check if target exists in routes
                let target_exists = route_set.contains(&normalized_target)
                    || self.matches_dynamic_route(&normalized_target, &route_set);

                if target_exists || link.is_dynamic {
                    let edge_key = (link.source.clone(), normalized_target.clone());
                    if !seen_edges.contains(&edge_key) {
                        seen_edges.insert(edge_key);
                        edges.push(PageEdge {
                            source: link.source.clone(),
                            target: if normalized_target.starts_with('@') {
                                normalized_target
                            } else {
                                self.resolve_dynamic_target(&normalized_target, &route_set)
                            },
                            link_type: match link.link_type {
                                LinkType::Declarative => "declarative".to_string(),
                                LinkType::Programmatic => "programmatic".to_string(),
                                LinkType::ComponentLink => "component-link".to_string(),
                            },
                            is_dynamic: link.is_dynamic,
                        });
                    }
                }
            }
        }

        Ok(PageGraph { nodes, edges })
    }

    /// Find all .tp files in the pages directory
    fn find_page_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_page_files(&self.pages_dir, &mut files)?;
        Ok(files)
    }

    fn collect_page_files(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                // Skip components and mocks directories (not routes)
                if dir_name != "components" && dir_name != "mocks" {
                    self.collect_page_files(&path, files)?;
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("tp") {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                // Skip non-route files
                if file_name != "template.tp"
                    && file_name != "store.tp"
                    && file_name != "layout.tp"
                {
                    files.push(path);
                }
            }
        }

        Ok(())
    }

    /// Find all .tp files in the components directory
    fn find_component_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_component_files(&self.components_dir, &mut files)?;
        Ok(files)
    }

    fn collect_component_files(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.collect_component_files(&path, files)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("tp") {
                files.push(path);
            }
        }

        Ok(())
    }

    /// Convert a file path to a route
    fn file_to_route(&self, file: &Path) -> Option<String> {
        let relative = file.strip_prefix(&self.pages_dir).ok()?;
        let mut route = String::from("/");

        let components: Vec<_> = relative
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();

        for (i, component) in components.iter().enumerate() {
            let is_last = i == components.len() - 1;

            if is_last {
                // Handle the file name
                let name = component.trim_end_matches(".tp");
                if name != "index" {
                    if route != "/" {
                        route.push('/');
                    }
                    route.push_str(name);
                }
            } else {
                // Handle directory
                if route != "/" {
                    route.push('/');
                }
                route.push_str(component);
            }
        }

        Some(route)
    }

    /// Convert a component file path to a component ID
    fn file_to_component_id(&self, file: &Path) -> String {
        let relative = file
            .strip_prefix(&self.components_dir)
            .unwrap_or(file)
            .to_string_lossy();
        format!("@components/{}", relative.trim_end_matches(".tp"))
    }

    /// Convert an import path to a component ID
    fn import_path_to_component_id(&self, import_path: &str) -> String {
        // Handle relative imports like "../components/atoms/button.tp"
        if import_path.contains("components/") {
            let parts: Vec<_> = import_path.split("components/").collect();
            if parts.len() > 1 {
                return format!("@components/{}", parts[1].trim_end_matches(".tp"));
            }
        }
        format!("@{}", import_path.trim_end_matches(".tp"))
    }

    /// Convert a component file to a display label
    fn component_to_label(&self, file: &Path) -> String {
        file.file_stem()
            .and_then(|n| n.to_str())
            .map(|s| self.to_pascal_case(s))
            .unwrap_or_else(|| "Component".to_string())
    }

    /// Convert a route to a display label
    fn route_to_label(&self, _route: &str, file: &Path) -> String {
        let file_name = file
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        if file_name == "index" {
            // Use parent directory name
            file.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|s| self.to_pascal_case(s))
                .unwrap_or_else(|| "Home".to_string())
        } else if file_name.starts_with('[') && file_name.ends_with(']') {
            // Dynamic route: [id] -> Detail
            file.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|s| format!("{}Detail", self.to_pascal_case(s)))
                .unwrap_or_else(|| "Detail".to_string())
        } else {
            self.to_pascal_case(file_name)
        }
    }

    fn to_pascal_case(&self, s: &str) -> String {
        s.split(|c: char| c == '-' || c == '_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                }
            })
            .collect()
    }

    /// Extract component imports from a file
    fn extract_component_imports(&self, file: &Path) -> Result<Vec<String>> {
        let source = fs::read_to_string(file)?;
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize()?;
        let mut parser = TopoParser::new(tokens);
        let program = parser.parse()?;

        let mut imports = Vec::new();

        for decl in &program.declarations {
            if let Declaration::Import(import_def) = decl {
                if import_def.path.contains("components/") {
                    imports.push(import_def.path.clone());
                }
            }
        }

        Ok(imports)
    }

    /// Extract links from a .tp file
    fn extract_links_from_file(&self, file: &Path, source_route: &str) -> Result<Vec<PageLink>> {
        let source = fs::read_to_string(file)?;
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize()?;
        let mut parser = TopoParser::new(tokens);
        let program = parser.parse()?;

        let mut links = Vec::new();
        let source_file = file.to_string_lossy().to_string();

        for decl in &program.declarations {
            self.extract_links_from_declaration(decl, source_route, &source_file, &mut links);
        }

        Ok(links)
    }

    fn extract_links_from_declaration(
        &self,
        decl: &Declaration,
        source_route: &str,
        source_file: &str,
        links: &mut Vec<PageLink>,
    ) {
        match decl {
            Declaration::Component(comp) => {
                self.extract_links_from_component(comp, source_route, source_file, links);
            }
            _ => {}
        }
    }

    fn extract_links_from_component(
        &self,
        comp: &ComponentDef,
        source_route: &str,
        source_file: &str,
        links: &mut Vec<PageLink>,
    ) {
        for prop in &comp.properties {
            // Check for href property
            if prop.key == "href" {
                if let Some((target, is_dynamic)) = self.extract_link_target(&prop.value) {
                    links.push(PageLink {
                        source: source_route.to_string(),
                        target: Some(target),
                        link_type: LinkType::Declarative,
                        is_dynamic,
                        source_file: source_file.to_string(),
                    });
                }
            }

            // Check for onClick with router.push
            if prop.key == "onClick" || prop.key == "click" {
                if let Some((target, is_dynamic)) = self.extract_router_push(&prop.value) {
                    links.push(PageLink {
                        source: source_route.to_string(),
                        target: Some(target),
                        link_type: LinkType::Programmatic,
                        is_dynamic,
                        source_file: source_file.to_string(),
                    });
                }
            }

            // Recursively check nested expressions
            self.extract_links_from_expression(&prop.value, source_route, source_file, links);
        }
    }

    fn extract_links_from_expression(
        &self,
        expr: &Expression,
        source_route: &str,
        source_file: &str,
        links: &mut Vec<PageLink>,
    ) {
        match expr {
            Expression::Object { properties } => {
                for prop in properties {
                    if prop.key == "href" {
                        if let Some((target, is_dynamic)) = self.extract_link_target(&prop.value) {
                            links.push(PageLink {
                                source: source_route.to_string(),
                                target: Some(target),
                                link_type: LinkType::Declarative,
                                is_dynamic,
                                source_file: source_file.to_string(),
                            });
                        }
                    }
                    self.extract_links_from_expression(&prop.value, source_route, source_file, links);
                }
            }
            Expression::Array { elements } => {
                for elem in elements {
                    self.extract_links_from_expression(elem, source_route, source_file, links);
                }
            }
            Expression::Call { callee, args } => {
                // Check for router.push()
                if let Some((target, is_dynamic)) = self.extract_router_push(expr) {
                    links.push(PageLink {
                        source: source_route.to_string(),
                        target: Some(target),
                        link_type: LinkType::Programmatic,
                        is_dynamic,
                        source_file: source_file.to_string(),
                    });
                }
                // Recurse into callee and args
                self.extract_links_from_expression(callee, source_route, source_file, links);
                for arg in args {
                    self.extract_links_from_expression(arg, source_route, source_file, links);
                }
            }
            Expression::Conditional { condition, then_branch, else_branch } => {
                self.extract_links_from_expression(condition, source_route, source_file, links);
                self.extract_links_from_expression(then_branch, source_route, source_file, links);
                self.extract_links_from_expression(else_branch, source_route, source_file, links);
            }
            Expression::BinaryOp { left, right, .. } => {
                self.extract_links_from_expression(left, source_route, source_file, links);
                self.extract_links_from_expression(right, source_route, source_file, links);
            }
            Expression::ForIn { items, body, .. } => {
                self.extract_links_from_expression(items, source_route, source_file, links);
                self.extract_links_from_expression(body, source_route, source_file, links);
            }
            _ => {}
        }
    }

    /// Extract link target from an expression
    fn extract_link_target(&self, expr: &Expression) -> Option<(String, bool)> {
        match expr {
            Expression::String { value } => Some((value.clone(), false)),
            Expression::BinaryOp {
                left,
                op: BinaryOperator::Add,
                ..
            } => {
                // Handle string concatenation like "/users/" + id
                if let Expression::String { value } = left.as_ref() {
                    Some((format!("{}[dynamic]", value), true))
                } else {
                    // Try to extract from nested
                    self.extract_link_target(left).map(|(t, _)| (t, true))
                }
            }
            Expression::Conditional { then_branch, .. } => {
                // Use the then branch as primary
                self.extract_link_target(then_branch).map(|(t, _)| (t, true))
            }
            _ => None, // Dynamic link (props.href, etc.)
        }
    }

    /// Extract router.push() target
    fn extract_router_push(&self, expr: &Expression) -> Option<(String, bool)> {
        if let Expression::Call { callee, args } = expr {
            if let Expression::MemberAccess { object, property } = callee.as_ref() {
                if let Expression::Identifier { name } = object.as_ref() {
                    if (name == "router" || name == "Router") && property == "push" {
                        if let Some(arg) = args.first() {
                            return self.extract_link_target(arg);
                        }
                    }
                }
            }
        }

        None
    }

    /// Normalize a route path
    fn normalize_route(&self, route: &str) -> String {
        let route = route.trim_end_matches('/');
        if route.is_empty() {
            "/".to_string()
        } else if !route.starts_with('/') {
            format!("/{}", route)
        } else {
            route.to_string()
        }
    }

    /// Check if a target matches any dynamic route
    fn matches_dynamic_route(&self, target: &str, routes: &HashSet<String>) -> bool {
        // Check for patterns like /users/123 matching /users/[id]
        let parts: Vec<_> = target.split('/').collect();

        for route in routes {
            if route.contains('[') {
                let route_parts: Vec<_> = route.split('/').collect();
                if parts.len() == route_parts.len() {
                    let matches = parts.iter().zip(route_parts.iter()).all(|(t, r)| {
                        r.starts_with('[') && r.ends_with(']') || t == r
                    });
                    if matches {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Resolve a dynamic target to its route pattern
    fn resolve_dynamic_target(&self, target: &str, routes: &HashSet<String>) -> String {
        // If target ends with [dynamic], find matching pattern
        if target.ends_with("[dynamic]") {
            let prefix = target.trim_end_matches("[dynamic]");
            for route in routes {
                if route.starts_with(prefix) && route.contains('[') {
                    return route.clone();
                }
            }
        }

        // Check if target matches a dynamic route pattern
        let parts: Vec<_> = target.split('/').collect();
        for route in routes {
            if route.contains('[') {
                let route_parts: Vec<_> = route.split('/').collect();
                if parts.len() == route_parts.len() {
                    let matches = parts.iter().zip(route_parts.iter()).all(|(t, r)| {
                        r.starts_with('[') && r.ends_with(']') || t == r
                    });
                    if matches {
                        return route.clone();
                    }
                }
            }
        }

        target.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_route() {
        let analyzer = LinkAnalyzer {
            pages_dir: PathBuf::from("pages"),
            components_dir: PathBuf::from("components"),
        };

        assert_eq!(analyzer.normalize_route("/about"), "/about");
        assert_eq!(analyzer.normalize_route("about"), "/about");
        assert_eq!(analyzer.normalize_route("/about/"), "/about");
        assert_eq!(analyzer.normalize_route(""), "/");
    }

    #[test]
    fn test_to_pascal_case() {
        let analyzer = LinkAnalyzer {
            pages_dir: PathBuf::from("pages"),
            components_dir: PathBuf::from("components"),
        };

        assert_eq!(analyzer.to_pascal_case("dashboard"), "Dashboard");
        assert_eq!(analyzer.to_pascal_case("quick-start"), "QuickStart");
        assert_eq!(analyzer.to_pascal_case("user_profile"), "UserProfile");
    }
}
