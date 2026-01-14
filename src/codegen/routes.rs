//! Routes code generation
//!
//! Generates JavaScript code for route definitions and guards.

use crate::ast::*;
use super::JsCodegen;

impl JsCodegen {
    pub(super) fn generate_routes(&mut self, routes: &RoutesDef) {
        // Get unique name to avoid duplicate declarations
        let unique_name = self.get_unique_routes_name(&routes.name);

        self.emit_line(&format!("// Routes: {}", unique_name));
        self.emit_line(&format!("const {} = {{", unique_name));
        self.indent += 1;

        for entry in &routes.routes {
            match &entry.config {
                RouteConfig::Path { path } => {
                    if entry.params.is_empty() {
                        // Simple route: home: () => "/"
                        self.emit_line(&format!("{}: () => '{}',", entry.name, path));
                    } else {
                        // Parameterized route: userDetail: (id) => `/users/${id}`
                        let params = entry.params.join(", ");
                        let template_path = self.convert_path_to_template(path, &entry.params);
                        self.emit_line(&format!("{}: ({}) => `{}`,", entry.name, params, template_path));
                    }
                }
                RouteConfig::PathWithGuards { path, guards: _, can_deactivate: _ } |
                RouteConfig::PathWithResolvers { path, resolvers: _ } |
                RouteConfig::PathWithGuardsAndResolvers { path, guards: _, can_deactivate: _, resolvers: _ } => {
                    // Route with guards/resolvers - same generation, guards/resolvers are handled elsewhere
                    if entry.params.is_empty() {
                        self.emit_line(&format!("{}: () => '{}',", entry.name, path));
                    } else {
                        let params = entry.params.join(", ");
                        let template_path = self.convert_path_to_template(path, &entry.params);
                        self.emit_line(&format!("{}: ({}) => `{}`,", entry.name, params, template_path));
                    }
                }
                RouteConfig::SubRoute { path, route_ref } => {
                    // Route with sub-routes: Object.assign wraps sub-routes to prepend base path
                    // This allows Routes.docs() -> "/docs" and Routes.docs.installation() -> "/docs/installation"
                    if entry.params.is_empty() {
                        // Generate wrapper that prepends base path to all sub-route functions
                        self.emit_line(&format!(
                            "{}: Object.assign(() => '{}', Object.fromEntries(Object.entries({}).map(([k, v]) => [k, (...args) => '{}' + v(...args)]))),",
                            entry.name, path, route_ref, path
                        ));
                    } else {
                        let params = entry.params.join(", ");
                        let template_path = self.convert_path_to_template(path, &entry.params);
                        self.emit_line(&format!(
                            "{}: Object.assign(({}) => `{}`, Object.fromEntries(Object.entries({}).map(([k, v]) => [k, (...args) => `{}` + v(...args)]))),",
                            entry.name, params, template_path, route_ref, template_path
                        ));
                    }
                }
            }
        }

        self.indent -= 1;
        self.emit_line("};");
        self.emit_line("");

        // Generate router object for navigation with guards
        self.generate_routes_router(routes, &unique_name);

        // Generate guards configuration if present
        self.generate_routes_guards(routes, &unique_name);
    }

    pub(super) fn generate_routes_router(&mut self, routes: &RoutesDef, unique_name: &str) {
        // Router name: "Routes" -> "__router", "DocsRoutes" -> "DocsRoutes_router"
        let router_name = if unique_name == "Routes" {
            "__router".to_string()
        } else {
            format!("{}_router", unique_name)
        };

        self.emit_line(&format!("// Router for navigation: {}", router_name));
        self.emit_line(&format!("const {} = {{", router_name));
        self.indent += 1;

        for entry in &routes.routes {
            let (path, guards, can_deactivate, resolvers) = match &entry.config {
                RouteConfig::Path { path } => (path.clone(), Vec::new(), Vec::new(), Vec::new()),
                RouteConfig::PathWithGuards { path, guards, can_deactivate } => {
                    (path.clone(), guards.clone(), can_deactivate.clone(), Vec::new())
                }
                RouteConfig::PathWithResolvers { path, resolvers } => {
                    (path.clone(), Vec::new(), Vec::new(), resolvers.clone())
                }
                RouteConfig::PathWithGuardsAndResolvers { path, guards, can_deactivate, resolvers } => {
                    (path.clone(), guards.clone(), can_deactivate.clone(), resolvers.clone())
                }
                RouteConfig::SubRoute { path, route_ref } => {
                    // Sub-route reference: spread the sub-router
                    let sub_router_name = format!("{}_router", route_ref);
                    self.emit_line(&format!(
                        "{}: {{ path: '{}', guards: [], canDeactivate: [], resolvers: [], ...{} }},",
                        entry.name, path, sub_router_name
                    ));
                    continue;
                }
            };

            let guards_str = if guards.is_empty() {
                "[]".to_string()
            } else {
                let guards_formatted = guards.iter()
                    .map(|g| format!("{}Guard", g))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", guards_formatted)
            };

            let can_deactivate_str = if can_deactivate.is_empty() {
                "[]".to_string()
            } else {
                let deactivate_formatted = can_deactivate.iter()
                    .map(|g| format!("{}Guard", g))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", deactivate_formatted)
            };

            let resolvers_str = if resolvers.is_empty() {
                "[]".to_string()
            } else {
                let resolvers_formatted = resolvers.iter()
                    .map(|r| format!("{}Resolver", r.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", resolvers_formatted)
            };

            self.emit_line(&format!(
                "{}: {{ path: '{}', guards: {}, canDeactivate: {}, resolvers: {} }},",
                entry.name, path, guards_str, can_deactivate_str, resolvers_str
            ));
        }

        self.indent -= 1;
        self.emit_line("};");
        self.emit_line("");

        // Generate navigateWithGuards helper only for main router
        if unique_name == "Routes" {
            self.emit_line("function navigateWithGuards(route) {");
            self.emit_line("  if (!route || !route.path) return false;");
            self.emit_line("  for (const guard of (route.guards || [])) {");
            self.emit_line("    if (guard && typeof guard.check === 'function' && !guard.check()) {");
            self.emit_line("      if (guard.redirect) window.location.hash = guard.redirect;");
            self.emit_line("      return false;");
            self.emit_line("    }");
            self.emit_line("  }");
            self.emit_line("  window.location.hash = route.path;");
            self.emit_line("  return true;");
            self.emit_line("}");
            self.emit_line("");

            // Generate navigateWithResolvers helper
            self.emit_line("async function navigateWithResolvers(route, params = {}) {");
            self.emit_line("  if (!route || !route.path) return { success: false };");
            self.emit_line("  // Execute guards first");
            self.emit_line("  for (const guard of (route.guards || [])) {");
            self.emit_line("    if (guard && typeof guard.check === 'function' && !guard.check()) {");
            self.emit_line("      if (guard.redirect) window.location.hash = guard.redirect;");
            self.emit_line("      return { success: false, blocked: true };");
            self.emit_line("    }");
            self.emit_line("  }");
            self.emit_line("  // Execute resolvers in parallel");
            self.emit_line("  const resolvedData = {};");
            self.emit_line("  const resolverPromises = (route.resolvers || []).map(async (resolver) => {");
            self.emit_line("    if (resolver && typeof resolver.resolve === 'function') {");
            self.emit_line("      const args = resolver.params ? resolver.params.map(p => params[p]) : [];");
            self.emit_line("      resolvedData[resolver.name || 'data'] = await resolver.resolve(...args);");
            self.emit_line("    }");
            self.emit_line("  });");
            self.emit_line("  try {");
            self.emit_line("    await Promise.all(resolverPromises);");
            self.emit_line("  } catch (e) {");
            self.emit_line("    console.error('Resolver execution error:', e);");
            self.emit_line("    return { success: false, error: e };");
            self.emit_line("  }");
            self.emit_line("  // Navigate");
            self.emit_line("  window.location.hash = route.path;");
            self.emit_line("  return { success: true, data: resolvedData };");
            self.emit_line("}");
            self.emit_line("");

            // Generate canLeave helper for canDeactivate guards
            self.emit_line("let __currentRoute = null;");
            self.emit_line("");
            self.emit_line("function setCurrentRoute(route) {");
            self.emit_line("  __currentRoute = route;");
            self.emit_line("}");
            self.emit_line("");
            self.emit_line("function canLeaveCurrentRoute() {");
            self.emit_line("  if (!__currentRoute) return true;");
            self.emit_line("  for (const guard of (__currentRoute.canDeactivate || [])) {");
            self.emit_line("    if (guard && typeof guard.check === 'function' && !guard.check()) {");
            self.emit_line("      return false;");
            self.emit_line("    }");
            self.emit_line("  }");
            self.emit_line("  return true;");
            self.emit_line("}");
            self.emit_line("");
            self.emit_line("// Enhanced navigation with canDeactivate support");
            self.emit_line("function navigateTo(route, params = {}) {");
            self.emit_line("  if (!canLeaveCurrentRoute()) {");
            self.emit_line("    return { success: false, blocked: 'canDeactivate' };");
            self.emit_line("  }");
            self.emit_line("  // Execute activate guards");
            self.emit_line("  for (const guard of (route.guards || [])) {");
            self.emit_line("    if (guard && typeof guard.check === 'function' && !guard.check()) {");
            self.emit_line("      if (guard.redirect) window.location.hash = guard.redirect;");
            self.emit_line("      return { success: false, blocked: 'guard' };");
            self.emit_line("    }");
            self.emit_line("  }");
            self.emit_line("  setCurrentRoute(route);");
            self.emit_line("  window.location.hash = route.path;");
            self.emit_line("  return { success: true };");
            self.emit_line("}");
            self.emit_line("");
            self.emit_line("// beforeunload handler for canDeactivate");
            self.emit_line("window.addEventListener('beforeunload', (e) => {");
            self.emit_line("  if (!canLeaveCurrentRoute()) {");
            self.emit_line("    e.preventDefault();");
            self.emit_line("    e.returnValue = '';");
            self.emit_line("  }");
            self.emit_line("});");
            self.emit_line("");
        }
    }

    pub(super) fn generate_routes_guards(&mut self, routes: &RoutesDef, unique_name: &str) {
        // Collect route-specific guards from PathWithGuards and PathWithGuardsAndResolvers configs
        let mut route_guards: Vec<(String, Vec<String>)> = Vec::new();
        for entry in &routes.routes {
            match &entry.config {
                RouteConfig::PathWithGuards { path, guards, can_deactivate: _ } => {
                    route_guards.push((path.clone(), guards.clone()));
                }
                RouteConfig::PathWithGuardsAndResolvers { path, guards, can_deactivate: _, .. } => {
                    route_guards.push((path.clone(), guards.clone()));
                }
                _ => {}
            }
        }

        // Only generate if there are guards or guard config
        if routes.guards.is_none() && route_guards.is_empty() {
            return;
        }

        self.emit_line(&format!("const {}Guards = {{", unique_name));
        self.indent += 1;

        // Global guards
        if let Some(guards_config) = &routes.guards {
            if !guards_config.global.is_empty() {
                let guards = guards_config.global.join(", ");
                self.emit_line(&format!("global: [{}],", guards));
            } else {
                self.emit_line("global: [],");
            }

            // Skip routes
            if !guards_config.skip.is_empty() {
                let skips = guards_config.skip.iter()
                    .map(|s| format!("'{}'", s))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.emit_line(&format!("skip: [{}],", skips));
            }
        } else {
            self.emit_line("global: [],");
        }

        // Route-specific guards
        if !route_guards.is_empty() {
            self.emit_line("routes: {");
            self.indent += 1;
            for (path, guards) in &route_guards {
                let guards_str = guards.join(", ");
                self.emit_line(&format!("'{}': [{}],", path, guards_str));
            }
            self.indent -= 1;
            self.emit_line("},");
        }

        self.indent -= 1;
        self.emit_line("};");
        self.emit_line("");
    }

    /// Convert path like "/users/{id}" to template literal "/users/${id}"
    pub(super) fn convert_path_to_template(&self, path: &str, params: &[String]) -> String {
        let mut result = path.to_string();
        for param in params {
            result = result.replace(&format!("{{{}}}", param), &format!("${{{}}}", param));
        }
        result
    }

    pub(super) fn generate_guard_setup(&mut self, setup: &GuardSetupDef) {
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

}
