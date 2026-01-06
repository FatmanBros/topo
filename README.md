# Topo

[日本語](./README.ja.md)

A UI framework that eliminates nesting hell.

## Overview

Topo is a Rust-based UI framework with a declarative DSL that compiles to JavaScript. It provides Angular-like reactive development without the nested component structure of React/Vue.

```tp
// Flat declarations instead of nested JSX
Header -> { type: text, content: "Hello" }
Button(label) -> { type: button, content: label, style: "px-4 py-2 bg-blue-500" }

App -> {
  children: [Header, Button("Click me")]
}
```

## Features

- **Flat DSL** - No nesting hell. Declare components at module level, compose by reference
- **4 Definition Operators**
  - `->` Component (UI definition)
  - `|` Store (NgRx-style state management)
  - `::` API Service (REST client generation)
  - `{}` Method (logic/functions)
- **File-based Routing** - `pages/users/[id].tp` → `/users/:id`
- **Validation Annotations** - `@required`, `@email`, `@minLength(8)`
- **Tailwind CSS** - Built-in support
- **i18n** - Internationalization support

## Installation

```bash
# Clone and build
git clone https://github.com/yourname/topo.git
cd topo
cargo build --release

# Add to PATH
export PATH="$PATH:$(pwd)/target/release"
```

## Quick Start

```bash
# Create new project
topo new my-app
cd my-app

# Start dev server
topo dev
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `topo new <name>` | Create new project |
| `topo init` | Initialize in current directory |
| `topo build` | Compile to JavaScript |
| `topo dev` | Dev server with file watching |
| `topo start` | Build and serve |
| `topo test` | Run E2E tests (Playwright) |

## Example

### Component

```tp
LoginButton -> {
  type: button
  content: t("sign_in")
  click: Auth.Login
  style: "px-4 py-2 bg-blue-600 text-white rounded"
}
```

### Store

```tp
Auth | {
  State {
    @required @email
    email: ""
    @required @minLength(8)
    password: ""
    isLoading: false
  }

  Actions {
    Login
    SetEmail(value)
  }

  Reducers {
    on SetEmail(value) { email: value }
  }

  Effects {
    on Login {
      await AuthApi.login($Auth)
    }
  }
}
```

### API Service

```tp
AuthApi :: {
  rest: "/api/auth"
  login: post("/login")
}
```

## Project Structure

```
my-app/
├── topo.config.json    # Configuration
├── pages/
│   ├── index.tp        # → /
│   ├── about.tp        # → /about
│   └── users/
│       └── [id].tp     # → /users/:id
└── components/
    └── atoms/
    └── molecules/
    └── organisms/
```

## License

MIT
