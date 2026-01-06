/**
 * topo-runtime
 * Minimal runtime for topo framework
 */

// ============================================================================
// Reactive State Management
// ============================================================================

type Subscriber = () => void;

interface Store<T extends object> {
  name: string;
  state: T;
  subscribe: (fn: Subscriber) => () => void;
  on: (action: string, reducer: (state: T, payload?: any) => T) => void;
  effect: (action: string, handler: (payload?: any) => Promise<void>) => void;
  selector: (name: string, fn: (state: T) => any) => void;
}

const stores: Map<string, Store<any>> = new Map();
const subscribers: Map<string, Set<Subscriber>> = new Map();
const reducers: Map<string, Map<string, (state: any, payload?: any) => any>> = new Map();
const effects: Map<string, Map<string, (payload?: any) => Promise<void>>> = new Map();
const selectors: Map<string, Map<string, (state: any) => any>> = new Map();

export function createStore<T extends object>(name: string, initialState: T): Store<T> {
  const state = new Proxy(initialState, {
    set(target, prop, value) {
      (target as any)[prop] = value;
      notifySubscribers(name);
      return true;
    }
  });

  subscribers.set(name, new Set());
  reducers.set(name, new Map());
  effects.set(name, new Map());
  selectors.set(name, new Map());

  const store: Store<T> = {
    name,
    state,
    subscribe(fn: Subscriber) {
      subscribers.get(name)!.add(fn);
      return () => subscribers.get(name)!.delete(fn);
    },
    on(action: string, reducer: (state: T, payload?: any) => T) {
      reducers.get(name)!.set(action, reducer);
    },
    effect(action: string, handler: (payload?: any) => Promise<void>) {
      effects.get(name)!.set(action, handler);
    },
    selector(selectorName: string, fn: (state: T) => any) {
      selectors.get(name)!.set(selectorName, fn);
      // Add getter to state
      Object.defineProperty(state, selectorName, {
        get: () => fn(state),
        enumerable: false
      });
    }
  };

  stores.set(name, store);
  return store;
}

function notifySubscribers(storeName: string) {
  const subs = subscribers.get(storeName);
  if (subs) {
    subs.forEach(fn => fn());
  }
}

export function dispatch(storeName: string, action: string, payload?: any) {
  const store = stores.get(storeName);
  if (!store) {
    console.error(`Store "${storeName}" not found`);
    return;
  }

  // Run reducer
  const reducer = reducers.get(storeName)?.get(action);
  if (reducer) {
    const newState = reducer(store.state, payload);
    Object.assign(store.state, newState);
  }

  // Run effect
  const effect = effects.get(storeName)?.get(action);
  if (effect) {
    effect(payload);
  }
}

// ============================================================================
// Reactive References
// ============================================================================

export function ref<T>(selector: () => T): { value: T } {
  let currentValue = selector();

  // Subscribe to all stores for simplicity
  stores.forEach((store) => {
    store.subscribe(() => {
      currentValue = selector();
    });
  });

  return {
    get value() {
      return selector();
    }
  };
}

export function computed<T>(fn: () => T): { value: T } {
  return ref(fn);
}

export function effect(fn: () => void | (() => void)) {
  let cleanup: (() => void) | void;

  const run = () => {
    if (cleanup) cleanup();
    cleanup = fn();
  };

  run();

  // Subscribe to all stores
  stores.forEach((store) => {
    store.subscribe(run);
  });
}

// ============================================================================
// Component Rendering
// ============================================================================

interface ComponentDef {
  type?: string;
  style?: string;
  content?: string;
  value?: any;
  children?: (ComponentDef | (() => ComponentDef))[];
  click?: () => void;
  change?: (value: any) => void;
  [key: string]: any;
}

export function mount(component: () => ComponentDef, element: HTMLElement) {
  const render = () => {
    const def = component();
    element.innerHTML = '';
    element.appendChild(createElement(def));
  };

  // Initial render
  render();

  // Subscribe to updates
  stores.forEach((store) => {
    store.subscribe(render);
  });
}

function createElement(def: ComponentDef): HTMLElement {
  const type = def.type || 'container';

  let el: HTMLElement;

  switch (type) {
    case 'text':
      el = document.createElement('span');
      el.textContent = def.content || def.value?.toString() || '';
      break;

    case 'button':
      el = document.createElement('button');
      el.textContent = def.content || '';
      if (def.click) {
        el.addEventListener('click', def.click);
      }
      break;

    case 'textbox':
      el = document.createElement('input');
      (el as HTMLInputElement).type = 'text';
      (el as HTMLInputElement).value = def.value?.toString() || '';
      (el as HTMLInputElement).placeholder = def.placeholder || '';
      if (def.change) {
        el.addEventListener('input', (e) => {
          def.change!((e.target as HTMLInputElement).value);
        });
      }
      break;

    case 'container':
    default:
      el = document.createElement('div');
      if (def.align === 'horizontal') {
        el.style.display = 'flex';
        el.style.flexDirection = 'row';
      } else if (def.align === 'vertical') {
        el.style.display = 'flex';
        el.style.flexDirection = 'column';
      }
      break;
  }

  // Apply style (Tailwind classes)
  if (def.style) {
    el.className = def.style;
  }

  // Render children
  if (def.children) {
    for (const child of def.children) {
      if (typeof child === 'function') {
        el.appendChild(createElement(child()));
      } else {
        el.appendChild(createElement(child));
      }
    }
  }

  return el;
}

// ============================================================================
// Exports
// ============================================================================

export { stores, dispatch as dispatchAction };
