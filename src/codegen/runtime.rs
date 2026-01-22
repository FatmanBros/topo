//! Runtime code generation
//!
//! Generates JavaScript runtime code for topo applications.

use super::JsCodegen;

impl JsCodegen {
    pub(super) fn emit_runtime_imports(&mut self) {
        // Inline minimal runtime
        self.emit_line("// topo runtime");
        self.emit_line("const stores = new Map();");
        self.emit_line("");
        self.emit_devtools_runtime();
        self.emit_line("");
        self.emit_animation_runtime();
        self.emit_line("");
        self.emit_runtime_validators();
        self.emit_line("");
        self.emit_runtime_pipes();
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
        self.emit_line("      const prevState = { ...state };");
        self.emit_line("      const reducer = reducers.get(action);");
        self.emit_line("      if (reducer) {");
        self.emit_line("        Object.assign(state, reducer(state, ...args));");
        self.emit_line("        listeners.forEach(fn => fn(state));");
        self.emit_line("      }");
        self.emit_line("      __devtools.log(name, action, args, prevState, state);");
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
        self.emit_line("  let lastLifecycleHooks = { init: [], destroy: [] };");
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
        self.emit_line("    // Update document title on page change");
        self.emit_line("    if (pageChanged && vdom) {");
        self.emit_line("      if (vdom.title) {");
        self.emit_line("        const titleText = resolveText(vdom.title);");
        self.emit_line("        document.title = __defaultTitle ? `${titleText} | ${__defaultTitle}` : titleText;");
        self.emit_line("      } else if (__defaultTitle) {");
        self.emit_line("        document.title = __defaultTitle;");
        self.emit_line("      }");
        self.emit_line("    }");
        self.emit_line("    // Collect lifecycle hooks from all components in the VDOM tree");
        self.emit_line("    const currentHooks = collectLifecycleHooks(vdom);");
        self.emit_line("    // Call lifecycle hooks on page change");
        self.emit_line("    if (pageChanged) {");
        self.emit_line("      // Call destroy hooks from previous page");
        self.emit_line("      lastLifecycleHooks.destroy.forEach(fn => { try { fn(); } catch(e) { console.error('Lifecycle destroy error:', e); } });");
        self.emit_line("      // Call init hooks from current page");
        self.emit_line("      currentHooks.init.forEach(fn => { try { fn(); } catch(e) { console.error('Lifecycle init error:', e); } });");
        self.emit_line("      // Update tracked hooks");
        self.emit_line("      lastLifecycleHooks = currentHooks;");
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
        self.emit_line("// Escape HTML special characters (for text content)");
        self.emit_line("function escapeHtml(str) {");
        self.emit_line("  if (str == null) return '';");
        self.emit_line("  return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/\"/g, '&quot;').replace(/'/g, '&#39;');");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("// Escape attribute values to prevent attribute injection (XSS)");
        self.emit_line("function escapeAttr(str) {");
        self.emit_line("  if (str == null) return '';");
        self.emit_line("  // Remove any characters that could break out of attribute context");
        self.emit_line("  return String(str).replace(/[\"'<>&]/g, c => ({ '\"': '&quot;', \"'\": '&#39;', '<': '&lt;', '>': '&gt;', '&': '&amp;' }[c]));");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function renderVdom(vdom) {");
        self.emit_line("  if (!vdom) return '';");
        self.emit_line("  const { type, content, value, style, children, align, inputType, placeholder, dataError, dataBind, dataField, options, rows, id } = vdom;");
        self.emit_line("  const resolvedContent = resolveText(content);");
        self.emit_line("  const resolvedPlaceholder = resolveText(placeholder);");
        self.emit_line("  const styleAttr = style ? ` class=\"${escapeAttr(style)}\"` : '';");
        self.emit_line("  const idAttr = id ? ` id=\"${escapeAttr(id)}\"` : '';");
        self.emit_line("  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';");
        self.emit_line("  const dataErrorAttr = dataError ? ` data-error=\"${escapeAttr(dataError)}\"` : '';");
        self.emit_line("  const dataBindAttr = dataBind ? ` data-bind=\"${escapeAttr(dataBind)}\"` : '';");
        self.emit_line("  const dataFieldAttr = dataField ? ` data-field=\"${escapeAttr(dataField)}\"` : '';");
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
        self.emit_line("    if (children) {");
        self.emit_line("      const childArr = Array.isArray(children) ? children : [children];");
        self.emit_line("      const inner = childArr.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');");
        self.emit_line("      return `<a href=\"${href}\"${styleAttr} data-link=\"true\">${inner}</a>`;");
        self.emit_line("    }");
        self.emit_line("    return `<a href=\"${href}\"${styleAttr} data-link=\"true\">${escapeHtml(resolvedContent || '')}</a>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'input') {");
        self.emit_line("    const inputTypeAttr = escapeAttr(inputType || 'text');");
        self.emit_line("    const placeholderAttr = resolvedPlaceholder ? ` placeholder=\"${escapeAttr(resolvedPlaceholder)}\"` : '';");
        self.emit_line("    const valueAttr = value !== undefined ? ` value=\"${escapeAttr(value)}\"` : '';");
        self.emit_line("    return `<input type=\"${inputTypeAttr}\"${styleAttr}${placeholderAttr}${valueAttr}${dataFieldAttr} data-input=\"true\" />`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'select') {");
        self.emit_line("    const placeholderOpt = resolvedPlaceholder ? `<option value=\"\" disabled selected>${escapeHtml(resolvedPlaceholder)}</option>` : '';");
        self.emit_line("    const opts = (options || []).map(o => {");
        self.emit_line("      const optVal = typeof o === 'object' ? o.value : o;");
        self.emit_line("      const optLabel = typeof o === 'object' ? resolveText(o.label) : o;");
        self.emit_line("      const selected = optVal === value ? ' selected' : '';");
        self.emit_line("      return `<option value=\"${escapeAttr(optVal)}\"${selected}>${escapeHtml(optLabel)}</option>`;");
        self.emit_line("    }).join('');");
        self.emit_line("    return `<select${styleAttr}${dataFieldAttr} data-input=\"true\">${placeholderOpt}${opts}</select>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'textarea') {");
        self.emit_line("    const placeholderAttr = resolvedPlaceholder ? ` placeholder=\"${escapeAttr(resolvedPlaceholder)}\"` : '';");
        self.emit_line("    const rowsAttr = rows ? ` rows=\"${parseInt(rows, 10) || 3}\"` : '';");
        self.emit_line("    return `<textarea${styleAttr}${placeholderAttr}${rowsAttr}${dataFieldAttr} data-input=\"true\">${escapeHtml(value || '')}</textarea>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'formfield') {");
        self.emit_line("    const { label, inputType, placeholder, value, errorMsg, dataError, dataField } = vdom;");
        self.emit_line("    const labelText = resolveText(label);");
        self.emit_line("    const placeholderText = resolveText(placeholder);");
        self.emit_line("    const inputTypeAttr = escapeAttr(inputType || 'text');");
        self.emit_line("    const placeholderAttr = placeholderText ? ` placeholder=\"${escapeAttr(placeholderText)}\"` : '';");
        self.emit_line("    const valueAttr = value !== undefined ? ` value=\"${escapeAttr(value)}\"` : '';");
        self.emit_line("    const dataFieldAttr = dataField ? ` data-field=\"${escapeAttr(dataField)}\"` : '';");
        self.emit_line("    const dataErrorAttr = dataError ? ` data-error=\"${escapeAttr(dataError)}\"` : '';");
        self.emit_line("    return `<div class=\"mb-4 flex flex-col\"><label class=\"block text-sm font-medium text-gray-700 mb-2\">${escapeHtml(labelText || '')}</label><input type=\"${inputTypeAttr}\" class=\"w-full px-4 py-3 border border-gray-300 rounded-lg focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500 transition\"${placeholderAttr}${valueAttr}${dataFieldAttr} data-input=\"true\" /><span class=\"text-red-500 text-sm mt-1\"${dataErrorAttr}>${escapeHtml(errorMsg || '')}</span></div>`;");
        self.emit_line("  }");
        self.emit_line("  if (type === 'form') {");
        self.emit_line("    const inner = (children || []).map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');");
        self.emit_line("    return `<form${styleAttr} class=\"${escapeAttr((style || '') + flexClass)}\" data-form=\"true\">${inner}</form>`;");
        self.emit_line("  }");
        self.emit_line("  if (children) {");
        self.emit_line("    const childArr = Array.isArray(children) ? children : [children];");
        self.emit_line("    const inner = childArr.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');");
        self.emit_line("    return `<div${idAttr} class=\"${escapeAttr((style || '') + flexClass)}\">${inner}</div>`;");
        self.emit_line("  }");
        self.emit_line("  return `<div${idAttr}${styleAttr}>${escapeHtml(resolvedContent != null ? resolvedContent : (value != null ? value : ''))}</div>`;");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("function bindEvents(el, vdom) {");
        self.emit_line("  el.querySelectorAll('[data-click]').forEach((btn, i) => {");
        self.emit_line("    const handler = findClickHandler(vdom, i);");
        self.emit_line("    if (handler) btn.onclick = handler;");
        self.emit_line("    // Keyboard events for buttons");
        self.emit_line("    const keydownHandler = findEventHandler(vdom, 'keydown', i);");
        self.emit_line("    if (keydownHandler) btn.onkeydown = keydownHandler;");
        self.emit_line("    const keyupHandler = findEventHandler(vdom, 'keyup', i);");
        self.emit_line("    if (keyupHandler) btn.onkeyup = keyupHandler;");
        self.emit_line("    // Mouse events for buttons");
        self.emit_line("    const mouseenterHandler = findEventHandler(vdom, 'mouseenter', i);");
        self.emit_line("    if (mouseenterHandler) btn.onmouseenter = mouseenterHandler;");
        self.emit_line("    const mouseleaveHandler = findEventHandler(vdom, 'mouseleave', i);");
        self.emit_line("    if (mouseleaveHandler) btn.onmouseleave = mouseleaveHandler;");
        self.emit_line("  });");
        self.emit_line("  el.querySelectorAll('[data-input]').forEach((input, i) => {");
        self.emit_line("    const handler = findInputHandler(vdom, i);");
        self.emit_line("    if (handler) {");
        self.emit_line("      // Use 'input' event for text inputs and textareas, 'change' for selects");
        self.emit_line("      if (input.tagName === 'SELECT') {");
        self.emit_line("        input.onchange = (e) => handler(e.target.value);");
        self.emit_line("      } else {");
        self.emit_line("        // Handle IME composition for CJK input");
        self.emit_line("        let isComposing = false;");
        self.emit_line("        input.addEventListener('compositionstart', () => isComposing = true);");
        self.emit_line("        input.addEventListener('compositionend', (e) => {");
        self.emit_line("          isComposing = false;");
        self.emit_line("          handler(e.target.value);");
        self.emit_line("        });");
        self.emit_line("        input.oninput = (e) => {");
        self.emit_line("          if (!isComposing) handler(e.target.value);");
        self.emit_line("        };");
        self.emit_line("      }");
        self.emit_line("    }");
        self.emit_line("    // Keyboard events for inputs");
        self.emit_line("    const keydownHandler = findEventHandler(vdom, 'keydown', i);");
        self.emit_line("    if (keydownHandler) input.onkeydown = keydownHandler;");
        self.emit_line("    const keyupHandler = findEventHandler(vdom, 'keyup', i);");
        self.emit_line("    if (keyupHandler) input.onkeyup = keyupHandler;");
        self.emit_line("    const keypressHandler = findEventHandler(vdom, 'keypress', i);");
        self.emit_line("    if (keypressHandler) input.onkeypress = keypressHandler;");
        self.emit_line("    // Focus events for inputs");
        self.emit_line("    const focusHandler = findEventHandler(vdom, 'focus', i);");
        self.emit_line("    if (focusHandler) input.onfocus = focusHandler;");
        self.emit_line("    const blurHandler = findEventHandler(vdom, 'blur', i);");
        self.emit_line("    if (blurHandler) input.onblur = blurHandler;");
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
        self.emit_line("// Generic event handler finder for keyboard, focus, mouse events");
        self.emit_line("function findEventHandler(vdom, eventName, index, count = { n: 0 }) {");
        self.emit_line("  if (!vdom) return null;");
        self.emit_line("  // Check both camelCase (onKeydown) and lowercase (keydown) forms");
        self.emit_line("  const camelName = 'on' + eventName.charAt(0).toUpperCase() + eventName.slice(1);");
        self.emit_line("  const handler = vdom[eventName] || vdom[camelName];");
        self.emit_line("  // Only count elements that could have this event (buttons for click-related, inputs for input-related)");
        self.emit_line("  const isClickElement = vdom.type === 'button' || vdom.type === 'submit' || vdom.click;");
        self.emit_line("  const isInputElement = vdom.type === 'input' || vdom.type === 'textarea' || vdom.type === 'select' || vdom.input || vdom.onInput;");
        self.emit_line("  const shouldCount = isClickElement || isInputElement;");
        self.emit_line("  if (handler && shouldCount && count.n++ === index) return handler;");
        self.emit_line("  for (const c of normalizeChildren(vdom.children)) {");
        self.emit_line("    const child = typeof c === 'function' ? c() : c;");
        self.emit_line("    const h = findEventHandler(child, eventName, index, count);");
        self.emit_line("    if (h) return h;");
        self.emit_line("  }");
        self.emit_line("  return null;");
        self.emit_line("}");
        self.emit_line("");

        // Lifecycle hooks collection
        self.emit_line("// Collect all lifecycle hooks from VDOM tree");
        self.emit_line("function collectLifecycleHooks(vdom, hooks = { init: [], destroy: [] }) {");
        self.emit_line("  if (!vdom) return hooks;");
        self.emit_line("  if (vdom.lifecycle) {");
        self.emit_line("    if (vdom.lifecycle.init) hooks.init.push(vdom.lifecycle.init);");
        self.emit_line("    if (vdom.lifecycle.destroy) hooks.destroy.push(vdom.lifecycle.destroy);");
        self.emit_line("  }");
        self.emit_line("  for (const c of normalizeChildren(vdom.children)) {");
        self.emit_line("    const child = typeof c === 'function' ? c() : c;");
        self.emit_line("    collectLifecycleHooks(child, hooks);");
        self.emit_line("  }");
        self.emit_line("  return hooks;");
        self.emit_line("}");
        self.emit_line("");

        // Router runtime
        self.emit_router_runtime();
    }

    pub(super) fn emit_router_runtime(&mut self) {
        self.emit_line("// Router");
        self.emit_line("const __basePath = window.__TOPO_BASE_PATH || '';");
        self.emit_line("const __defaultTitle = window.__TOPO_DEFAULT_TITLE || '';");
        self.emit_line("const routeState = { path: '/', params: {}, query: {} };");
        self.emit_line("const routes = [];");
        self.emit_line("let currentPage = null;");
        self.emit_line("let __rerender = () => {};");
        self.emit_line("");

        // Route registration
        self.emit_line("function registerRoute(pattern, component, meta = null) {");
        self.emit_line("  const paramNames = [];");
        self.emit_line("  const regexPattern = pattern.replace(/\\[([^\\]]+)\\]/g, (_, name) => {");
        self.emit_line("    if (name.startsWith('...')) {");
        self.emit_line("      paramNames.push(name.slice(3));");
        self.emit_line("      return '(.*)';");
        self.emit_line("    }");
        self.emit_line("    paramNames.push(name);");
        self.emit_line("    return '([^/]+)';");
        self.emit_line("  });");
        self.emit_line("  routes.push({ pattern: new RegExp(`^${regexPattern}$`), paramNames, component, meta });");
        self.emit_line("}");
        self.emit_line("");

        // Route matching
        self.emit_line("function matchRoute(path) {");
        self.emit_line("  for (const route of routes) {");
        self.emit_line("    const match = path.match(route.pattern);");
        self.emit_line("    if (match) {");
        self.emit_line("      const params = {};");
        self.emit_line("      route.paramNames.forEach((name, i) => { params[name] = match[i + 1]; });");
        self.emit_line("      return { component: route.component, params, meta: route.meta };");
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
        self.emit_line("  history.pushState(null, '', __basePath + path);");
        self.emit_line("  updateRoute();");
        self.emit_line("  __rerender();");
        self.emit_line("}");
        self.emit_line("");

        // Update route
        self.emit_line("function updateRoute() {");
        self.emit_line("  let path = location.pathname;");
        self.emit_line("  if (__basePath && path.startsWith(__basePath)) {");
        self.emit_line("    path = path.slice(__basePath.length) || '/';");
        self.emit_line("  }");
        self.emit_line("  // Normalize trailing slash (except root)");
        self.emit_line("  if (path !== '/' && path.endsWith('/')) {");
        self.emit_line("    path = path.slice(0, -1);");
        self.emit_line("  }");
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
        self.emit_line("  // Apply route metadata from __routeMeta or matched.meta");
        self.emit_line("  const meta = (typeof __routeMeta !== 'undefined' && __routeMeta[path]) || (matched && matched.meta);");
        self.emit_line("  if (meta && meta.title) {");
        self.emit_line("    document.title = __defaultTitle ? `${meta.title} | ${__defaultTitle}` : meta.title;");
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

        // Initialize router on load (browser only)
        self.emit_line("if (typeof window !== 'undefined') {");
        self.emit_line("  window.addEventListener('popstate', updateRoute);");
        self.emit_line("  document.addEventListener('DOMContentLoaded', updateRoute);");
        self.emit_line("}");
        self.emit_line("");

        // SSR support - export renderPage for server-side rendering
        self.emit_line("// SSR support");
        self.emit_line("const __components = new Map();");
        self.emit_line("function registerComponent(name, fn) { __components.set(name, fn); }");
        self.emit_line("");
        self.emit_line("async function renderPage(componentName, params = {}) {");
        self.emit_line("  // Set route params for the component");
        self.emit_line("  routeState.params = params;");
        self.emit_line("  routeState.path = '/';");
        self.emit_line("");
        self.emit_line("  // Find the component");
        self.emit_line("  const component = __components.get(componentName);");
        self.emit_line("  if (!component) {");
        self.emit_line("    return { content: '', title: null };");
        self.emit_line("  }");
        self.emit_line("");
        self.emit_line("  try {");
        self.emit_line("    // Execute component to get vdom");
        self.emit_line("    const vdom = typeof component === 'function' ? component() : component;");
        self.emit_line("    const content = renderVdom(vdom);");
        self.emit_line("    const title = vdom && vdom.title ? resolveText(vdom.title) : null;");
        self.emit_line("    return { content, title };");
        self.emit_line("  } catch (e) {");
        self.emit_line("    console.error('SSR render error:', e);");
        self.emit_line("    return { content: '', title: null };");
        self.emit_line("  }");
        self.emit_line("}");
        self.emit_line("");
        // Use UMD-style conditional export to work in both browser and Node.js
        self.emit_line("// Export for SSR (Node.js) - browser script context ignores this");
        self.emit_line("if (typeof module !== 'undefined' && module.exports) {");
        self.emit_line("  module.exports = { renderPage };");
        self.emit_line("}");
    }

    pub(super) fn emit_runtime_validators(&mut self) {
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

    pub(super) fn emit_runtime_pipes(&mut self) {
        self.emit_line("// Built-in Pipes");
        self.emit_line("const __pipes = {");
        self.indent += 1;

        // uppercase - Convert string to uppercase
        self.emit_line("uppercase(value) {");
        self.emit_line("  return value != null ? String(value).toUpperCase() : '';");
        self.emit_line("},");

        // lowercase - Convert string to lowercase
        self.emit_line("lowercase(value) {");
        self.emit_line("  return value != null ? String(value).toLowerCase() : '';");
        self.emit_line("},");

        // capitalize - Capitalize first letter
        self.emit_line("capitalize(value) {");
        self.emit_line("  if (value == null) return '';");
        self.emit_line("  const s = String(value);");
        self.emit_line("  return s.charAt(0).toUpperCase() + s.slice(1);");
        self.emit_line("},");

        // titlecase - Capitalize first letter of each word
        self.emit_line("titlecase(value) {");
        self.emit_line("  if (value == null) return '';");
        self.emit_line("  return String(value).replace(/\\b\\w/g, c => c.toUpperCase());");
        self.emit_line("},");

        // trim - Remove whitespace from both ends
        self.emit_line("trim(value) {");
        self.emit_line("  return value != null ? String(value).trim() : '';");
        self.emit_line("},");

        // number - Format number with locale
        self.emit_line("number(value, locale = 'en-US') {");
        self.emit_line("  if (value == null) return '';");
        self.emit_line("  const num = Number(value);");
        self.emit_line("  return isNaN(num) ? '' : num.toLocaleString(locale);");
        self.emit_line("},");

        // currency - Format as currency
        self.emit_line("currency(value, currency = 'USD', locale = 'en-US') {");
        self.emit_line("  if (value == null) return '';");
        self.emit_line("  const num = Number(value);");
        self.emit_line("  if (isNaN(num)) return '';");
        self.emit_line("  return new Intl.NumberFormat(locale, { style: 'currency', currency }).format(num);");
        self.emit_line("},");

        // percent - Format as percentage
        self.emit_line("percent(value, decimals = 0, locale = 'en-US') {");
        self.emit_line("  if (value == null) return '';");
        self.emit_line("  const num = Number(value);");
        self.emit_line("  if (isNaN(num)) return '';");
        self.emit_line("  return new Intl.NumberFormat(locale, { style: 'percent', minimumFractionDigits: decimals, maximumFractionDigits: decimals }).format(num);");
        self.emit_line("},");

        // date - Format date
        self.emit_line("date(value, format = 'short', locale = 'en-US') {");
        self.emit_line("  if (value == null) return '';");
        self.emit_line("  const d = value instanceof Date ? value : new Date(value);");
        self.emit_line("  if (isNaN(d.getTime())) return '';");
        self.emit_line("  const options = format === 'short' ? { dateStyle: 'short' } :");
        self.emit_line("                  format === 'medium' ? { dateStyle: 'medium' } :");
        self.emit_line("                  format === 'long' ? { dateStyle: 'long' } :");
        self.emit_line("                  format === 'full' ? { dateStyle: 'full' } : { dateStyle: 'short' };");
        self.emit_line("  return new Intl.DateTimeFormat(locale, options).format(d);");
        self.emit_line("},");

        // time - Format time
        self.emit_line("time(value, format = 'short', locale = 'en-US') {");
        self.emit_line("  if (value == null) return '';");
        self.emit_line("  const d = value instanceof Date ? value : new Date(value);");
        self.emit_line("  if (isNaN(d.getTime())) return '';");
        self.emit_line("  const options = format === 'short' ? { timeStyle: 'short' } :");
        self.emit_line("                  format === 'medium' ? { timeStyle: 'medium' } :");
        self.emit_line("                  format === 'long' ? { timeStyle: 'long' } : { timeStyle: 'short' };");
        self.emit_line("  return new Intl.DateTimeFormat(locale, options).format(d);");
        self.emit_line("},");

        // datetime - Format date and time
        self.emit_line("datetime(value, dateFormat = 'short', timeFormat = 'short', locale = 'en-US') {");
        self.emit_line("  if (value == null) return '';");
        self.emit_line("  const d = value instanceof Date ? value : new Date(value);");
        self.emit_line("  if (isNaN(d.getTime())) return '';");
        self.emit_line("  return new Intl.DateTimeFormat(locale, { dateStyle: dateFormat, timeStyle: timeFormat }).format(d);");
        self.emit_line("},");

        // relative - Relative time (e.g., "2 days ago")
        self.emit_line("relative(value, locale = 'en-US') {");
        self.emit_line("  if (value == null) return '';");
        self.emit_line("  const d = value instanceof Date ? value : new Date(value);");
        self.emit_line("  if (isNaN(d.getTime())) return '';");
        self.emit_line("  const diff = (d - new Date()) / 1000;");
        self.emit_line("  const units = [");
        self.emit_line("    ['year', 31536000], ['month', 2592000], ['week', 604800],");
        self.emit_line("    ['day', 86400], ['hour', 3600], ['minute', 60], ['second', 1]");
        self.emit_line("  ];");
        self.emit_line("  for (const [unit, secs] of units) {");
        self.emit_line("    if (Math.abs(diff) >= secs) {");
        self.emit_line("      return new Intl.RelativeTimeFormat(locale).format(Math.round(diff / secs), unit);");
        self.emit_line("    }");
        self.emit_line("  }");
        self.emit_line("  return 'just now';");
        self.emit_line("},");

        // slice - Slice string or array
        self.emit_line("slice(value, start, end) {");
        self.emit_line("  if (value == null) return '';");
        self.emit_line("  return (Array.isArray(value) ? value : String(value)).slice(start, end);");
        self.emit_line("},");

        // truncate - Truncate with ellipsis
        self.emit_line("truncate(value, maxLength = 50, suffix = '...') {");
        self.emit_line("  if (value == null) return '';");
        self.emit_line("  const s = String(value);");
        self.emit_line("  return s.length > maxLength ? s.slice(0, maxLength - suffix.length) + suffix : s;");
        self.emit_line("},");

        // replace - Replace text
        self.emit_line("replace(value, search, replacement = '') {");
        self.emit_line("  if (value == null) return '';");
        self.emit_line("  return String(value).replace(new RegExp(search, 'g'), replacement);");
        self.emit_line("},");

        // json - Convert to JSON string
        self.emit_line("json(value, indent = 0) {");
        self.emit_line("  try { return JSON.stringify(value, null, indent); } catch { return ''; }");
        self.emit_line("},");

        // default - Provide default value if null/undefined
        self.emit_line("default(value, defaultValue = '') {");
        self.emit_line("  return value != null ? value : defaultValue;");
        self.emit_line("},");

        // join - Join array elements
        self.emit_line("join(value, separator = ', ') {");
        self.emit_line("  return Array.isArray(value) ? value.join(separator) : String(value);");
        self.emit_line("},");

        // reverse - Reverse string or array
        self.emit_line("reverse(value) {");
        self.emit_line("  if (Array.isArray(value)) return [...value].reverse();");
        self.emit_line("  return value != null ? String(value).split('').reverse().join('') : '';");
        self.emit_line("},");

        // padStart - Pad string at start
        self.emit_line("padStart(value, length, char = ' ') {");
        self.emit_line("  return value != null ? String(value).padStart(length, char) : '';");
        self.emit_line("},");

        // padEnd - Pad string at end
        self.emit_line("padEnd(value, length, char = ' ') {");
        self.emit_line("  return value != null ? String(value).padEnd(length, char) : '';");
        self.emit_line("},");

        self.indent -= 1;
        self.emit_line("};");
    }

    pub(super) fn emit_devtools_runtime(&mut self) {
        self.emit_line("// Store DevTools (dev mode only)");
        self.emit_line("const __devtools = {");
        self.indent += 1;
        self.emit_line("enabled: false,");
        self.emit_line("history: [],");
        self.emit_line("maxHistory: 50,");
        self.emit_line("");

        // getStores - get all store states
        self.emit_line("getStores() {");
        self.emit_line("  const result = {};");
        self.emit_line("  stores.forEach((store, name) => {");
        self.emit_line("    result[name] = { ...store.state };");
        self.emit_line("  });");
        self.emit_line("  return result;");
        self.emit_line("},");
        self.emit_line("");

        // getStore - get specific store state
        self.emit_line("getStore(name) {");
        self.emit_line("  const store = stores.get(name);");
        self.emit_line("  return store ? { ...store.state } : null;");
        self.emit_line("},");
        self.emit_line("");

        // log - log action with state change
        self.emit_line("log(storeName, action, args, prevState, nextState) {");
        self.emit_line("  if (!this.enabled) return;");
        self.emit_line("  const entry = {");
        self.emit_line("    timestamp: new Date().toISOString(),");
        self.emit_line("    store: storeName,");
        self.emit_line("    action,");
        self.emit_line("    args,");
        self.emit_line("    prevState: { ...prevState },");
        self.emit_line("    nextState: { ...nextState },");
        self.emit_line("    diff: this._diff(prevState, nextState)");
        self.emit_line("  };");
        self.emit_line("  this.history.push(entry);");
        self.emit_line("  if (this.history.length > this.maxHistory) this.history.shift();");
        self.emit_line("  console.groupCollapsed(`%c[${storeName}] ${action}`, 'color: #8b5cf6; font-weight: bold');");
        self.emit_line("  console.log('%cprev state', 'color: #9ca3af', prevState);");
        self.emit_line("  console.log('%caction', 'color: #3b82f6', { type: action, payload: args });");
        self.emit_line("  console.log('%cnext state', 'color: #22c55e', nextState);");
        self.emit_line("  if (entry.diff.length > 0) console.log('%cdiff', 'color: #f59e0b', entry.diff);");
        self.emit_line("  console.groupEnd();");
        self.emit_line("},");
        self.emit_line("");

        // _diff - compute state diff
        self.emit_line("_diff(prev, next) {");
        self.emit_line("  const changes = [];");
        self.emit_line("  const allKeys = new Set([...Object.keys(prev), ...Object.keys(next)]);");
        self.emit_line("  allKeys.forEach(key => {");
        self.emit_line("    if (JSON.stringify(prev[key]) !== JSON.stringify(next[key])) {");
        self.emit_line("      changes.push({ key, from: prev[key], to: next[key] });");
        self.emit_line("    }");
        self.emit_line("  });");
        self.emit_line("  return changes;");
        self.emit_line("},");
        self.emit_line("");

        // getHistory - get action history
        self.emit_line("getHistory() { return [...this.history]; },");
        self.emit_line("");

        // clearHistory
        self.emit_line("clearHistory() { this.history = []; },");
        self.emit_line("");

        // enable/disable
        self.emit_line("enable() { this.enabled = true; console.log('%c[topo] DevTools enabled', 'color: #22c55e'); },");
        self.emit_line("disable() { this.enabled = false; console.log('%c[topo] DevTools disabled', 'color: #9ca3af'); }");

        self.indent -= 1;
        self.emit_line("};");
        self.emit_line("");

        // Expose to window
        self.emit_line("if (typeof window !== 'undefined') {");
        self.emit_line("  window.__TOPO__ = __devtools;");
        self.emit_line("}");
    }

    pub(super) fn emit_animation_runtime(&mut self) {
        self.emit_line("// Animation runtime");
        self.emit_line("const __animations = new Map();");
        self.emit_line("");

        // __animate function - uses Web Animations API for one-shot animations
        self.emit_line("async function __animate(element, animationName, overrides = {}) {");
        self.indent += 1;
        self.emit_line("const anim = __animations.get(animationName);");
        self.emit_line("if (!anim || !element) return;");
        self.emit_line("");
        self.emit_line("function parseDuration(dur) {");
        self.emit_line("  if (typeof dur === 'number') return dur;");
        self.emit_line("  if (dur.endsWith('ms')) return parseFloat(dur);");
        self.emit_line("  if (dur.endsWith('s')) return parseFloat(dur) * 1000;");
        self.emit_line("  return parseFloat(dur);");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("const options = {");
        self.emit_line("  duration: parseDuration(overrides.duration || anim.duration),");
        self.emit_line("  easing: overrides.easing || anim.easing || 'ease',");
        self.emit_line("  fill: anim.fill || 'forwards'");
        self.emit_line("};");
        self.emit_line("");
        self.emit_line("const animation = element.animate(anim.keyframes, options);");
        self.emit_line("return animation.finished;");
        self.indent -= 1;
        self.emit_line("}");
        self.emit_line("");

        // __applyAnimation function - uses CSS @keyframes for persistent animations
        self.emit_line("function __applyAnimation(element, animationName) {");
        self.indent += 1;
        self.emit_line("const anim = __animations.get(animationName);");
        self.emit_line("if (!anim || !element) return;");
        self.emit_line("");
        self.emit_line("const cssName = `topo-anim-${animationName}`;");
        self.emit_line("");
        self.emit_line("// Check if keyframes already exist");
        self.emit_line("if (!document.querySelector(`style[data-anim=\"${cssName}\"]`)) {");
        self.indent += 1;
        self.emit_line("// Generate CSS keyframes");
        self.emit_line("let keyframesCSS = `@keyframes ${cssName} {\\n`;");
        self.emit_line("");
        self.emit_line("anim.keyframes.forEach((kf, i) => {");
        self.emit_line("  const percent = kf.offset !== undefined ? kf.offset * 100 : (i / (anim.keyframes.length - 1)) * 100;");
        self.emit_line("  keyframesCSS += `  ${percent}% {\\n`;");
        self.emit_line("  Object.entries(kf).forEach(([prop, val]) => {");
        self.emit_line("    if (prop !== 'offset' && prop !== 'easing') {");
        self.emit_line("      // Convert camelCase to kebab-case");
        self.emit_line("      const cssProp = prop.replace(/([A-Z])/g, '-$1').toLowerCase();");
        self.emit_line("      keyframesCSS += `    ${cssProp}: ${val};\\n`;");
        self.emit_line("    }");
        self.emit_line("  });");
        self.emit_line("  keyframesCSS += `  }\\n`;");
        self.emit_line("});");
        self.emit_line("");
        self.emit_line("keyframesCSS += '}';");
        self.emit_line("");
        self.emit_line("// Inject stylesheet");
        self.emit_line("const style = document.createElement('style');");
        self.emit_line("style.setAttribute('data-anim', cssName);");
        self.emit_line("style.textContent = keyframesCSS;");
        self.emit_line("document.head.appendChild(style);");
        self.indent -= 1;
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("// Apply animation to element");
        self.emit_line("const duration = anim.duration || '300ms';");
        self.emit_line("const easing = anim.easing || 'ease';");
        self.emit_line("const fill = anim.fill || 'forwards';");
        self.emit_line("const iteration = anim.iteration || 'infinite';");
        self.emit_line("element.style.animation = `${cssName} ${duration} ${easing} ${iteration} ${fill}`;");
        self.indent -= 1;
        self.emit_line("}");
    }

    /// Generate icon data for used icons only (tree-shaking)
    pub fn emit_icon_data(&mut self) {
        if self.used_icons.is_empty() {
            return;
        }

        // Collect icon data first to avoid borrow conflict
        let icon_data = super::icons::get_icon_data();
        let icon_entries: Vec<(String, String)> = self.used_icons
            .iter()
            .filter_map(|name| {
                icon_data.get(name.as_str()).map(|path| (name.clone(), path.to_string()))
            })
            .collect();

        self.emit_line("// Icon data (tree-shaken)");
        self.emit_line("const __icons = {");
        self.indent += 1;

        for (icon_name, svg_path) in &icon_entries {
            self.emit_line(&format!("'{}': `{}`,", icon_name, svg_path));
        }

        self.indent -= 1;
        self.emit_line("};");
        self.emit_line("");

        // Icon component function
        self.emit_line("function Icon(props) {");
        self.indent += 1;
        self.emit_line("const name = props.name;");
        self.emit_line("const size = props.size || 24;");
        self.emit_line("const color = props.color || 'currentColor';");
        self.emit_line("const strokeWidth = props.strokeWidth || 2;");
        self.emit_line("const className = props.class || props.style || '';");
        self.emit_line("");
        self.emit_line("const path = __icons[name];");
        self.emit_line("if (!path) {");
        self.emit_line("  console.warn(`Icon \"${name}\" not found`);");
        self.emit_line("  return { type: 'span', props: { content: `[${name}]` } };");
        self.emit_line("}");
        self.emit_line("");
        self.emit_line("return {");
        self.emit_line("  type: 'svg',");
        self.emit_line("  props: {");
        self.emit_line("    attr: {");
        self.emit_line("      xmlns: 'http://www.w3.org/2000/svg',");
        self.emit_line("      width: size,");
        self.emit_line("      height: size,");
        self.emit_line("      viewBox: '0 0 24 24',");
        self.emit_line("      fill: 'none',");
        self.emit_line("      stroke: color,");
        self.emit_line("      'stroke-width': strokeWidth,");
        self.emit_line("      'stroke-linecap': 'round',");
        self.emit_line("      'stroke-linejoin': 'round',");
        self.emit_line("      class: className");
        self.emit_line("    },");
        self.emit_line("    innerHTML: path");
        self.emit_line("  }");
        self.emit_line("};");
        self.indent -= 1;
        self.emit_line("}");
        self.emit_line("");
    }

    /// Track an icon usage for tree-shaking
    pub fn track_icon(&mut self, name: &str) {
        self.used_icons.insert(name.to_string());
    }

    /// Get the set of used icons
    pub fn get_used_icons(&self) -> &std::collections::HashSet<String> {
        &self.used_icons
    }
}
