// topo runtime
const stores = new Map();

// Validators
const validators = {
  required(value, _args, field) {
    if (value === null || value === undefined || value === '') {
      return { valid: false, error: `${field} is required` };
    }
    return { valid: true };
  },
  min(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    if (typeof value === 'number' && value < min) {
      return { valid: false, error: `${field} must be at least ${min}` };
    }
    return { valid: true };
  },
  max(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    if (typeof value === 'number' && value > max) {
      return { valid: false, error: `${field} must be at most ${max}` };
    }
    return { valid: true };
  },
  minLength(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    return { valid: true };
  },
  maxLength(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    return { valid: true };
  },
  email(value, _args, field) {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (typeof value === 'string' && !emailRegex.test(value)) {
      return { valid: false, error: `${field} must be a valid email address` };
    }
    return { valid: true };
  },
  pattern(value, args, field) {
    const pattern = new RegExp(args[0]);
    if (typeof value === 'string' && !pattern.test(value)) {
      return { valid: false, error: `${field} does not match the required pattern` };
    }
    return { valid: true };
  },
  url(value, _args, field) {
    try {
      new URL(value);
      return { valid: true };
    } catch {
      return { valid: false, error: `${field} must be a valid URL` };
    }
  },
  alphanumeric(value, _args, field) {
    if (typeof value === 'string' && !/^[a-zA-Z0-9]+$/.test(value)) {
      return { valid: false, error: `${field} must contain only letters and numbers` };
    }
    return { valid: true };
  },
  range(value, args, field) {
    const [min, max] = args;
    if (typeof value === 'number' && (value < min || value > max)) {
      return { valid: false, error: `${field} must be between ${min} and ${max}` };
    }
    return { valid: true };
  },
};

function validate(data, rules) {
  const errors = {};
  for (const [field, fieldRules] of Object.entries(rules)) {
    const value = data[field];
    for (const rule of fieldRules) {
      const validator = validators[rule.name];
      if (validator) {
        const result = validator(value, rule.args || [], field);
        if (!result.valid) {
          errors[field] = errors[field] || [];
          errors[field].push(result.error);
        }
      }
    }
  }
  return { valid: Object.keys(errors).length === 0, errors };
}

function createStore(name, initialState) {
  const state = { ...initialState };
  const listeners = [];
  const reducers = new Map();
  const effects = new Map();
  const selectors = new Map();

  const store = {
    get state() { return state; },
    on(action, reducer) { reducers.set(action, reducer); },
    effect(action, handler) { effects.set(action, handler); },
    selector(name, fn) { selectors.set(name, fn); },
    subscribe(fn) { listeners.push(fn); },
    dispatch(action, ...args) {
      const reducer = reducers.get(action);
      if (reducer) {
        Object.assign(state, reducer(state, ...args));
        listeners.forEach(fn => fn(state));
      }
      const effect = effects.get(action);
      if (effect) effect(...args);
    },
    select(name) {
      const selector = selectors.get(name);
      return selector ? selector(state) : undefined;
    }
  };
  stores.set(name, store);
  return store;
}

function dispatch(storeName, action, ...args) {
  const store = stores.get(storeName);
  if (store) store.dispatch(action, ...args);
}

function mount(componentFn, container) {
  const el = document.querySelector(container);
  if (!el) return;
  const render = () => {
    const vdom = componentFn();
    el.innerHTML = renderVdom(vdom);
    bindEvents(el, vdom);
  };
  stores.forEach(store => store.subscribe(render));
  render();
}

function renderVdom(vdom) {
  if (!vdom) return '';
  const { type, content, value, style, children, align, inputType, placeholder } = vdom;
  const styleAttr = style ? ` class="${style}"` : '';
  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';
  
  if (type === 'text') {
    return `<span${styleAttr}>${content || value || ''}</span>`;
  }
  if (type === 'button') {
    return `<button${styleAttr} data-click="true">${content || ''}</button>`;
  }
  if (type === 'input') {
    const inputTypeAttr = inputType || 'text';
    const placeholderAttr = placeholder ? ` placeholder="${placeholder}"` : '';
    const valueAttr = value !== undefined ? ` value="${value}"` : '';
    return `<input type="${inputTypeAttr}"${styleAttr}${placeholderAttr}${valueAttr} data-input="true" />`;
  }
  if (children) {
    const inner = children.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');
    return `<div class="${(style || '') + flexClass}">${inner}</div>`;
  }
  return `<div${styleAttr}>${content || value || ''}</div>`;
}

function bindEvents(el, vdom) {
  el.querySelectorAll('[data-click]').forEach((btn, i) => {
    const handler = findClickHandler(vdom, i);
    if (handler) btn.onclick = handler;
  });
  el.querySelectorAll('[data-input]').forEach((input, i) => {
    const handler = findInputHandler(vdom, i);
    if (handler) input.oninput = (e) => handler(e.target.value);
  });
}

function findClickHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.click && count.n++ === index) return vdom.click;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findClickHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function findInputHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.input && count.n++ === index) return vdom.input;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findInputHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function Text(content, style) {
  return {
    type: 'text',
    content: content,
    style: style
  };
}

function Heading(content, level) {
  return {
    type: 'text',
    content: content,
    style: level
  };
}

function Code(content) {
  return {
    type: 'text',
    content: content,
    style: 'font-mono text-sm bg-gray-900 text-green-400 p-6 rounded-xl whitespace-pre-wrap'
  };
}

function Badge(content, color) {
  return {
    type: 'text',
    content: content,
    style: color
  };
}


// topo runtime
const stores = new Map();

// Validators
const validators = {
  required(value, _args, field) {
    if (value === null || value === undefined || value === '') {
      return { valid: false, error: `${field} is required` };
    }
    return { valid: true };
  },
  min(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    if (typeof value === 'number' && value < min) {
      return { valid: false, error: `${field} must be at least ${min}` };
    }
    return { valid: true };
  },
  max(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    if (typeof value === 'number' && value > max) {
      return { valid: false, error: `${field} must be at most ${max}` };
    }
    return { valid: true };
  },
  minLength(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    return { valid: true };
  },
  maxLength(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    return { valid: true };
  },
  email(value, _args, field) {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (typeof value === 'string' && !emailRegex.test(value)) {
      return { valid: false, error: `${field} must be a valid email address` };
    }
    return { valid: true };
  },
  pattern(value, args, field) {
    const pattern = new RegExp(args[0]);
    if (typeof value === 'string' && !pattern.test(value)) {
      return { valid: false, error: `${field} does not match the required pattern` };
    }
    return { valid: true };
  },
  url(value, _args, field) {
    try {
      new URL(value);
      return { valid: true };
    } catch {
      return { valid: false, error: `${field} must be a valid URL` };
    }
  },
  alphanumeric(value, _args, field) {
    if (typeof value === 'string' && !/^[a-zA-Z0-9]+$/.test(value)) {
      return { valid: false, error: `${field} must contain only letters and numbers` };
    }
    return { valid: true };
  },
  range(value, args, field) {
    const [min, max] = args;
    if (typeof value === 'number' && (value < min || value > max)) {
      return { valid: false, error: `${field} must be between ${min} and ${max}` };
    }
    return { valid: true };
  },
};

function validate(data, rules) {
  const errors = {};
  for (const [field, fieldRules] of Object.entries(rules)) {
    const value = data[field];
    for (const rule of fieldRules) {
      const validator = validators[rule.name];
      if (validator) {
        const result = validator(value, rule.args || [], field);
        if (!result.valid) {
          errors[field] = errors[field] || [];
          errors[field].push(result.error);
        }
      }
    }
  }
  return { valid: Object.keys(errors).length === 0, errors };
}

function createStore(name, initialState) {
  const state = { ...initialState };
  const listeners = [];
  const reducers = new Map();
  const effects = new Map();
  const selectors = new Map();

  const store = {
    get state() { return state; },
    on(action, reducer) { reducers.set(action, reducer); },
    effect(action, handler) { effects.set(action, handler); },
    selector(name, fn) { selectors.set(name, fn); },
    subscribe(fn) { listeners.push(fn); },
    dispatch(action, ...args) {
      const reducer = reducers.get(action);
      if (reducer) {
        Object.assign(state, reducer(state, ...args));
        listeners.forEach(fn => fn(state));
      }
      const effect = effects.get(action);
      if (effect) effect(...args);
    },
    select(name) {
      const selector = selectors.get(name);
      return selector ? selector(state) : undefined;
    }
  };
  stores.set(name, store);
  return store;
}

function dispatch(storeName, action, ...args) {
  const store = stores.get(storeName);
  if (store) store.dispatch(action, ...args);
}

function mount(componentFn, container) {
  const el = document.querySelector(container);
  if (!el) return;
  const render = () => {
    const vdom = componentFn();
    el.innerHTML = renderVdom(vdom);
    bindEvents(el, vdom);
  };
  stores.forEach(store => store.subscribe(render));
  render();
}

function renderVdom(vdom) {
  if (!vdom) return '';
  const { type, content, value, style, children, align, inputType, placeholder } = vdom;
  const styleAttr = style ? ` class="${style}"` : '';
  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';
  
  if (type === 'text') {
    return `<span${styleAttr}>${content || value || ''}</span>`;
  }
  if (type === 'button') {
    return `<button${styleAttr} data-click="true">${content || ''}</button>`;
  }
  if (type === 'input') {
    const inputTypeAttr = inputType || 'text';
    const placeholderAttr = placeholder ? ` placeholder="${placeholder}"` : '';
    const valueAttr = value !== undefined ? ` value="${value}"` : '';
    return `<input type="${inputTypeAttr}"${styleAttr}${placeholderAttr}${valueAttr} data-input="true" />`;
  }
  if (children) {
    const inner = children.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');
    return `<div class="${(style || '') + flexClass}">${inner}</div>`;
  }
  return `<div${styleAttr}>${content || value || ''}</div>`;
}

function bindEvents(el, vdom) {
  el.querySelectorAll('[data-click]').forEach((btn, i) => {
    const handler = findClickHandler(vdom, i);
    if (handler) btn.onclick = handler;
  });
  el.querySelectorAll('[data-input]').forEach((input, i) => {
    const handler = findInputHandler(vdom, i);
    if (handler) input.oninput = (e) => handler(e.target.value);
  });
}

function findClickHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.click && count.n++ === index) return vdom.click;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findClickHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function findInputHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.input && count.n++ === index) return vdom.input;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findInputHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function Text(content, style) {
  return {
    type: 'text',
    content: content,
    style: style
  };
}

function Heading(content, level) {
  return {
    type: 'text',
    content: content,
    style: level
  };
}

function Code(content) {
  return {
    type: 'text',
    content: content,
    style: 'font-mono text-sm bg-gray-900 text-green-400 p-6 rounded-xl whitespace-pre-wrap'
  };
}

function Badge(content, color) {
  return {
    type: 'text',
    content: content,
    style: color
  };
}

// topo runtime
const stores = new Map();

// Validators
const validators = {
  required(value, _args, field) {
    if (value === null || value === undefined || value === '') {
      return { valid: false, error: `${field} is required` };
    }
    return { valid: true };
  },
  min(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    if (typeof value === 'number' && value < min) {
      return { valid: false, error: `${field} must be at least ${min}` };
    }
    return { valid: true };
  },
  max(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    if (typeof value === 'number' && value > max) {
      return { valid: false, error: `${field} must be at most ${max}` };
    }
    return { valid: true };
  },
  minLength(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    return { valid: true };
  },
  maxLength(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    return { valid: true };
  },
  email(value, _args, field) {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (typeof value === 'string' && !emailRegex.test(value)) {
      return { valid: false, error: `${field} must be a valid email address` };
    }
    return { valid: true };
  },
  pattern(value, args, field) {
    const pattern = new RegExp(args[0]);
    if (typeof value === 'string' && !pattern.test(value)) {
      return { valid: false, error: `${field} does not match the required pattern` };
    }
    return { valid: true };
  },
  url(value, _args, field) {
    try {
      new URL(value);
      return { valid: true };
    } catch {
      return { valid: false, error: `${field} must be a valid URL` };
    }
  },
  alphanumeric(value, _args, field) {
    if (typeof value === 'string' && !/^[a-zA-Z0-9]+$/.test(value)) {
      return { valid: false, error: `${field} must contain only letters and numbers` };
    }
    return { valid: true };
  },
  range(value, args, field) {
    const [min, max] = args;
    if (typeof value === 'number' && (value < min || value > max)) {
      return { valid: false, error: `${field} must be between ${min} and ${max}` };
    }
    return { valid: true };
  },
};

function validate(data, rules) {
  const errors = {};
  for (const [field, fieldRules] of Object.entries(rules)) {
    const value = data[field];
    for (const rule of fieldRules) {
      const validator = validators[rule.name];
      if (validator) {
        const result = validator(value, rule.args || [], field);
        if (!result.valid) {
          errors[field] = errors[field] || [];
          errors[field].push(result.error);
        }
      }
    }
  }
  return { valid: Object.keys(errors).length === 0, errors };
}

function createStore(name, initialState) {
  const state = { ...initialState };
  const listeners = [];
  const reducers = new Map();
  const effects = new Map();
  const selectors = new Map();

  const store = {
    get state() { return state; },
    on(action, reducer) { reducers.set(action, reducer); },
    effect(action, handler) { effects.set(action, handler); },
    selector(name, fn) { selectors.set(name, fn); },
    subscribe(fn) { listeners.push(fn); },
    dispatch(action, ...args) {
      const reducer = reducers.get(action);
      if (reducer) {
        Object.assign(state, reducer(state, ...args));
        listeners.forEach(fn => fn(state));
      }
      const effect = effects.get(action);
      if (effect) effect(...args);
    },
    select(name) {
      const selector = selectors.get(name);
      return selector ? selector(state) : undefined;
    }
  };
  stores.set(name, store);
  return store;
}

function dispatch(storeName, action, ...args) {
  const store = stores.get(storeName);
  if (store) store.dispatch(action, ...args);
}

function mount(componentFn, container) {
  const el = document.querySelector(container);
  if (!el) return;
  const render = () => {
    const vdom = componentFn();
    el.innerHTML = renderVdom(vdom);
    bindEvents(el, vdom);
  };
  stores.forEach(store => store.subscribe(render));
  render();
}

function renderVdom(vdom) {
  if (!vdom) return '';
  const { type, content, value, style, children, align, inputType, placeholder } = vdom;
  const styleAttr = style ? ` class="${style}"` : '';
  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';
  
  if (type === 'text') {
    return `<span${styleAttr}>${content || value || ''}</span>`;
  }
  if (type === 'button') {
    return `<button${styleAttr} data-click="true">${content || ''}</button>`;
  }
  if (type === 'input') {
    const inputTypeAttr = inputType || 'text';
    const placeholderAttr = placeholder ? ` placeholder="${placeholder}"` : '';
    const valueAttr = value !== undefined ? ` value="${value}"` : '';
    return `<input type="${inputTypeAttr}"${styleAttr}${placeholderAttr}${valueAttr} data-input="true" />`;
  }
  if (children) {
    const inner = children.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');
    return `<div class="${(style || '') + flexClass}">${inner}</div>`;
  }
  return `<div${styleAttr}>${content || value || ''}</div>`;
}

function bindEvents(el, vdom) {
  el.querySelectorAll('[data-click]').forEach((btn, i) => {
    const handler = findClickHandler(vdom, i);
    if (handler) btn.onclick = handler;
  });
  el.querySelectorAll('[data-input]').forEach((input, i) => {
    const handler = findInputHandler(vdom, i);
    if (handler) input.oninput = (e) => handler(e.target.value);
  });
}

function findClickHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.click && count.n++ === index) return vdom.click;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findClickHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function findInputHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.input && count.n++ === index) return vdom.input;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findInputHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function Logo() {
  return {
    type: 'text',
    content: 'topo',
    style: 'text-3xl font-bold text-indigo-600 mb-8'
  };
}

function Sidebar() {
  return {
    style: 'w-56 bg-white border-r border-gray-200 p-6 min-h-screen fixed left-0 top-0',
    align: 'vertical',
    children: [Logo, NavItem('Home'), NavItem('Getting Started'), NavItem('Syntax'), NavItem('Components'), NavItem('State'), NavItem('API Services')]
  };
}


// topo runtime
const stores = new Map();

// Validators
const validators = {
  required(value, _args, field) {
    if (value === null || value === undefined || value === '') {
      return { valid: false, error: `${field} is required` };
    }
    return { valid: true };
  },
  min(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    if (typeof value === 'number' && value < min) {
      return { valid: false, error: `${field} must be at least ${min}` };
    }
    return { valid: true };
  },
  max(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    if (typeof value === 'number' && value > max) {
      return { valid: false, error: `${field} must be at most ${max}` };
    }
    return { valid: true };
  },
  minLength(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    return { valid: true };
  },
  maxLength(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    return { valid: true };
  },
  email(value, _args, field) {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (typeof value === 'string' && !emailRegex.test(value)) {
      return { valid: false, error: `${field} must be a valid email address` };
    }
    return { valid: true };
  },
  pattern(value, args, field) {
    const pattern = new RegExp(args[0]);
    if (typeof value === 'string' && !pattern.test(value)) {
      return { valid: false, error: `${field} does not match the required pattern` };
    }
    return { valid: true };
  },
  url(value, _args, field) {
    try {
      new URL(value);
      return { valid: true };
    } catch {
      return { valid: false, error: `${field} must be a valid URL` };
    }
  },
  alphanumeric(value, _args, field) {
    if (typeof value === 'string' && !/^[a-zA-Z0-9]+$/.test(value)) {
      return { valid: false, error: `${field} must contain only letters and numbers` };
    }
    return { valid: true };
  },
  range(value, args, field) {
    const [min, max] = args;
    if (typeof value === 'number' && (value < min || value > max)) {
      return { valid: false, error: `${field} must be between ${min} and ${max}` };
    }
    return { valid: true };
  },
};

function validate(data, rules) {
  const errors = {};
  for (const [field, fieldRules] of Object.entries(rules)) {
    const value = data[field];
    for (const rule of fieldRules) {
      const validator = validators[rule.name];
      if (validator) {
        const result = validator(value, rule.args || [], field);
        if (!result.valid) {
          errors[field] = errors[field] || [];
          errors[field].push(result.error);
        }
      }
    }
  }
  return { valid: Object.keys(errors).length === 0, errors };
}

function createStore(name, initialState) {
  const state = { ...initialState };
  const listeners = [];
  const reducers = new Map();
  const effects = new Map();
  const selectors = new Map();

  const store = {
    get state() { return state; },
    on(action, reducer) { reducers.set(action, reducer); },
    effect(action, handler) { effects.set(action, handler); },
    selector(name, fn) { selectors.set(name, fn); },
    subscribe(fn) { listeners.push(fn); },
    dispatch(action, ...args) {
      const reducer = reducers.get(action);
      if (reducer) {
        Object.assign(state, reducer(state, ...args));
        listeners.forEach(fn => fn(state));
      }
      const effect = effects.get(action);
      if (effect) effect(...args);
    },
    select(name) {
      const selector = selectors.get(name);
      return selector ? selector(state) : undefined;
    }
  };
  stores.set(name, store);
  return store;
}

function dispatch(storeName, action, ...args) {
  const store = stores.get(storeName);
  if (store) store.dispatch(action, ...args);
}

function mount(componentFn, container) {
  const el = document.querySelector(container);
  if (!el) return;
  const render = () => {
    const vdom = componentFn();
    el.innerHTML = renderVdom(vdom);
    bindEvents(el, vdom);
  };
  stores.forEach(store => store.subscribe(render));
  render();
}

function renderVdom(vdom) {
  if (!vdom) return '';
  const { type, content, value, style, children, align, inputType, placeholder } = vdom;
  const styleAttr = style ? ` class="${style}"` : '';
  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';
  
  if (type === 'text') {
    return `<span${styleAttr}>${content || value || ''}</span>`;
  }
  if (type === 'button') {
    return `<button${styleAttr} data-click="true">${content || ''}</button>`;
  }
  if (type === 'input') {
    const inputTypeAttr = inputType || 'text';
    const placeholderAttr = placeholder ? ` placeholder="${placeholder}"` : '';
    const valueAttr = value !== undefined ? ` value="${value}"` : '';
    return `<input type="${inputTypeAttr}"${styleAttr}${placeholderAttr}${valueAttr} data-input="true" />`;
  }
  if (children) {
    const inner = children.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');
    return `<div class="${(style || '') + flexClass}">${inner}</div>`;
  }
  return `<div${styleAttr}>${content || value || ''}</div>`;
}

function bindEvents(el, vdom) {
  el.querySelectorAll('[data-click]').forEach((btn, i) => {
    const handler = findClickHandler(vdom, i);
    if (handler) btn.onclick = handler;
  });
  el.querySelectorAll('[data-input]').forEach((input, i) => {
    const handler = findInputHandler(vdom, i);
    if (handler) input.oninput = (e) => handler(e.target.value);
  });
}

function findClickHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.click && count.n++ === index) return vdom.click;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findClickHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function findInputHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.input && count.n++ === index) return vdom.input;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findInputHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function Text(content, style) {
  return {
    type: 'text',
    content: content,
    style: style
  };
}

function Heading(content, level) {
  return {
    type: 'text',
    content: content,
    style: level
  };
}

function Code(content) {
  return {
    type: 'text',
    content: content,
    style: 'font-mono text-sm bg-gray-900 text-green-400 p-6 rounded-xl whitespace-pre-wrap'
  };
}

function Badge(content, color) {
  return {
    type: 'text',
    content: content,
    style: color
  };
}

// topo runtime
const stores = new Map();

// Validators
const validators = {
  required(value, _args, field) {
    if (value === null || value === undefined || value === '') {
      return { valid: false, error: `${field} is required` };
    }
    return { valid: true };
  },
  min(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    if (typeof value === 'number' && value < min) {
      return { valid: false, error: `${field} must be at least ${min}` };
    }
    return { valid: true };
  },
  max(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    if (typeof value === 'number' && value > max) {
      return { valid: false, error: `${field} must be at most ${max}` };
    }
    return { valid: true };
  },
  minLength(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    return { valid: true };
  },
  maxLength(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    return { valid: true };
  },
  email(value, _args, field) {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (typeof value === 'string' && !emailRegex.test(value)) {
      return { valid: false, error: `${field} must be a valid email address` };
    }
    return { valid: true };
  },
  pattern(value, args, field) {
    const pattern = new RegExp(args[0]);
    if (typeof value === 'string' && !pattern.test(value)) {
      return { valid: false, error: `${field} does not match the required pattern` };
    }
    return { valid: true };
  },
  url(value, _args, field) {
    try {
      new URL(value);
      return { valid: true };
    } catch {
      return { valid: false, error: `${field} must be a valid URL` };
    }
  },
  alphanumeric(value, _args, field) {
    if (typeof value === 'string' && !/^[a-zA-Z0-9]+$/.test(value)) {
      return { valid: false, error: `${field} must contain only letters and numbers` };
    }
    return { valid: true };
  },
  range(value, args, field) {
    const [min, max] = args;
    if (typeof value === 'number' && (value < min || value > max)) {
      return { valid: false, error: `${field} must be between ${min} and ${max}` };
    }
    return { valid: true };
  },
};

function validate(data, rules) {
  const errors = {};
  for (const [field, fieldRules] of Object.entries(rules)) {
    const value = data[field];
    for (const rule of fieldRules) {
      const validator = validators[rule.name];
      if (validator) {
        const result = validator(value, rule.args || [], field);
        if (!result.valid) {
          errors[field] = errors[field] || [];
          errors[field].push(result.error);
        }
      }
    }
  }
  return { valid: Object.keys(errors).length === 0, errors };
}

function createStore(name, initialState) {
  const state = { ...initialState };
  const listeners = [];
  const reducers = new Map();
  const effects = new Map();
  const selectors = new Map();

  const store = {
    get state() { return state; },
    on(action, reducer) { reducers.set(action, reducer); },
    effect(action, handler) { effects.set(action, handler); },
    selector(name, fn) { selectors.set(name, fn); },
    subscribe(fn) { listeners.push(fn); },
    dispatch(action, ...args) {
      const reducer = reducers.get(action);
      if (reducer) {
        Object.assign(state, reducer(state, ...args));
        listeners.forEach(fn => fn(state));
      }
      const effect = effects.get(action);
      if (effect) effect(...args);
    },
    select(name) {
      const selector = selectors.get(name);
      return selector ? selector(state) : undefined;
    }
  };
  stores.set(name, store);
  return store;
}

function dispatch(storeName, action, ...args) {
  const store = stores.get(storeName);
  if (store) store.dispatch(action, ...args);
}

function mount(componentFn, container) {
  const el = document.querySelector(container);
  if (!el) return;
  const render = () => {
    const vdom = componentFn();
    el.innerHTML = renderVdom(vdom);
    bindEvents(el, vdom);
  };
  stores.forEach(store => store.subscribe(render));
  render();
}

function renderVdom(vdom) {
  if (!vdom) return '';
  const { type, content, value, style, children, align, inputType, placeholder } = vdom;
  const styleAttr = style ? ` class="${style}"` : '';
  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';
  
  if (type === 'text') {
    return `<span${styleAttr}>${content || value || ''}</span>`;
  }
  if (type === 'button') {
    return `<button${styleAttr} data-click="true">${content || ''}</button>`;
  }
  if (type === 'input') {
    const inputTypeAttr = inputType || 'text';
    const placeholderAttr = placeholder ? ` placeholder="${placeholder}"` : '';
    const valueAttr = value !== undefined ? ` value="${value}"` : '';
    return `<input type="${inputTypeAttr}"${styleAttr}${placeholderAttr}${valueAttr} data-input="true" />`;
  }
  if (children) {
    const inner = children.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');
    return `<div class="${(style || '') + flexClass}">${inner}</div>`;
  }
  return `<div${styleAttr}>${content || value || ''}</div>`;
}

function bindEvents(el, vdom) {
  el.querySelectorAll('[data-click]').forEach((btn, i) => {
    const handler = findClickHandler(vdom, i);
    if (handler) btn.onclick = handler;
  });
  el.querySelectorAll('[data-input]').forEach((input, i) => {
    const handler = findInputHandler(vdom, i);
    if (handler) input.oninput = (e) => handler(e.target.value);
  });
}

function findClickHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.click && count.n++ === index) return vdom.click;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findClickHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function findInputHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.input && count.n++ === index) return vdom.input;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findInputHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function Logo() {
  return {
    type: 'text',
    content: 'topo',
    style: 'text-3xl font-bold text-indigo-600 mb-8'
  };
}

function Sidebar() {
  return {
    style: 'w-56 bg-white border-r border-gray-200 p-6 min-h-screen fixed left-0 top-0',
    align: 'vertical',
    children: [Logo, NavItem('Home'), NavItem('Getting Started'), NavItem('Syntax'), NavItem('Components'), NavItem('State'), NavItem('API Services')]
  };
}

// topo runtime
const stores = new Map();

// Validators
const validators = {
  required(value, _args, field) {
    if (value === null || value === undefined || value === '') {
      return { valid: false, error: `${field} is required` };
    }
    return { valid: true };
  },
  min(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    if (typeof value === 'number' && value < min) {
      return { valid: false, error: `${field} must be at least ${min}` };
    }
    return { valid: true };
  },
  max(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    if (typeof value === 'number' && value > max) {
      return { valid: false, error: `${field} must be at most ${max}` };
    }
    return { valid: true };
  },
  minLength(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    return { valid: true };
  },
  maxLength(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    return { valid: true };
  },
  email(value, _args, field) {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (typeof value === 'string' && !emailRegex.test(value)) {
      return { valid: false, error: `${field} must be a valid email address` };
    }
    return { valid: true };
  },
  pattern(value, args, field) {
    const pattern = new RegExp(args[0]);
    if (typeof value === 'string' && !pattern.test(value)) {
      return { valid: false, error: `${field} does not match the required pattern` };
    }
    return { valid: true };
  },
  url(value, _args, field) {
    try {
      new URL(value);
      return { valid: true };
    } catch {
      return { valid: false, error: `${field} must be a valid URL` };
    }
  },
  alphanumeric(value, _args, field) {
    if (typeof value === 'string' && !/^[a-zA-Z0-9]+$/.test(value)) {
      return { valid: false, error: `${field} must contain only letters and numbers` };
    }
    return { valid: true };
  },
  range(value, args, field) {
    const [min, max] = args;
    if (typeof value === 'number' && (value < min || value > max)) {
      return { valid: false, error: `${field} must be between ${min} and ${max}` };
    }
    return { valid: true };
  },
};

function validate(data, rules) {
  const errors = {};
  for (const [field, fieldRules] of Object.entries(rules)) {
    const value = data[field];
    for (const rule of fieldRules) {
      const validator = validators[rule.name];
      if (validator) {
        const result = validator(value, rule.args || [], field);
        if (!result.valid) {
          errors[field] = errors[field] || [];
          errors[field].push(result.error);
        }
      }
    }
  }
  return { valid: Object.keys(errors).length === 0, errors };
}

function createStore(name, initialState) {
  const state = { ...initialState };
  const listeners = [];
  const reducers = new Map();
  const effects = new Map();
  const selectors = new Map();

  const store = {
    get state() { return state; },
    on(action, reducer) { reducers.set(action, reducer); },
    effect(action, handler) { effects.set(action, handler); },
    selector(name, fn) { selectors.set(name, fn); },
    subscribe(fn) { listeners.push(fn); },
    dispatch(action, ...args) {
      const reducer = reducers.get(action);
      if (reducer) {
        Object.assign(state, reducer(state, ...args));
        listeners.forEach(fn => fn(state));
      }
      const effect = effects.get(action);
      if (effect) effect(...args);
    },
    select(name) {
      const selector = selectors.get(name);
      return selector ? selector(state) : undefined;
    }
  };
  stores.set(name, store);
  return store;
}

function dispatch(storeName, action, ...args) {
  const store = stores.get(storeName);
  if (store) store.dispatch(action, ...args);
}

function mount(componentFn, container) {
  const el = document.querySelector(container);
  if (!el) return;
  const render = () => {
    const vdom = componentFn();
    el.innerHTML = renderVdom(vdom);
    bindEvents(el, vdom);
  };
  stores.forEach(store => store.subscribe(render));
  render();
}

function renderVdom(vdom) {
  if (!vdom) return '';
  const { type, content, value, style, children, align, inputType, placeholder } = vdom;
  const styleAttr = style ? ` class="${style}"` : '';
  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';
  
  if (type === 'text') {
    return `<span${styleAttr}>${content || value || ''}</span>`;
  }
  if (type === 'button') {
    return `<button${styleAttr} data-click="true">${content || ''}</button>`;
  }
  if (type === 'input') {
    const inputTypeAttr = inputType || 'text';
    const placeholderAttr = placeholder ? ` placeholder="${placeholder}"` : '';
    const valueAttr = value !== undefined ? ` value="${value}"` : '';
    return `<input type="${inputTypeAttr}"${styleAttr}${placeholderAttr}${valueAttr} data-input="true" />`;
  }
  if (children) {
    const inner = children.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');
    return `<div class="${(style || '') + flexClass}">${inner}</div>`;
  }
  return `<div${styleAttr}>${content || value || ''}</div>`;
}

function bindEvents(el, vdom) {
  el.querySelectorAll('[data-click]').forEach((btn, i) => {
    const handler = findClickHandler(vdom, i);
    if (handler) btn.onclick = handler;
  });
  el.querySelectorAll('[data-input]').forEach((input, i) => {
    const handler = findInputHandler(vdom, i);
    if (handler) input.oninput = (e) => handler(e.target.value);
  });
}

function findClickHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.click && count.n++ === index) return vdom.click;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findClickHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function findInputHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.input && count.n++ === index) return vdom.input;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findInputHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function Hero() {
  return {
    style: 'mb-16',
    align: 'vertical',
    children: [Heading('topo', 'text-7xl font-black text-gray-900 mb-4'), Text('A UI framework that eliminates nesting hell', 'text-2xl text-gray-600 mb-6'), Text('Write flat, readable UI code. No more callback pyramids or deeply nested components.', 'text-lg text-gray-500 mb-10 max-w-xl')]
  };
}

function Features() {
  return {
    style: 'grid grid-cols-3 gap-6 mb-16',
    children: [Card('Flat Definitions', 'Define components at the top level. Compose by reference with children: [A, B, C]'), Card('NgRx-style State', 'Built-in state with Actions, Reducers, Effects, and Selectors'), Card('Auto API Services', 'Define REST endpoints, get CRUD methods auto-generated')]
  };
}

function Syntax() {
  return {
    style: 'mb-16',
    align: 'vertical',
    children: [Heading('4 Definition Operators', 'text-3xl font-bold text-gray-900 mb-6'), Operator('->  Component (UI)', 'Component', 'font-mono text-lg bg-blue-50 text-blue-700 px-4 py-2 rounded mb-2'), Operator('|   Store (State)', 'Store', 'font-mono text-lg bg-purple-50 text-purple-700 px-4 py-2 rounded mb-2'), Operator('::  API Service', 'API', 'font-mono text-lg bg-green-50 text-green-700 px-4 py-2 rounded mb-2'), Operator('{}  Method (Logic)', 'Method', 'font-mono text-lg bg-orange-50 text-orange-700 px-4 py-2 rounded mb-2')]
  };
}

function Example() {
  return {
    style: 'mb-16',
    align: 'vertical',
    children: [Heading('Example: Counter', 'text-3xl font-bold text-gray-900 mb-6'), Code('Counter | {\n    State { count: 0 }\n    Actions { Increment, Decrement }\n    Reducers {\n        on Increment { count: count + 1 }\n        on Decrement { count: count - 1 }\n    }\n}\n\nDisplay -> {\n    type: text\n    value: $Counter.count\n}\n\nButton -> {\n    type: button\n    content: \"+\"\n    click: Counter.Increment\n}')]
  };
}

function GettingStarted() {
  return {
    style: 'mb-16',
    align: 'vertical',
    children: [Heading('Getting Started', 'text-3xl font-bold text-gray-900 mb-6'), Step('1. Install:  cargo install --path .'), Step('2. Create:   topo new my-app'), Step('3. Run:      cd my-app && topo start')]
  };
}

function Footer() {
  return {
    type: 'text',
    content: 'Built with topo - MIT License',
    style: 'text-gray-400 text-sm mt-16 pt-8 border-t border-gray-200'
  };
}


// topo runtime
const stores = new Map();

// Validators
const validators = {
  required(value, _args, field) {
    if (value === null || value === undefined || value === '') {
      return { valid: false, error: `${field} is required` };
    }
    return { valid: true };
  },
  min(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    if (typeof value === 'number' && value < min) {
      return { valid: false, error: `${field} must be at least ${min}` };
    }
    return { valid: true };
  },
  max(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    if (typeof value === 'number' && value > max) {
      return { valid: false, error: `${field} must be at most ${max}` };
    }
    return { valid: true };
  },
  minLength(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    return { valid: true };
  },
  maxLength(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    return { valid: true };
  },
  email(value, _args, field) {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (typeof value === 'string' && !emailRegex.test(value)) {
      return { valid: false, error: `${field} must be a valid email address` };
    }
    return { valid: true };
  },
  pattern(value, args, field) {
    const pattern = new RegExp(args[0]);
    if (typeof value === 'string' && !pattern.test(value)) {
      return { valid: false, error: `${field} does not match the required pattern` };
    }
    return { valid: true };
  },
  url(value, _args, field) {
    try {
      new URL(value);
      return { valid: true };
    } catch {
      return { valid: false, error: `${field} must be a valid URL` };
    }
  },
  alphanumeric(value, _args, field) {
    if (typeof value === 'string' && !/^[a-zA-Z0-9]+$/.test(value)) {
      return { valid: false, error: `${field} must contain only letters and numbers` };
    }
    return { valid: true };
  },
  range(value, args, field) {
    const [min, max] = args;
    if (typeof value === 'number' && (value < min || value > max)) {
      return { valid: false, error: `${field} must be between ${min} and ${max}` };
    }
    return { valid: true };
  },
};

function validate(data, rules) {
  const errors = {};
  for (const [field, fieldRules] of Object.entries(rules)) {
    const value = data[field];
    for (const rule of fieldRules) {
      const validator = validators[rule.name];
      if (validator) {
        const result = validator(value, rule.args || [], field);
        if (!result.valid) {
          errors[field] = errors[field] || [];
          errors[field].push(result.error);
        }
      }
    }
  }
  return { valid: Object.keys(errors).length === 0, errors };
}

function createStore(name, initialState) {
  const state = { ...initialState };
  const listeners = [];
  const reducers = new Map();
  const effects = new Map();
  const selectors = new Map();

  const store = {
    get state() { return state; },
    on(action, reducer) { reducers.set(action, reducer); },
    effect(action, handler) { effects.set(action, handler); },
    selector(name, fn) { selectors.set(name, fn); },
    subscribe(fn) { listeners.push(fn); },
    dispatch(action, ...args) {
      const reducer = reducers.get(action);
      if (reducer) {
        Object.assign(state, reducer(state, ...args));
        listeners.forEach(fn => fn(state));
      }
      const effect = effects.get(action);
      if (effect) effect(...args);
    },
    select(name) {
      const selector = selectors.get(name);
      return selector ? selector(state) : undefined;
    }
  };
  stores.set(name, store);
  return store;
}

function dispatch(storeName, action, ...args) {
  const store = stores.get(storeName);
  if (store) store.dispatch(action, ...args);
}

function mount(componentFn, container) {
  const el = document.querySelector(container);
  if (!el) return;
  const render = () => {
    const vdom = componentFn();
    el.innerHTML = renderVdom(vdom);
    bindEvents(el, vdom);
  };
  stores.forEach(store => store.subscribe(render));
  render();
}

function renderVdom(vdom) {
  if (!vdom) return '';
  const { type, content, value, style, children, align, inputType, placeholder } = vdom;
  const styleAttr = style ? ` class="${style}"` : '';
  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';
  
  if (type === 'text') {
    return `<span${styleAttr}>${content || value || ''}</span>`;
  }
  if (type === 'button') {
    return `<button${styleAttr} data-click="true">${content || ''}</button>`;
  }
  if (type === 'input') {
    const inputTypeAttr = inputType || 'text';
    const placeholderAttr = placeholder ? ` placeholder="${placeholder}"` : '';
    const valueAttr = value !== undefined ? ` value="${value}"` : '';
    return `<input type="${inputTypeAttr}"${styleAttr}${placeholderAttr}${valueAttr} data-input="true" />`;
  }
  if (children) {
    const inner = children.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');
    return `<div class="${(style || '') + flexClass}">${inner}</div>`;
  }
  return `<div${styleAttr}>${content || value || ''}</div>`;
}

function bindEvents(el, vdom) {
  el.querySelectorAll('[data-click]').forEach((btn, i) => {
    const handler = findClickHandler(vdom, i);
    if (handler) btn.onclick = handler;
  });
  el.querySelectorAll('[data-input]').forEach((input, i) => {
    const handler = findInputHandler(vdom, i);
    if (handler) input.oninput = (e) => handler(e.target.value);
  });
}

function findClickHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.click && count.n++ === index) return vdom.click;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findClickHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function findInputHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.input && count.n++ === index) return vdom.input;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findInputHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function Text(content, style) {
  return {
    type: 'text',
    content: content,
    style: style
  };
}

function Heading(content, level) {
  return {
    type: 'text',
    content: content,
    style: level
  };
}

function Code(content) {
  return {
    type: 'text',
    content: content,
    style: 'font-mono text-sm bg-gray-900 text-green-400 p-6 rounded-xl whitespace-pre-wrap'
  };
}

function Badge(content, color) {
  return {
    type: 'text',
    content: content,
    style: color
  };
}

// topo runtime
const stores = new Map();

// Validators
const validators = {
  required(value, _args, field) {
    if (value === null || value === undefined || value === '') {
      return { valid: false, error: `${field} is required` };
    }
    return { valid: true };
  },
  min(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    if (typeof value === 'number' && value < min) {
      return { valid: false, error: `${field} must be at least ${min}` };
    }
    return { valid: true };
  },
  max(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    if (typeof value === 'number' && value > max) {
      return { valid: false, error: `${field} must be at most ${max}` };
    }
    return { valid: true };
  },
  minLength(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    return { valid: true };
  },
  maxLength(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    return { valid: true };
  },
  email(value, _args, field) {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (typeof value === 'string' && !emailRegex.test(value)) {
      return { valid: false, error: `${field} must be a valid email address` };
    }
    return { valid: true };
  },
  pattern(value, args, field) {
    const pattern = new RegExp(args[0]);
    if (typeof value === 'string' && !pattern.test(value)) {
      return { valid: false, error: `${field} does not match the required pattern` };
    }
    return { valid: true };
  },
  url(value, _args, field) {
    try {
      new URL(value);
      return { valid: true };
    } catch {
      return { valid: false, error: `${field} must be a valid URL` };
    }
  },
  alphanumeric(value, _args, field) {
    if (typeof value === 'string' && !/^[a-zA-Z0-9]+$/.test(value)) {
      return { valid: false, error: `${field} must contain only letters and numbers` };
    }
    return { valid: true };
  },
  range(value, args, field) {
    const [min, max] = args;
    if (typeof value === 'number' && (value < min || value > max)) {
      return { valid: false, error: `${field} must be between ${min} and ${max}` };
    }
    return { valid: true };
  },
};

function validate(data, rules) {
  const errors = {};
  for (const [field, fieldRules] of Object.entries(rules)) {
    const value = data[field];
    for (const rule of fieldRules) {
      const validator = validators[rule.name];
      if (validator) {
        const result = validator(value, rule.args || [], field);
        if (!result.valid) {
          errors[field] = errors[field] || [];
          errors[field].push(result.error);
        }
      }
    }
  }
  return { valid: Object.keys(errors).length === 0, errors };
}

function createStore(name, initialState) {
  const state = { ...initialState };
  const listeners = [];
  const reducers = new Map();
  const effects = new Map();
  const selectors = new Map();

  const store = {
    get state() { return state; },
    on(action, reducer) { reducers.set(action, reducer); },
    effect(action, handler) { effects.set(action, handler); },
    selector(name, fn) { selectors.set(name, fn); },
    subscribe(fn) { listeners.push(fn); },
    dispatch(action, ...args) {
      const reducer = reducers.get(action);
      if (reducer) {
        Object.assign(state, reducer(state, ...args));
        listeners.forEach(fn => fn(state));
      }
      const effect = effects.get(action);
      if (effect) effect(...args);
    },
    select(name) {
      const selector = selectors.get(name);
      return selector ? selector(state) : undefined;
    }
  };
  stores.set(name, store);
  return store;
}

function dispatch(storeName, action, ...args) {
  const store = stores.get(storeName);
  if (store) store.dispatch(action, ...args);
}

function mount(componentFn, container) {
  const el = document.querySelector(container);
  if (!el) return;
  const render = () => {
    const vdom = componentFn();
    el.innerHTML = renderVdom(vdom);
    bindEvents(el, vdom);
  };
  stores.forEach(store => store.subscribe(render));
  render();
}

function renderVdom(vdom) {
  if (!vdom) return '';
  const { type, content, value, style, children, align, inputType, placeholder } = vdom;
  const styleAttr = style ? ` class="${style}"` : '';
  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';
  
  if (type === 'text') {
    return `<span${styleAttr}>${content || value || ''}</span>`;
  }
  if (type === 'button') {
    return `<button${styleAttr} data-click="true">${content || ''}</button>`;
  }
  if (type === 'input') {
    const inputTypeAttr = inputType || 'text';
    const placeholderAttr = placeholder ? ` placeholder="${placeholder}"` : '';
    const valueAttr = value !== undefined ? ` value="${value}"` : '';
    return `<input type="${inputTypeAttr}"${styleAttr}${placeholderAttr}${valueAttr} data-input="true" />`;
  }
  if (children) {
    const inner = children.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');
    return `<div class="${(style || '') + flexClass}">${inner}</div>`;
  }
  return `<div${styleAttr}>${content || value || ''}</div>`;
}

function bindEvents(el, vdom) {
  el.querySelectorAll('[data-click]').forEach((btn, i) => {
    const handler = findClickHandler(vdom, i);
    if (handler) btn.onclick = handler;
  });
  el.querySelectorAll('[data-input]').forEach((input, i) => {
    const handler = findInputHandler(vdom, i);
    if (handler) input.oninput = (e) => handler(e.target.value);
  });
}

function findClickHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.click && count.n++ === index) return vdom.click;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findClickHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function findInputHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.input && count.n++ === index) return vdom.input;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findInputHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function Logo() {
  return {
    type: 'text',
    content: 'topo',
    style: 'text-3xl font-bold text-indigo-600 mb-8'
  };
}

function Sidebar() {
  return {
    style: 'w-56 bg-white border-r border-gray-200 p-6 min-h-screen fixed left-0 top-0',
    align: 'vertical',
    children: [Logo, NavItem('Home'), NavItem('Getting Started'), NavItem('Syntax'), NavItem('Components'), NavItem('State'), NavItem('API Services')]
  };
}

// topo runtime
const stores = new Map();

// Validators
const validators = {
  required(value, _args, field) {
    if (value === null || value === undefined || value === '') {
      return { valid: false, error: `${field} is required` };
    }
    return { valid: true };
  },
  min(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    if (typeof value === 'number' && value < min) {
      return { valid: false, error: `${field} must be at least ${min}` };
    }
    return { valid: true };
  },
  max(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    if (typeof value === 'number' && value > max) {
      return { valid: false, error: `${field} must be at most ${max}` };
    }
    return { valid: true };
  },
  minLength(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    return { valid: true };
  },
  maxLength(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    return { valid: true };
  },
  email(value, _args, field) {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (typeof value === 'string' && !emailRegex.test(value)) {
      return { valid: false, error: `${field} must be a valid email address` };
    }
    return { valid: true };
  },
  pattern(value, args, field) {
    const pattern = new RegExp(args[0]);
    if (typeof value === 'string' && !pattern.test(value)) {
      return { valid: false, error: `${field} does not match the required pattern` };
    }
    return { valid: true };
  },
  url(value, _args, field) {
    try {
      new URL(value);
      return { valid: true };
    } catch {
      return { valid: false, error: `${field} must be a valid URL` };
    }
  },
  alphanumeric(value, _args, field) {
    if (typeof value === 'string' && !/^[a-zA-Z0-9]+$/.test(value)) {
      return { valid: false, error: `${field} must contain only letters and numbers` };
    }
    return { valid: true };
  },
  range(value, args, field) {
    const [min, max] = args;
    if (typeof value === 'number' && (value < min || value > max)) {
      return { valid: false, error: `${field} must be between ${min} and ${max}` };
    }
    return { valid: true };
  },
};

function validate(data, rules) {
  const errors = {};
  for (const [field, fieldRules] of Object.entries(rules)) {
    const value = data[field];
    for (const rule of fieldRules) {
      const validator = validators[rule.name];
      if (validator) {
        const result = validator(value, rule.args || [], field);
        if (!result.valid) {
          errors[field] = errors[field] || [];
          errors[field].push(result.error);
        }
      }
    }
  }
  return { valid: Object.keys(errors).length === 0, errors };
}

function createStore(name, initialState) {
  const state = { ...initialState };
  const listeners = [];
  const reducers = new Map();
  const effects = new Map();
  const selectors = new Map();

  const store = {
    get state() { return state; },
    on(action, reducer) { reducers.set(action, reducer); },
    effect(action, handler) { effects.set(action, handler); },
    selector(name, fn) { selectors.set(name, fn); },
    subscribe(fn) { listeners.push(fn); },
    dispatch(action, ...args) {
      const reducer = reducers.get(action);
      if (reducer) {
        Object.assign(state, reducer(state, ...args));
        listeners.forEach(fn => fn(state));
      }
      const effect = effects.get(action);
      if (effect) effect(...args);
    },
    select(name) {
      const selector = selectors.get(name);
      return selector ? selector(state) : undefined;
    }
  };
  stores.set(name, store);
  return store;
}

function dispatch(storeName, action, ...args) {
  const store = stores.get(storeName);
  if (store) store.dispatch(action, ...args);
}

function mount(componentFn, container) {
  const el = document.querySelector(container);
  if (!el) return;
  const render = () => {
    const vdom = componentFn();
    el.innerHTML = renderVdom(vdom);
    bindEvents(el, vdom);
  };
  stores.forEach(store => store.subscribe(render));
  render();
}

function renderVdom(vdom) {
  if (!vdom) return '';
  const { type, content, value, style, children, align, inputType, placeholder } = vdom;
  const styleAttr = style ? ` class="${style}"` : '';
  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';
  
  if (type === 'text') {
    return `<span${styleAttr}>${content || value || ''}</span>`;
  }
  if (type === 'button') {
    return `<button${styleAttr} data-click="true">${content || ''}</button>`;
  }
  if (type === 'input') {
    const inputTypeAttr = inputType || 'text';
    const placeholderAttr = placeholder ? ` placeholder="${placeholder}"` : '';
    const valueAttr = value !== undefined ? ` value="${value}"` : '';
    return `<input type="${inputTypeAttr}"${styleAttr}${placeholderAttr}${valueAttr} data-input="true" />`;
  }
  if (children) {
    const inner = children.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');
    return `<div class="${(style || '') + flexClass}">${inner}</div>`;
  }
  return `<div${styleAttr}>${content || value || ''}</div>`;
}

function bindEvents(el, vdom) {
  el.querySelectorAll('[data-click]').forEach((btn, i) => {
    const handler = findClickHandler(vdom, i);
    if (handler) btn.onclick = handler;
  });
  el.querySelectorAll('[data-input]').forEach((input, i) => {
    const handler = findInputHandler(vdom, i);
    if (handler) input.oninput = (e) => handler(e.target.value);
  });
}

function findClickHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.click && count.n++ === index) return vdom.click;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findClickHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function findInputHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.input && count.n++ === index) return vdom.input;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findInputHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function Hero() {
  return {
    style: 'mb-16',
    align: 'vertical',
    children: [Heading('topo', 'text-7xl font-black text-gray-900 mb-4'), Text('A UI framework that eliminates nesting hell', 'text-2xl text-gray-600 mb-6'), Text('Write flat, readable UI code. No more callback pyramids or deeply nested components.', 'text-lg text-gray-500 mb-10 max-w-xl')]
  };
}

function Features() {
  return {
    style: 'grid grid-cols-3 gap-6 mb-16',
    children: [Card('Flat Definitions', 'Define components at the top level. Compose by reference with children: [A, B, C]'), Card('NgRx-style State', 'Built-in state with Actions, Reducers, Effects, and Selectors'), Card('Auto API Services', 'Define REST endpoints, get CRUD methods auto-generated')]
  };
}

function Syntax() {
  return {
    style: 'mb-16',
    align: 'vertical',
    children: [Heading('4 Definition Operators', 'text-3xl font-bold text-gray-900 mb-6'), Operator('->  Component (UI)', 'Component', 'font-mono text-lg bg-blue-50 text-blue-700 px-4 py-2 rounded mb-2'), Operator('|   Store (State)', 'Store', 'font-mono text-lg bg-purple-50 text-purple-700 px-4 py-2 rounded mb-2'), Operator('::  API Service', 'API', 'font-mono text-lg bg-green-50 text-green-700 px-4 py-2 rounded mb-2'), Operator('{}  Method (Logic)', 'Method', 'font-mono text-lg bg-orange-50 text-orange-700 px-4 py-2 rounded mb-2')]
  };
}

function Example() {
  return {
    style: 'mb-16',
    align: 'vertical',
    children: [Heading('Example: Counter', 'text-3xl font-bold text-gray-900 mb-6'), Code('Counter | {\n    State { count: 0 }\n    Actions { Increment, Decrement }\n    Reducers {\n        on Increment { count: count + 1 }\n        on Decrement { count: count - 1 }\n    }\n}\n\nDisplay -> {\n    type: text\n    value: $Counter.count\n}\n\nButton -> {\n    type: button\n    content: \"+\"\n    click: Counter.Increment\n}')]
  };
}

function GettingStarted() {
  return {
    style: 'mb-16',
    align: 'vertical',
    children: [Heading('Getting Started', 'text-3xl font-bold text-gray-900 mb-6'), Step('1. Install:  cargo install --path .'), Step('2. Create:   topo new my-app'), Step('3. Run:      cd my-app && topo start')]
  };
}

function Footer() {
  return {
    type: 'text',
    content: 'Built with topo - MIT License',
    style: 'text-gray-400 text-sm mt-16 pt-8 border-t border-gray-200'
  };
}

// topo runtime
const stores = new Map();

// Validators
const validators = {
  required(value, _args, field) {
    if (value === null || value === undefined || value === '') {
      return { valid: false, error: `${field} is required` };
    }
    return { valid: true };
  },
  min(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    if (typeof value === 'number' && value < min) {
      return { valid: false, error: `${field} must be at least ${min}` };
    }
    return { valid: true };
  },
  max(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    if (typeof value === 'number' && value > max) {
      return { valid: false, error: `${field} must be at most ${max}` };
    }
    return { valid: true };
  },
  minLength(value, args, field) {
    const min = args[0];
    if (typeof value === 'string' && value.length < min) {
      return { valid: false, error: `${field} must be at least ${min} characters` };
    }
    return { valid: true };
  },
  maxLength(value, args, field) {
    const max = args[0];
    if (typeof value === 'string' && value.length > max) {
      return { valid: false, error: `${field} must be at most ${max} characters` };
    }
    return { valid: true };
  },
  email(value, _args, field) {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (typeof value === 'string' && !emailRegex.test(value)) {
      return { valid: false, error: `${field} must be a valid email address` };
    }
    return { valid: true };
  },
  pattern(value, args, field) {
    const pattern = new RegExp(args[0]);
    if (typeof value === 'string' && !pattern.test(value)) {
      return { valid: false, error: `${field} does not match the required pattern` };
    }
    return { valid: true };
  },
  url(value, _args, field) {
    try {
      new URL(value);
      return { valid: true };
    } catch {
      return { valid: false, error: `${field} must be a valid URL` };
    }
  },
  alphanumeric(value, _args, field) {
    if (typeof value === 'string' && !/^[a-zA-Z0-9]+$/.test(value)) {
      return { valid: false, error: `${field} must contain only letters and numbers` };
    }
    return { valid: true };
  },
  range(value, args, field) {
    const [min, max] = args;
    if (typeof value === 'number' && (value < min || value > max)) {
      return { valid: false, error: `${field} must be between ${min} and ${max}` };
    }
    return { valid: true };
  },
};

function validate(data, rules) {
  const errors = {};
  for (const [field, fieldRules] of Object.entries(rules)) {
    const value = data[field];
    for (const rule of fieldRules) {
      const validator = validators[rule.name];
      if (validator) {
        const result = validator(value, rule.args || [], field);
        if (!result.valid) {
          errors[field] = errors[field] || [];
          errors[field].push(result.error);
        }
      }
    }
  }
  return { valid: Object.keys(errors).length === 0, errors };
}

function createStore(name, initialState) {
  const state = { ...initialState };
  const listeners = [];
  const reducers = new Map();
  const effects = new Map();
  const selectors = new Map();

  const store = {
    get state() { return state; },
    on(action, reducer) { reducers.set(action, reducer); },
    effect(action, handler) { effects.set(action, handler); },
    selector(name, fn) { selectors.set(name, fn); },
    subscribe(fn) { listeners.push(fn); },
    dispatch(action, ...args) {
      const reducer = reducers.get(action);
      if (reducer) {
        Object.assign(state, reducer(state, ...args));
        listeners.forEach(fn => fn(state));
      }
      const effect = effects.get(action);
      if (effect) effect(...args);
    },
    select(name) {
      const selector = selectors.get(name);
      return selector ? selector(state) : undefined;
    }
  };
  stores.set(name, store);
  return store;
}

function dispatch(storeName, action, ...args) {
  const store = stores.get(storeName);
  if (store) store.dispatch(action, ...args);
}

function mount(componentFn, container) {
  const el = document.querySelector(container);
  if (!el) return;
  const render = () => {
    const vdom = componentFn();
    el.innerHTML = renderVdom(vdom);
    bindEvents(el, vdom);
  };
  stores.forEach(store => store.subscribe(render));
  render();
}

function renderVdom(vdom) {
  if (!vdom) return '';
  const { type, content, value, style, children, align, inputType, placeholder } = vdom;
  const styleAttr = style ? ` class="${style}"` : '';
  const flexClass = align === 'horizontal' ? ' flex flex-row' : align === 'vertical' ? ' flex flex-col' : '';
  
  if (type === 'text') {
    return `<span${styleAttr}>${content || value || ''}</span>`;
  }
  if (type === 'button') {
    return `<button${styleAttr} data-click="true">${content || ''}</button>`;
  }
  if (type === 'input') {
    const inputTypeAttr = inputType || 'text';
    const placeholderAttr = placeholder ? ` placeholder="${placeholder}"` : '';
    const valueAttr = value !== undefined ? ` value="${value}"` : '';
    return `<input type="${inputTypeAttr}"${styleAttr}${placeholderAttr}${valueAttr} data-input="true" />`;
  }
  if (children) {
    const inner = children.map(c => typeof c === 'function' ? renderVdom(c()) : renderVdom(c)).join('');
    return `<div class="${(style || '') + flexClass}">${inner}</div>`;
  }
  return `<div${styleAttr}>${content || value || ''}</div>`;
}

function bindEvents(el, vdom) {
  el.querySelectorAll('[data-click]').forEach((btn, i) => {
    const handler = findClickHandler(vdom, i);
    if (handler) btn.onclick = handler;
  });
  el.querySelectorAll('[data-input]').forEach((input, i) => {
    const handler = findInputHandler(vdom, i);
    if (handler) input.oninput = (e) => handler(e.target.value);
  });
}

function findClickHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.click && count.n++ === index) return vdom.click;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findClickHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}

function findInputHandler(vdom, index, count = { n: 0 }) {
  if (!vdom) return null;
  if (vdom.input && count.n++ === index) return vdom.input;
  if (vdom.children) {
    for (const c of vdom.children) {
      const child = typeof c === 'function' ? c() : c;
      const h = findInputHandler(child, index, count);
      if (h) return h;
    }
  }
  return null;
}




function HeroTitle() {
  return {
    type: 'text',
    content: 'topo',
    style: 'text-7xl font-black text-gray-900 mb-4'
  };
}

function HeroTagline() {
  return {
    type: 'text',
    content: 'A UI framework that eliminates nesting hell',
    style: 'text-2xl text-gray-600 mb-6'
  };
}

function HeroDesc() {
  return {
    type: 'text',
    content: 'Write flat, readable UI code. No more callback pyramids or deeply nested components.',
    style: 'text-lg text-gray-500 mb-10 max-w-xl'
  };
}

function Hero() {
  return {
    style: 'mb-16',
    align: 'vertical',
    children: [HeroTitle, HeroTagline, HeroDesc]
  };
}

function Feature1Title() {
  return {
    type: 'text',
    content: 'Flat Definitions',
    style: 'text-xl font-semibold text-gray-800 mb-2'
  };
}

function Feature1Desc() {
  return {
    type: 'text',
    content: 'Define components at the top level. Compose by reference with children: [A, B, C]',
    style: 'text-gray-600'
  };
}

function Feature1() {
  return {
    style: 'bg-white p-6 rounded-xl shadow-sm border border-gray-100',
    align: 'vertical',
    children: [Feature1Title, Feature1Desc]
  };
}

function Feature2Title() {
  return {
    type: 'text',
    content: 'NgRx-style State',
    style: 'text-xl font-semibold text-gray-800 mb-2'
  };
}

function Feature2Desc() {
  return {
    type: 'text',
    content: 'Built-in state with Actions, Reducers, Effects, and Selectors',
    style: 'text-gray-600'
  };
}

function Feature2() {
  return {
    style: 'bg-white p-6 rounded-xl shadow-sm border border-gray-100',
    align: 'vertical',
    children: [Feature2Title, Feature2Desc]
  };
}

function Feature3Title() {
  return {
    type: 'text',
    content: 'Auto API Services',
    style: 'text-xl font-semibold text-gray-800 mb-2'
  };
}

function Feature3Desc() {
  return {
    type: 'text',
    content: 'Define REST endpoints, get CRUD methods auto-generated',
    style: 'text-gray-600'
  };
}

function Feature3() {
  return {
    style: 'bg-white p-6 rounded-xl shadow-sm border border-gray-100',
    align: 'vertical',
    children: [Feature3Title, Feature3Desc]
  };
}

function Features() {
  return {
    style: 'grid grid-cols-3 gap-6 mb-16',
    children: [Feature1, Feature2, Feature3]
  };
}

function SyntaxTitle() {
  return {
    type: 'text',
    content: '4 Definition Operators',
    style: 'text-3xl font-bold text-gray-900 mb-6'
  };
}

function Op1() {
  return {
    type: 'text',
    content: '->  Component (UI)',
    style: 'font-mono text-lg bg-blue-50 text-blue-700 px-4 py-2 rounded mb-2'
  };
}

function Op2() {
  return {
    type: 'text',
    content: '|   Store (State)',
    style: 'font-mono text-lg bg-purple-50 text-purple-700 px-4 py-2 rounded mb-2'
  };
}

function Op3() {
  return {
    type: 'text',
    content: '::  API Service',
    style: 'font-mono text-lg bg-green-50 text-green-700 px-4 py-2 rounded mb-2'
  };
}

function Op4() {
  return {
    type: 'text',
    content: '{}  Method (Logic)',
    style: 'font-mono text-lg bg-orange-50 text-orange-700 px-4 py-2 rounded mb-2'
  };
}

function Syntax() {
  return {
    style: 'mb-16',
    align: 'vertical',
    children: [SyntaxTitle, Op1, Op2, Op3, Op4]
  };
}

function ExampleTitle() {
  return {
    type: 'text',
    content: 'Example: Counter',
    style: 'text-3xl font-bold text-gray-900 mb-6'
  };
}

function ExampleCode() {
  return {
    type: 'text',
    content: 'Counter | {\n    State { count: 0 }\n    Actions { Increment, Decrement }\n    Reducers {\n        on Increment { count: count + 1 }\n        on Decrement { count: count - 1 }\n    }\n}\n\nDisplay -> {\n    type: text\n    value: $Counter.count\n}\n\nButton -> {\n    type: button\n    content: \"+\"\n    click: Counter.Increment\n}',
    style: 'font-mono text-sm bg-gray-900 text-green-400 p-6 rounded-xl whitespace-pre-wrap'
  };
}

function Example() {
  return {
    style: 'mb-16',
    align: 'vertical',
    children: [ExampleTitle, ExampleCode]
  };
}

function StartTitle() {
  return {
    type: 'text',
    content: 'Getting Started',
    style: 'text-3xl font-bold text-gray-900 mb-6'
  };
}

function Step1() {
  return {
    type: 'text',
    content: '1. Install:  cargo install --path .',
    style: 'font-mono bg-gray-100 px-4 py-3 rounded mb-3'
  };
}

function Step2() {
  return {
    type: 'text',
    content: '2. Create:   topo new my-app',
    style: 'font-mono bg-gray-100 px-4 py-3 rounded mb-3'
  };
}

function Step3() {
  return {
    type: 'text',
    content: '3. Run:      cd my-app && topo start',
    style: 'font-mono bg-gray-100 px-4 py-3 rounded mb-3'
  };
}

function Start() {
  return {
    style: 'mb-16',
    align: 'vertical',
    children: [StartTitle, Step1, Step2, Step3]
  };
}

function Footer() {
  return {
    type: 'text',
    content: 'Built with topo - MIT License',
    style: 'text-gray-400 text-sm mt-16 pt-8 border-t border-gray-200'
  };
}

function Content() {
  return {
    style: 'ml-56 p-12 bg-gray-50 min-h-screen',
    align: 'vertical',
    children: [Hero, Features, Syntax, Example, Start, Footer]
  };
}

function App() {
  return {
    style: 'font-sans',
    children: [Sidebar, Content]
  };
}

// Mount app
mount(App, '#app');

