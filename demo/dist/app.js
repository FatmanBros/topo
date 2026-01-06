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
  regex(value, args, field) {
    const pattern = new RegExp(args[0]);
    const customMsg = args[1];
    if (typeof value === 'string' && !pattern.test(value)) {
      return { valid: false, error: customMsg || `${field} does not match the required format` };
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
    // Use routed page if available, otherwise use provided component
    const page = currentPage || componentFn;
    const vdom = typeof page === 'function' ? page() : page;
    el.innerHTML = renderVdom(vdom);
    bindEvents(el, vdom);
  };
  stores.forEach(store => store.subscribe && store.subscribe(render));
  // Re-render on route change
  window.addEventListener('popstate', () => { updateRoute(); render(); });
  // Make render accessible for navigation
  __rerender = render;
  // Initial route setup and render
  updateRoute();
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
  if (type === 'link') {
    const href = vdom.href || '#';
    return `<a href="${href}"${styleAttr} data-link="true">${content || ''}</a>`;
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
  el.querySelectorAll('[data-link]').forEach((link) => {
    link.onclick = (e) => {
      e.preventDefault();
      navigate(link.getAttribute('href'));
    };
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

// Router
const routeState = { path: '/', params: {}, query: {} };
const routes = [];
let currentPage = null;
let __rerender = () => {};

function registerRoute(pattern, component) {
  const paramNames = [];
  const regexPattern = pattern.replace(/\[([^\]]+)\]/g, (_, name) => {
    if (name.startsWith('...')) {
      paramNames.push(name.slice(3));
      return '(.*)';
    }
    paramNames.push(name);
    return '([^/]+)';
  });
  routes.push({ pattern: new RegExp(`^${regexPattern}$`), paramNames, component });
}

function matchRoute(path) {
  for (const route of routes) {
    const match = path.match(route.pattern);
    if (match) {
      const params = {};
      route.paramNames.forEach((name, i) => { params[name] = match[i + 1]; });
      return { component: route.component, params };
    }
  }
  return null;
}

function parseQuery(search) {
  const query = {};
  if (search) {
    new URLSearchParams(search).forEach((v, k) => { query[k] = v; });
  }
  return query;
}

function navigate(path) {
  history.pushState(null, '', path);
  updateRoute();
  __rerender();
}

function updateRoute() {
  const path = location.pathname;
  const matched = matchRoute(path);
  routeState.path = path;
  routeState.query = parseQuery(location.search);
  if (matched) {
    routeState.params = matched.params;
    currentPage = matched.component;
  } else {
    routeState.params = {};
    currentPage = null;
  }
  stores.forEach(store => store.dispatch && store.dispatch('__routeChange'));
}

const Router = {
  get state() { return routeState; },
  Navigate: navigate,
  subscribe(fn) { /* handled by stores */ }
};
stores.set('Router', Router);

const $route = routeState;

window.addEventListener('popstate', updateRoute);
document.addEventListener('DOMContentLoaded', updateRoute);


// i18n Internationalization
const __i18n = {
  locale: 'ja',
  locales: ["ja", "en"],
  translations: {
    'password_placeholder': {
      'ja': 'パスワードを入力',
      'en': 'Enter your password',
    },
    'email_placeholder': {
      'ja': 'メールアドレスを入力',
      'en': 'Enter your email',
    },
    'about': {
      'ja': 'topoについて',
      'en': 'About topo',
    },
    'sign_in': {
      'ja': 'サインイン',
      'en': 'Sign In',
    },
    'email': {
      'en': 'Email',
      'ja': 'メールアドレス',
    },
    'users': {
      'en': 'Users',
      'ja': 'ユーザー',
    },
    'password': {
      'ja': 'パスワード',
      'en': 'Password',
    },
    'back_home': {
      'en': 'Back to Home',
      'ja': 'ホームに戻る',
    },
    'welcome': {
      'en': 'Welcome to topo!',
      'ja': 'topoへようこそ！',
    },
  },
  subscribers: [],
};

function t(key, params = {}) {
  const translation = __i18n.translations[key];
  if (!translation) return key;
  let text = translation[__i18n.locale] || translation[Object.keys(translation)[0]] || key;
  // Replace {{param}} placeholders
  for (const [k, v] of Object.entries(params)) {
    text = text.replace(new RegExp(`{{${k}}}`, 'g'), v);
  }
  return text;
}

const $i18n = {
  get locale() { return __i18n.locale; },
  get locales() { return __i18n.locales; },
  setLocale(locale) {
    if (__i18n.locales.includes(locale)) {
      __i18n.locale = locale;
      __i18n.subscribers.forEach(fn => fn());
      __rerender();
    }
  },
  subscribe(fn) { __i18n.subscribers.push(fn); },
};
stores.set('i18n', $i18n);


// File-based routes
registerRoute('/', AppPage);
registerRoute('/about', AboutPage);
registerRoute('/login', LoginPage);
registerRoute('/users', UsersPage);
registerRoute('/users/[id]', UsersDetailPage);

function UserDetailTitle() {
  return {
    type: 'text',
    content: 'User Detail',
    style: 'text-3xl font-bold text-gray-900 mb-4'
  };
}

function UserIdDisplay() {
  return {
    type: 'text',
    content: $route.params.id,
    style: 'text-6xl font-mono text-blue-600 mb-4'
  };
}

function UserIdLabel() {
  return {
    type: 'text',
    content: 'User ID from URL parameter',
    style: 'text-sm text-gray-500 mb-8'
  };
}

function BackToUsersLink() {
  return {
    type: 'link',
    href: '/users',
    content: 'Back to Users',
    style: 'px-4 py-2 bg-gray-500 text-white rounded hover:bg-gray-600'
  };
}

function UsersDetailPage() {
  return {
    style: 'min-h-screen flex flex-col items-center justify-center bg-gradient-to-br from-purple-50 to-violet-100',
    align: 'vertical',
    children: [UserDetailTitle, UserIdDisplay, UserIdLabel, BackToUsersLink]
  };
}


function UsersTitle() {
  return {
    type: 'text',
    content: 'Users',
    style: 'text-3xl font-bold text-gray-900 mb-4'
  };
}

function UsersList() {
  return {
    style: 'flex flex-col gap-2 mb-8',
    children: [User1Link, User2Link, User3Link]
  };
}

function User1Link() {
  return {
    type: 'link',
    href: '/users/1',
    content: 'User 1',
    style: 'px-4 py-2 bg-white rounded shadow hover:bg-gray-50'
  };
}

function User2Link() {
  return {
    type: 'link',
    href: '/users/2',
    content: 'User 2',
    style: 'px-4 py-2 bg-white rounded shadow hover:bg-gray-50'
  };
}

function User3Link() {
  return {
    type: 'link',
    href: '/users/3',
    content: 'User 3',
    style: 'px-4 py-2 bg-white rounded shadow hover:bg-gray-50'
  };
}

function BackHomeLink() {
  return {
    type: 'link',
    href: '/',
    content: 'Back to Home',
    style: 'px-4 py-2 bg-gray-500 text-white rounded hover:bg-gray-600'
  };
}

function UsersPage() {
  return {
    style: 'min-h-screen flex flex-col items-center justify-center bg-gradient-to-br from-orange-50 to-amber-100',
    align: 'vertical',
    children: [UsersTitle, UsersList, BackHomeLink]
  };
}


function HomeTitle() {
  return {
    type: 'text',
    content: 'Welcome to topo!',
    style: 'text-4xl font-bold text-gray-900 mb-4'
  };
}

function HomeDescription() {
  return {
    type: 'text',
    content: 'A UI framework that eliminates nesting hell',
    style: 'text-lg text-gray-600 mb-8'
  };
}

function NavLinks() {
  return {
    style: 'flex gap-4',
    children: [AboutLink, UsersLink, LoginLink]
  };
}

function AboutLink() {
  return {
    type: 'link',
    href: '/about',
    content: 'About',
    style: 'px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600'
  };
}

function UsersLink() {
  return {
    type: 'link',
    href: '/users',
    content: 'Users',
    style: 'px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600'
  };
}

function LoginLink() {
  return {
    type: 'link',
    href: '/login',
    content: 'Login',
    style: 'px-4 py-2 bg-purple-500 text-white rounded hover:bg-purple-600'
  };
}

function CurrentPath() {
  return {
    type: 'text',
    content: $route.path,
    style: 'mt-8 text-sm text-gray-400'
  };
}

function AppPage() {
  return {
    style: 'min-h-screen flex flex-col items-center justify-center bg-gradient-to-br from-blue-50 to-indigo-100',
    align: 'vertical',
    children: [HomeTitle, HomeDescription, NavLinks, CurrentPath]
  };
}


const LoginForm = createStore('LoginForm', {
  email: '',
  password: '',
  isSubmitting: false,
  emailError: '',
  passwordError: ''
});

const LoginFormValidationRules = {
  email: [{ name: 'required' }, { name: 'email' }],
  password: [{ name: 'required' }, { name: 'minLength', args: [8] }]
};

const LoginFormFieldLabels = {
  email: 'メールアドレス',
  password: 'パスワード',
  isSubmitting: 'isSubmitting'
};

LoginForm.labels = LoginFormFieldLabels;
function validateLoginForm(data) {
  const errors = {};
  for (const [field, fieldRules] of Object.entries(LoginFormValidationRules)) {
    const value = data[field];
    const label = LoginFormFieldLabels[field] || field;
    for (const rule of fieldRules) {
      const validator = validators[rule.name];
      if (validator) {
        const result = validator(value, rule.args || [], label);
        if (!result.valid) {
          errors[field] = errors[field] || [];
          errors[field].push(result.error);
        }
      }
    }
  }
  return { valid: Object.keys(errors).length === 0, errors };
}

LoginForm.on('SetEmail', (state, value) => {
  const result = validateLoginForm({ ...state, email: value });
  const error = result.errors.email ? result.errors.email[0] : '';
  return {
    ...state,
    email: value,
    emailError: error
  };
});

LoginForm.on('SetPassword', (state, value) => {
  const result = validateLoginForm({ ...state, password: value });
  const error = result.errors.password ? result.errors.password[0] : '';
  return {
    ...state,
    password: value,
    passwordError: error
  };
});

LoginForm.on('Submit', (state) => {
  const result = validateLoginForm(state);
  return {
    ...state,
    emailError: result.errors.email ? result.errors.email[0] : '',
    passwordError: result.errors.password ? result.errors.password[0] : ''
  };
});

LoginForm.on('SetSubmitting', (state, value) => ({
  ...state,
  isSubmitting: value
}));



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




function LoginTitle() {
  return {
    type: 'text',
    content: t('sign_in'),
    style: 'text-2xl font-bold text-gray-900 mb-6 text-center'
  };
}

function SubmitButton() {
  return {
    type: 'button',
    content: t('sign_in'),
    click: () => dispatch('LoginForm', 'Submit'),
    style: 'w-full bg-indigo-600 text-white py-3 px-6 rounded-lg font-semibold hover:bg-indigo-700 transition cursor-pointer'
  };
}

function LangJa() {
  return {
    type: 'button',
    content: '日本語',
    click: () => $i18n.setLocale('ja'),
    style: 'px-3 py-1 text-sm rounded bg-gray-200 hover:bg-gray-300 mr-2'
  };
}

function LangEn() {
  return {
    type: 'button',
    content: 'English',
    click: () => $i18n.setLocale('en'),
    style: 'px-3 py-1 text-sm rounded bg-gray-200 hover:bg-gray-300'
  };
}

function LangSwitcher() {
  return {
    style: 'flex justify-center mb-4',
    align: 'horizontal',
    children: [LangJa, LangEn]
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
    children: [LangSwitcher, LoginTitle, FormField(t('email'), 'email', t('email_placeholder'), LoginForm.state.email, (value) => dispatch('LoginForm', 'SetEmail', value), LoginForm.state.emailError), FormField(t('password'), 'password', t('password_placeholder'), LoginForm.state.password, (value) => dispatch('LoginForm', 'SetPassword', value), LoginForm.state.passwordError), SubmitButton, DebugInfo]
  };
}

function LoginApp() {
  return {
    style: 'min-h-screen bg-gradient-to-br from-indigo-100 to-purple-100 flex items-center justify-center p-4',
    children: [LoginFormCard]
  };
}



function LoginPage() {
  return {
    children: [LoginApp]
  };
}


function AboutTitle() {
  return {
    type: 'text',
    content: 'About topo',
    style: 'text-3xl font-bold text-gray-900 mb-4'
  };
}

function AboutContent() {
  return {
    type: 'text',
    content: 'topo is a declarative UI framework built with Rust.',
    style: 'text-lg text-gray-600 mb-8'
  };
}

function BackLink() {
  return {
    type: 'link',
    href: '/',
    content: 'Back to Home',
    style: 'px-4 py-2 bg-gray-500 text-white rounded hover:bg-gray-600'
  };
}

function AboutPage() {
  return {
    style: 'min-h-screen flex flex-col items-center justify-center bg-gradient-to-br from-green-50 to-emerald-100',
    align: 'vertical',
    children: [AboutTitle, AboutContent, BackLink]
  };
}


