//! API service code generation
//!
//! Generates JavaScript code for API services (REST, WebSocket, SSE).

use crate::ast::*;
use super::JsCodegen;

impl JsCodegen {
    pub(super) fn generate_api_service(&mut self, api: &ApiServiceDef) {
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

        // Add mock support if mock path is specified
        if let Some(mock_path) = &api.mock {
            self.emit_line(&format!("_mockPath: '{}',", mock_path));
            self.emit_line("_mockData: null,");
            self.emit_line("");
            self.emit_line("async _loadMock() {");
            self.indent += 1;
            self.emit_line("if (this._mockData !== null) return this._mockData;");
            self.emit_line("try {");
            self.indent += 1;
            self.emit_line("const response = await fetch(this._mockPath);");
            self.emit_line("this._mockData = await response.json();");
            self.emit_line("return this._mockData;");
            self.indent -= 1;
            self.emit_line("} catch (e) {");
            self.indent += 1;
            self.emit_line("console.warn('Failed to load mock data:', e);");
            self.emit_line("return null;");
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("},");
            self.emit_line("");
        }

        if let Some(base_url) = &api.rest {
            // Generate CRUD methods
            self.emit_line(&format!("baseUrl: '{}',", base_url));
            self.emit_line("");

            // getAll - mock対応
            self.emit_line("async getAll() {");
            self.indent += 1;
            if api.mock.is_some() {
                self.emit_line("if (typeof __devtools !== 'undefined' && __devtools.enabled) {");
                self.indent += 1;
                self.emit_line("const mock = await this._loadMock();");
                self.emit_line("if (mock) return Array.isArray(mock) ? mock : mock.data || mock.items || [];");
                self.indent -= 1;
                self.emit_line("}");
            }
            self.emit_line(&format!("return fetch('{}').then(r => r.json());", base_url));
            self.indent -= 1;
            self.emit_line("},");
            self.emit_line("");

            // getById - mock対応
            self.emit_line("async getById(id) {");
            self.indent += 1;
            if api.mock.is_some() {
                self.emit_line("if (typeof __devtools !== 'undefined' && __devtools.enabled) {");
                self.indent += 1;
                self.emit_line("const mock = await this._loadMock();");
                self.emit_line("if (mock) {");
                self.indent += 1;
                self.emit_line("const items = Array.isArray(mock) ? mock : mock.data || mock.items || [];");
                self.emit_line("return items.find(item => item.id === id || item.id === String(id));");
                self.indent -= 1;
                self.emit_line("}");
                self.indent -= 1;
                self.emit_line("}");
            }
            self.emit_line(&format!("return fetch(`{}/${{id}}`).then(r => r.json());", base_url));
            self.indent -= 1;
            self.emit_line("},");
            self.emit_line("");

            // create - mock対応（devモードではデータをそのまま返す）
            self.emit_line("async create(data) {");
            self.indent += 1;
            if api.mock.is_some() {
                self.emit_line("if (typeof __devtools !== 'undefined' && __devtools.enabled) {");
                self.indent += 1;
                self.emit_line("console.log('[Mock] create:', data);");
                self.emit_line("return { ...data, id: Date.now() };");
                self.indent -= 1;
                self.emit_line("}");
            }
            self.emit_line(&format!(
                "return fetch('{}', {{ method: 'POST', body: JSON.stringify(data), headers: {{ 'Content-Type': 'application/json' }} }}).then(r => r.json());",
                base_url
            ));
            self.indent -= 1;
            self.emit_line("},");
            self.emit_line("");

            // update - mock対応
            self.emit_line("async update(id, data) {");
            self.indent += 1;
            if api.mock.is_some() {
                self.emit_line("if (typeof __devtools !== 'undefined' && __devtools.enabled) {");
                self.indent += 1;
                self.emit_line("console.log('[Mock] update:', id, data);");
                self.emit_line("return { ...data, id };");
                self.indent -= 1;
                self.emit_line("}");
            }
            self.emit_line(&format!(
                "return fetch(`{}/${{id}}`, {{ method: 'PUT', body: JSON.stringify(data), headers: {{ 'Content-Type': 'application/json' }} }}).then(r => r.json());",
                base_url
            ));
            self.indent -= 1;
            self.emit_line("},");
            self.emit_line("");

            // delete - mock対応
            self.emit_line("async delete(id) {");
            self.indent += 1;
            if api.mock.is_some() {
                self.emit_line("if (typeof __devtools !== 'undefined' && __devtools.enabled) {");
                self.indent += 1;
                self.emit_line("console.log('[Mock] delete:', id);");
                self.emit_line("return { success: true };");
                self.indent -= 1;
                self.emit_line("}");
            }
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
        let Some(subscribe_url) = api.subscribe.as_ref() else {
            return; // No subscribe URL, nothing to generate
        };
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
        let Some(subscribe_url) = api.subscribe.as_ref() else {
            return; // No subscribe URL, nothing to generate
        };
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

        // Generate JSDoc comment with type information
        let has_request = endpoint.request_type.is_some();
        let has_response = endpoint.response_type.is_some();
        let has_error = endpoint.error_type.is_some();

        if has_request || has_response || has_error {
            self.emit_line("/**");
            if let Some(ref req_type) = endpoint.request_type {
                self.emit_line(&format!(" * @param {{{}}} data - Request body", self.format_type_annotation_for_jsdoc(req_type)));
            }
            if let Some(ref res_type) = endpoint.response_type {
                self.emit_line(&format!(" * @returns {{Promise<{}>}}", self.format_type_annotation_for_jsdoc(res_type)));
            }
            if let Some(ref err_type) = endpoint.error_type {
                self.emit_line(&format!(" * @throws {{{}}} API error", self.format_type_annotation_for_jsdoc(err_type)));
            }
            self.emit_line(" */");
        }

        // Generate function with appropriate parameters
        let param_name = if has_request { "data" } else { "" };
        self.emit_line(&format!("async {}({}) {{", endpoint.name, param_name));
        self.indent += 1;

        // Add mock support for custom endpoints
        if api.mock.is_some() {
            self.emit_line("if (typeof __devtools !== 'undefined' && __devtools.enabled) {");
            self.indent += 1;
            self.emit_line("const mock = await this._loadMock();");
            self.emit_line("if (mock) {");
            self.indent += 1;
            // Try to find endpoint-specific mock data
            self.emit_line(&format!("const endpointMock = mock['{}'] || mock.{};", endpoint.name, endpoint.name));
            self.emit_line("if (endpointMock !== undefined) return endpointMock;");
            // Fallback: for GET methods, return the array/data
            if matches!(endpoint.method, HttpMethod::Get) {
                self.emit_line("return Array.isArray(mock) ? mock : mock.data || mock.items || mock;");
            } else {
                if has_request {
                    self.emit_line(&format!("console.log('[Mock] {}:', data);", endpoint.name));
                    self.emit_line("return { ...data, id: Date.now() };");
                } else {
                    self.emit_line(&format!("console.log('[Mock] {}');", endpoint.name));
                    self.emit_line("return { success: true };");
                }
            }
            self.indent -= 1;
            self.emit_line("}");
            self.indent -= 1;
            self.emit_line("}");
        }

        // Generate fetch call
        let needs_body = matches!(endpoint.method, HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch) && has_request;

        if needs_body {
            self.emit_line(&format!(
                "return fetch('{}', {{ method: '{}', headers: {{ 'Content-Type': 'application/json' }}, body: JSON.stringify(data) }}).then(r => r.json());",
                full_path, method
            ));
        } else {
            self.emit_line(&format!(
                "return fetch('{}', {{ method: '{}' }}).then(r => r.json());",
                full_path, method
            ));
        }
        self.indent -= 1;
        self.emit_line("},");
    }

    /// Format TypeAnnotation for JSDoc
    #[allow(clippy::only_used_in_recursion)]
    fn format_type_annotation_for_jsdoc(&self, type_ann: &TypeAnnotation) -> String {
        match type_ann {
            TypeAnnotation::Primitive { name } => name.clone(),
            TypeAnnotation::Array { element_type } => {
                format!("{}[]", self.format_type_annotation_for_jsdoc(element_type))
            }
            TypeAnnotation::Optional { inner_type } => {
                format!("{}|null", self.format_type_annotation_for_jsdoc(inner_type))
            }
            TypeAnnotation::Object { fields } => {
                let field_strs: Vec<String> = fields
                    .iter()
                    .map(|f| format!("{}: {}", f.name, self.format_type_annotation_for_jsdoc(&f.type_annotation)))
                    .collect();
                format!("{{ {} }}", field_strs.join(", "))
            }
            TypeAnnotation::Union { types } => {
                let type_strs: Vec<String> = types.iter().map(|t| self.format_type_annotation_for_jsdoc(t)).collect();
                format!("({})", type_strs.join("|"))
            }
            TypeAnnotation::Reference { name } => name.clone(),
        }
    }
}
