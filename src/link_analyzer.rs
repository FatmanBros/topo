//! Link analyzer for extracting page navigation relationships
//!
//! This module analyzes .tp files to extract:
//! - `href` attributes in link components
//! - `router.push()` programmatic navigation calls
//! - Shared component relationships

use crate::ast::{BinaryOperator, ComponentDef, Declaration, Expression, ObjectMember};
// Config is now optional - we auto-detect directories
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

/// Node type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Page,
    Api,
    Component,
}

/// Documentation comment parsed from file
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocComment {
    /// Display name from @name tag
    pub name: Option<String>,
    /// Description from @description tag
    pub description: Option<String>,
    /// Author from @author tag
    pub author: Option<String>,
    /// Version from @version tag
    pub version: Option<String>,
    /// Raw doc comment text
    pub raw: Option<String>,
}

/// Node in the page graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageNode {
    /// Route path as ID (e.g., "/dashboard" or "@api/users")
    pub id: String,
    /// Display label (e.g., "Dashboard" or "UserApi")
    pub label: String,
    /// Source file path
    pub file: String,
    /// Whether this is a dynamic route (e.g., /users/[id])
    pub is_dynamic: bool,
    /// Node type (page, api, component)
    pub node_type: NodeType,
    /// Depth for layout ranking (based on path segments)
    /// Static segments count as 2, dynamic segments count as 1
    pub depth: u32,
    /// Documentation comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<DocComment>,
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
    pub api_services: Vec<ApiServiceNode>,
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

/// API Service info for graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiServiceNode {
    pub id: String,
    pub name: String,
    pub file: String,
    /// REST base path (e.g., "/api/authority")
    pub rest_path: Option<String>,
    pub endpoints: Vec<ApiEndpoint>,
}

/// API Endpoint info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub method: String,
    pub path: String,
    pub name: String,
}

/// Link analyzer for .tp files
pub struct LinkAnalyzer {
    #[allow(dead_code)]
    root_dir: PathBuf,
    pages_dir: PathBuf,
    components_dir: PathBuf,
}

impl LinkAnalyzer {
    /// Create a new link analyzer from current directory
    pub fn new() -> Result<Self> {
        let root_dir = std::env::current_dir()?;

        // Try to find pages directory (check multiple common locations)
        let pages_candidates = vec![
            root_dir.join("pages"),
            root_dir.join("src/pages"),
            root_dir.join("demo/pages"),
            root_dir.join("app/pages"),
        ];
        let pages_dir = pages_candidates
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| root_dir.join("pages"));

        // Try to find components directory
        let components_candidates = vec![
            root_dir.join("components"),
            root_dir.join("src/components"),
            root_dir.join("demo/components"),
            root_dir.join("app/components"),
        ];
        let components_dir = components_candidates
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| root_dir.join("components"));

        Ok(Self {
            root_dir,
            pages_dir,
            components_dir,
        })
    }

    /// Extract documentation comment from the beginning of a file
    /// Parses JSDoc-style comments: /** ... */
    fn extract_doc_comment(source: &str) -> Option<DocComment> {
        let trimmed = source.trim_start();

        // Check if file starts with a doc comment
        if !trimmed.starts_with("/**") {
            return None;
        }

        // Find the end of the doc comment
        let end_pos = trimmed.find("*/")?;
        let comment_content = &trimmed[3..end_pos]; // Skip "/**"

        let mut doc = DocComment::default();
        doc.raw = Some(comment_content.trim().to_string());

        // Parse each line for @tags
        for line in comment_content.lines() {
            let line = line.trim().trim_start_matches('*').trim();

            if let Some(rest) = line.strip_prefix("@name") {
                doc.name = Some(rest.trim_start_matches(':').trim().to_string());
            } else if let Some(rest) = line.strip_prefix("@description") {
                doc.description = Some(rest.trim_start_matches(':').trim().to_string());
            } else if let Some(rest) = line.strip_prefix("@author") {
                doc.author = Some(rest.trim_start_matches(':').trim().to_string());
            } else if let Some(rest) = line.strip_prefix("@version") {
                doc.version = Some(rest.trim_start_matches(':').trim().to_string());
            }
        }

        // Only return if at least one field is populated
        if doc.name.is_some() || doc.description.is_some() || doc.author.is_some() || doc.version.is_some() {
            Some(doc)
        } else {
            None
        }
    }

    /// Find all .tp files recursively from a directory
    #[allow(dead_code)]
    fn find_tp_files_in_dir(&self, dir: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_tp_files_recursive(dir, &mut files)?;
        Ok(files)
    }

    /// Recursively collect .tp files, excluding node_modules, target, .git
    #[allow(dead_code)]
    fn collect_tp_files_recursive(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // Skip common non-source directories
        if dir_name == "node_modules" || dir_name == "target" || dir_name.starts_with('.') {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.collect_tp_files_recursive(&path, files)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("tp") {
                files.push(path);
            }
        }

        Ok(())
    }

    /// Calculate depth for layout ranking based on path segments
    /// Static segments count as 2, dynamic segments (e.g., [id]) count as 1
    /// This allows dynamic routes to appear between their parent and sibling static routes
    /// e.g., / = 0, /user = 2, /user/[id] = 3, /customer/info = 4
    fn calculate_depth(route: &str) -> u32 {
        if route == "/" {
            return 0;
        }

        let segments: Vec<&str> = route.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
        let mut depth = 0u32;

        for segment in segments {
            if segment.starts_with('[') && segment.ends_with(']') {
                // Dynamic segment counts as 1
                depth += 1;
            } else {
                // Static segment counts as 2
                depth += 2;
            }
        }

        depth
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

                // Extract doc comment from file
                let doc = fs::read_to_string(file)
                    .ok()
                    .and_then(|source| Self::extract_doc_comment(&source));

                nodes.push(PageNode {
                    id: route.clone(),
                    label,
                    file: file
                        .strip_prefix(&self.pages_dir)
                        .unwrap_or(file)
                        .to_string_lossy()
                        .to_string(),
                    is_dynamic,
                    node_type: NodeType::Page,
                    depth: Self::calculate_depth(&route),
                    doc,
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
            // Extract doc comment from component file
            let doc = fs::read_to_string(&component.file)
                .ok()
                .and_then(|source| Self::extract_doc_comment(&source));

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
                node_type: NodeType::Component,
                depth: 0, // Components are handled separately in layout
                doc,
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

        // Analyze API services by following imports
        let (api_services, api_edges) = self.find_api_services(&page_files, &component_files)?;

        // Add API edges (source is already a route)
        for (source_route, api_id) in api_edges {
            let edge_key = (source_route.clone(), api_id.clone());
            if !seen_edges.contains(&edge_key) {
                seen_edges.insert(edge_key);
                edges.push(PageEdge {
                    source: source_route,
                    target: api_id,
                    link_type: "api-call".to_string(),
                    is_dynamic: false,
                });
            }
        }

        Ok(PageGraph { nodes, edges, api_services })
    }

    /// Find API services by following imports from pages/components
    /// Returns (api_services, edges) where edges is (source_route, api_endpoint_id)
    /// api_endpoint_id format: @api/servicename/methodname
    fn find_api_services(&self, page_files: &[PathBuf], component_files: &[PathBuf]) -> Result<(Vec<ApiServiceNode>, Vec<(String, String)>)> {
        use crate::ast::Declaration;
        use regex::Regex;

        let mut services = Vec::new();
        let mut api_edges: Vec<(String, String)> = Vec::new();
        let mut seen_services: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

        // Build maps:
        // 1. component file -> (api_name, [method_names]) it uses
        // 2. component file -> other component files it imports
        // 3. api_name -> service info
        let mut component_api_calls: HashMap<PathBuf, Vec<(String, String)>> = HashMap::new(); // (api_name, method)
        let mut component_imports: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        let mut api_name_to_id: HashMap<String, String> = HashMap::new(); // ApiName -> @api/apiname

        // Helper to extract component imports from source
        fn extract_component_imports(source: &str, file: &Path) -> Vec<PathBuf> {
            let mut imports = Vec::new();
            for line in source.lines() {
                if line.trim().starts_with("import") {
                    if let Some(path_start) = line.find('"') {
                        if let Some(path_end) = line.rfind('"') {
                            let import_path = &line[path_start + 1..path_end];
                            // Only track .tp component files
                            if import_path.ends_with(".tp") && !import_path.contains("services/") {
                                if let Some(comp_file) = file
                                    .parent()
                                    .map(|p| p.join(import_path))
                                    .and_then(|p| p.canonicalize().ok())
                                {
                                    imports.push(comp_file);
                                }
                            }
                        }
                    }
                }
            }
            imports
        }

        // Helper to extract API method calls from source (e.g., AuthorityApi.login)
        fn extract_api_calls(source: &str, api_names: &[String]) -> Vec<(String, String)> {
            let mut calls = Vec::new();
            for api_name in api_names {
                // Match ApiName.methodName patterns
                let pattern = format!(r"{}\s*\.\s*(\w+)\s*\(", regex::escape(api_name));
                if let Ok(re) = Regex::new(&pattern) {
                    for cap in re.captures_iter(source) {
                        if let Some(method) = cap.get(1) {
                            calls.push((api_name.clone(), method.as_str().to_string()));
                        }
                    }
                }
            }
            calls
        }

        // First pass: collect API service definitions from all component files
        let mut all_api_names: Vec<String> = Vec::new();
        for file in component_files {
            if let Ok(source) = fs::read_to_string(file) {
                for line in source.lines() {
                    if line.trim().starts_with("import") && line.contains("services/") {
                        if let Some(path_start) = line.find('"') {
                            if let Some(path_end) = line.rfind('"') {
                                let import_path = &line[path_start + 1..path_end];
                                let service_file = file
                                    .parent()
                                    .map(|p| p.join(import_path))
                                    .and_then(|p| p.canonicalize().ok());

                                if let Some(service_path) = service_file {
                                    if !seen_services.contains(&service_path) {
                                        if let Ok(service_source) = fs::read_to_string(&service_path) {
                                            let mut lexer = Lexer::new(&service_source);
                                            if let Ok(tokens) = lexer.tokenize() {
                                                let mut parser = TopoParser::new(tokens);
                                                if let Ok(program) = parser.parse() {
                                                    for decl in program.declarations {
                                                        if let Declaration::ApiService(api) = decl {
                                                            let api_id = format!("@api/{}", api.name.to_lowercase());
                                                            api_name_to_id.insert(api.name.clone(), api_id.clone());
                                                            all_api_names.push(api.name.clone());

                                                            let endpoints: Vec<ApiEndpoint> = api
                                                                .endpoints
                                                                .iter()
                                                                .map(|ep| ApiEndpoint {
                                                                    method: format!("{:?}", ep.method).to_uppercase(),
                                                                    path: ep.path.clone(),
                                                                    name: ep.name.clone(),
                                                                })
                                                                .collect();

                                                            services.push(ApiServiceNode {
                                                                id: api_id,
                                                                name: api.name.clone(),
                                                                file: service_path
                                                                    .file_name()
                                                                    .and_then(|n| n.to_str())
                                                                    .unwrap_or("")
                                                                    .to_string(),
                                                                rest_path: api.rest.clone(),
                                                                endpoints,
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        seen_services.insert(service_path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Second pass: collect API calls and component imports
        for file in component_files {
            if let Ok(source) = fs::read_to_string(file) {
                // Track component imports for dependency resolution
                let comp_imports = extract_component_imports(&source, file);
                if !comp_imports.is_empty() {
                    component_imports.insert(file.clone(), comp_imports);
                }

                // Track API method calls
                let api_calls = extract_api_calls(&source, &all_api_names);
                if !api_calls.is_empty() {
                    component_api_calls.insert(file.clone(), api_calls);
                }
            }
        }

        // Helper to recursively collect all API calls from a component (including transitive)
        fn collect_all_api_calls(
            file: &PathBuf,
            api_calls: &HashMap<PathBuf, Vec<(String, String)>>,
            imports: &HashMap<PathBuf, Vec<PathBuf>>,
            visited: &mut HashSet<PathBuf>,
        ) -> Vec<(String, String)> {
            if visited.contains(file) {
                return Vec::new();
            }
            visited.insert(file.clone());

            let mut calls = Vec::new();

            // Add direct API calls
            if let Some(direct) = api_calls.get(file) {
                calls.extend(direct.clone());
            }

            // Recursively add API calls from imported components
            if let Some(imported) = imports.get(file) {
                for imp in imported {
                    calls.extend(collect_all_api_calls(imp, api_calls, imports, visited));
                }
            }

            calls
        }

        // Now, for each page, find which components it imports and add API edges
        for file in page_files {
            if let Some(route) = self.file_to_route(file) {
                if let Ok(source) = fs::read_to_string(file) {
                    // Check for direct API calls in pages
                    let direct_calls = extract_api_calls(&source, &all_api_names);
                    for (api_name, method) in direct_calls {
                        if let Some(api_id) = api_name_to_id.get(&api_name) {
                            let endpoint_id = format!("{}/{}", api_id, method);
                            api_edges.push((route.clone(), endpoint_id));
                        }
                    }

                    // Check for component imports and transitively inherit their API calls
                    let comp_imports = extract_component_imports(&source, file);
                    for comp_path in comp_imports {
                        let mut visited = HashSet::new();
                        let calls = collect_all_api_calls(&comp_path, &component_api_calls, &component_imports, &mut visited);
                        for (api_name, method) in calls {
                            if let Some(api_id) = api_name_to_id.get(&api_name) {
                                let endpoint_id = format!("{}/{}", api_id, method);
                                api_edges.push((route.clone(), endpoint_id));
                            }
                        }
                    }
                }
            }
        }

        Ok((services, api_edges))
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
            Expression::Object { members } => {
                for member in members {
                    match member {
                        ObjectMember::Property(prop) => {
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
                        ObjectMember::Spread { expr } => {
                            self.extract_links_from_expression(expr, source_route, source_file, links);
                        }
                    }
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
            root_dir: PathBuf::from("."),
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
            root_dir: PathBuf::from("."),
            pages_dir: PathBuf::from("pages"),
            components_dir: PathBuf::from("components"),
        };

        assert_eq!(analyzer.to_pascal_case("dashboard"), "Dashboard");
        assert_eq!(analyzer.to_pascal_case("quick-start"), "QuickStart");
        assert_eq!(analyzer.to_pascal_case("user_profile"), "UserProfile");
    }
}
