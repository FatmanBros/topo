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

const LoginForm = createStore('LoginForm', {
  email: '',
  password: '',
  emailError: '',
  passwordError: '',
  isSubmitting: false
});

const LoginFormValidationRules = {
  email: [{ name: 'required' }, { name: 'email' }],
  password: [{ name: 'required' }, { name: 'minLength', args: [8] }]
};

function validateLoginForm(data) {
  return validate(data, LoginFormValidationRules);
}

LoginForm.on('SetEmail', (state, value) => ({
  ...state,
  email: value
}));

LoginForm.on('SetPassword', (state, value) => ({
  ...state,
  password: value
}));

LoginForm.on('SetEmailError', (state, msg) => ({
  ...state,
  emailError: msg
}));

LoginForm.on('SetPasswordError', (state, msg) => ({
  ...state,
  passwordError: msg
}));

LoginForm.on('SetSubmitting', (state, value) => ({
  ...state,
  isSubmitting: value
}));



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

const LoginForm = createStore('LoginForm', {
  email: '',
  password: '',
  emailError: '',
  passwordError: '',
  isSubmitting: false
});

const LoginFormValidationRules = {
  email: [{ name: 'required' }, { name: 'email' }],
  password: [{ name: 'required' }, { name: 'minLength', args: [8] }]
};

function validateLoginForm(data) {
  return validate(data, LoginFormValidationRules);
}

LoginForm.on('SetEmail', (state, value) => ({
  ...state,
  email: value
}));

LoginForm.on('SetPassword', (state, value) => ({
  ...state,
  password: value
}));

LoginForm.on('SetEmailError', (state, msg) => ({
  ...state,
  emailError: msg
}));

LoginForm.on('SetPasswordError', (state, msg) => ({
  ...state,
  passwordError: msg
}));

LoginForm.on('SetSubmitting', (state, value) => ({
  ...state,
  isSubmitting: value
}));


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

function Input(inputType, placeholder, value, onInput) {
  return {
    type: 'input',
    inputType: inputType,
    placeholder: placeholder,
    value: value,
    input: onInput,
    style: 'w-full px-4 py-3 border border-gray-300 rounded-lg focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500 outline-none transition'
  };
}

function Button(content, onClick, variant) {
  return {
    type: 'button',
    content: content,
    click: onClick,
    style: variant
  };
}

function PrimaryButton(content, onClick) {
  return {
    type: 'button',
    content: content,
    click: onClick,
    style: 'w-full bg-indigo-600 text-white py-3 px-6 rounded-lg font-semibold hover:bg-indigo-700 transition'
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

const LoginForm = createStore('LoginForm', {
  email: '',
  password: '',
  emailError: '',
  passwordError: '',
  isSubmitting: false
});

const LoginFormValidationRules = {
  email: [{ name: 'required' }, { name: 'email' }],
  password: [{ name: 'required' }, { name: 'minLength', args: [8] }]
};

function validateLoginForm(data) {
  return validate(data, LoginFormValidationRules);
}

LoginForm.on('SetEmail', (state, value) => ({
  ...state,
  email: value
}));

LoginForm.on('SetPassword', (state, value) => ({
  ...state,
  password: value
}));

LoginForm.on('SetEmailError', (state, msg) => ({
  ...state,
  emailError: msg
}));

LoginForm.on('SetPasswordError', (state, msg) => ({
  ...state,
  passwordError: msg
}));

LoginForm.on('SetSubmitting', (state, value) => ({
  ...state,
  isSubmitting: value
}));


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

function Input(inputType, placeholder, value, onInput) {
  return {
    type: 'input',
    inputType: inputType,
    placeholder: placeholder,
    value: value,
    input: onInput,
    style: 'w-full px-4 py-3 border border-gray-300 rounded-lg focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500 outline-none transition'
  };
}

function Button(content, onClick, variant) {
  return {
    type: 'button',
    content: content,
    click: onClick,
    style: variant
  };
}

function PrimaryButton(content, onClick) {
  return {
    type: 'button',
    content: content,
    click: onClick,
    style: 'w-full bg-indigo-600 text-white py-3 px-6 rounded-lg font-semibold hover:bg-indigo-700 transition'
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

function Label(text) {
  return {
    type: text,
    content: text,
    style: 'block text-sm font-medium text-gray-700 mb-2'
  };
}

function ErrorText(msg) {
  return {
    type: 'text',
    content: msg,
    style: 'text-red-500 text-sm mt-1'
  };
}

function FormField(label, inputType, placeholder, value, onInput, errorMsg) {
  return {
    style: 'mb-4',
    align: 'vertical',
    children: [Label(label), Input(inputType, placeholder, value, onInput), ErrorText(errorMsg)]
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

const LoginForm = createStore('LoginForm', {
  email: '',
  password: '',
  emailError: '',
  passwordError: '',
  isSubmitting: false
});

const LoginFormValidationRules = {
  email: [{ name: 'required' }, { name: 'email' }],
  password: [{ name: 'required' }, { name: 'minLength', args: [8] }]
};

function validateLoginForm(data) {
  return validate(data, LoginFormValidationRules);
}

LoginForm.on('SetEmail', (state, value) => ({
  ...state,
  email: value
}));

LoginForm.on('SetPassword', (state, value) => ({
  ...state,
  password: value
}));

LoginForm.on('SetEmailError', (state, msg) => ({
  ...state,
  emailError: msg
}));

LoginForm.on('SetPasswordError', (state, msg) => ({
  ...state,
  passwordError: msg
}));

LoginForm.on('SetSubmitting', (state, value) => ({
  ...state,
  isSubmitting: value
}));


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

function Input(inputType, placeholder, value, onInput) {
  return {
    type: 'input',
    inputType: inputType,
    placeholder: placeholder,
    value: value,
    input: onInput,
    style: 'w-full px-4 py-3 border border-gray-300 rounded-lg focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500 outline-none transition'
  };
}

function Button(content, onClick, variant) {
  return {
    type: 'button',
    content: content,
    click: onClick,
    style: variant
  };
}

function PrimaryButton(content, onClick) {
  return {
    type: 'button',
    content: content,
    click: onClick,
    style: 'w-full bg-indigo-600 text-white py-3 px-6 rounded-lg font-semibold hover:bg-indigo-700 transition'
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

function Label(text) {
  return {
    type: text,
    content: text,
    style: 'block text-sm font-medium text-gray-700 mb-2'
  };
}

function ErrorText(msg) {
  return {
    type: 'text',
    content: msg,
    style: 'text-red-500 text-sm mt-1'
  };
}

function FormField(label, inputType, placeholder, value, onInput, errorMsg) {
  return {
    style: 'mb-4',
    align: 'vertical',
    children: [Label(label), Input(inputType, placeholder, value, onInput), ErrorText(errorMsg)]
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




function LoginTitle() {
  return {
    type: 'text',
    content: 'Sign In',
    style: 'text-2xl font-bold text-gray-900 mb-6 text-center'
  };
}

function EmailField() {
  return {
    style: 'mb-4',
    align: 'vertical',
    children: [Label('Email'), Input('email', 'Enter your email', LoginForm.state.email, LoginForm.SetEmail)]
  };
}

function PasswordField() {
  return {
    style: 'mb-6',
    align: 'vertical',
    children: [Label('Password'), Input('password', 'Enter your password', LoginForm.state.password, LoginForm.SetPassword)]
  };
}

function SubmitButton() {
  return {
    type: 'button',
    content: 'Sign In',
    click: () => dispatch('LoginForm', 'Submit'),
    style: 'w-full bg-indigo-600 text-white py-3 px-6 rounded-lg font-semibold hover:bg-indigo-700 transition cursor-pointer'
  };
}

function DebugInfo() {
  return {
    type: 'text',
    content: LoginForm.state.email,
    style: 'text-sm text-gray-500 mt-4 text-center'
  };
}

function LoginFormCard() {
  return {
    style: 'bg-white p-8 rounded-xl shadow-lg max-w-md w-full',
    align: 'vertical',
    children: [LoginTitle, EmailField, PasswordField, SubmitButton, DebugInfo]
  };
}

function App() {
  return {
    style: 'min-h-screen bg-gradient-to-br from-indigo-100 to-purple-100 flex items-center justify-center p-4',
    children: [LoginFormCard]
  };
}

// Mount app
mount(App, '#app');

