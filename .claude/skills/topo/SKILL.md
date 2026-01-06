# topo Implementation Skill

topo is a UI framework that eliminates nesting hell. This skill provides guidance for implementing features in the topo language and compiler.

## Language Syntax

### Component Definition (`->`)
```tp
ComponentName(param1, param2) -> {
    type: text | button | input | select | textarea | submit | link
    content: "text" | expression
    style: "tailwind classes"
    children: [Child1, Child2]
    align: horizontal | vertical
    click: Store.Action
    input: Store.SetField
    value: $Store.field
}
```

### Store Definition (`|`)
```tp
StoreName | {
    State {
        field: initialValue
        @required @email
        email: ""
        emailError: ""
    }
    Actions {
        SetField(value)
        Submit()
    }
    Reducers {
        on SetField(value) {
            field: value
        }
    }
    Effects {
        on Submit() {
            // async operations
        }
    }
}
```

### API Service Definition (`::`)
```tp
ApiName :: {
    rest: "/api/resource"
    get getAll() -> "/"
    get getById(id) -> "/{id}"
    post create(data) -> "/"
    put update(id, data) -> "/{id}"
    delete remove(id) -> "/{id}"
}
```

### Test Definition (`Test`)
```tp
Test "テスト名" {
    goto "/path"
    fill $Store.field "value"
    click submit
    expect $Store.fieldError visible
    expect url "/"
    mock ApiService.method -> { data: "mock" }
    wait 500
}
```

### Theme Definition (`*`)
```tp
ThemeName * {
    --primary: "#3b82f6"
    --secondary: "#6b7280"
}
```

## Key Concepts

### Store References
- `$Store.field` - Read state value
- `Store.Action` - Dispatch action
- `Store.Action(args)` - Dispatch with arguments

### Auto-generated Features
- Validation annotations: `@required`, `@email`, `@min(n)`, `@max(n)`, `@pattern("regex")`, `@label("Label")`
- Auto-generated `Set{Field}` actions with validation
- Auto-generated `Submit` action that validates all fields
- Auto-generated error fields (`fieldError`) for each validated field

### Data Attributes
- `data-field="Store.field"` - For test selectors
- `data-error="Store.fieldError"` - For error elements
- `data-bind="Store.field"` - For bound text elements

### Silent Dispatch (dispatchField)
For input handlers, use `dispatchField` to update state without full re-render:
- Updates store silently via `dispatchSilent`
- Directly updates DOM elements with matching `data-error` attributes
- Preserves input focus

## File Structure

```
project/
├── topo.config.json      # Project configuration
├── demo/                 # Source files
│   ├── index.tp          # Entry point
│   ├── pages/            # File-based routing
│   │   ├── index.tp      # /
│   │   ├── login.tp      # /login
│   │   └── users/
│   │       ├── index.tp  # /users
│   │       └── [id].tp   # /users/:id (dynamic)
│   └── home/
│       ├── atoms/        # Atomic components
│       ├── molecules/    # Molecule components
│       ├── organisms/    # Organism components
│       └── stores/       # State stores
├── dist/                 # Build output
└── tests/                # Generated Playwright tests
```

## Configuration (topo.config.json)

```json
{
  "project": { "name": "app", "version": "0.1.0" },
  "build": { "mode": "spa", "output": "dist" },
  "dev": { "port": 7090 },
  "paths": { "pages": "demo/pages" },
  "style": {
    "tailwind": { "enabled": true, "cdn": true }
  },
  "i18n": {
    "locales": ["ja", "en"],
    "defaultLocale": "ja",
    "translations": {
      "key": { "ja": "日本語", "en": "English" }
    }
  }
}
```

## CLI Commands

```bash
topo build              # Build project
topo start              # Build and serve
topo dev                # Development server
topo test               # Run Playwright tests (headless)
topo test --headed      # Run with browser visible
topo test --ui          # Open Playwright UI
topo parse file.tp      # Debug: show AST
topo check              # Check for errors
```

## Implementation Guide

### Adding New Element Types
1. Add type to `renderVdom()` in `src/codegen/mod.rs`
2. Add event binding in `bindEvents()` if interactive
3. Update `findInputHandler()` or `findClickHandler()` as needed

### Adding New AST Nodes
1. Define struct in `src/ast/mod.rs`
2. Add variant to `Declaration` or relevant enum
3. Add token if needed in `src/lexer/token.rs`
4. Add keyword mapping in `src/lexer/mod.rs`
5. Add parser in `src/parser/mod.rs`
6. Handle in `src/codegen/mod.rs`

### Adding Validation Annotations
1. Add annotation name check in `generate_store()`
2. Generate validation logic in reducer
3. Update error field accordingly

### Component Parameters Auto-fill
In `generate_expression()` for `Call`:
- `dataError` - Auto-filled from error Reference props
- `dataField` - Auto-filled from value Reference props

## Testing DSL

### Test Targets
- `$Store.field` → `[data-field="Store.field"]`
- `submit` → `button[type="submit"]`
- `text "content"` → `text=content`
- `button "label"` → `button:has-text("label")`
- `url` → Special handling for URL assertions

### Test Assertions
- `visible` → `toBeVisible()`
- `hidden` → `toBeHidden()`
- `"value"` → `toHaveText("value")` or `toHaveURL("value")`

## i18n

```tp
// Use translation
content: t("key")

// Switch locale
click: $i18n.setLocale("en")
```

## Common Patterns

### Form Field Component
```tp
FormField(label, inputType, placeholder, value, onInput, errorMsg, dataError, dataField) -> {
    align: vertical
    children: [
        Label(label),
        Input(inputType, placeholder, value, onInput, dataField),
        ErrorText(errorMsg, dataError)
    ]
}
```

### Login Form with Validation
```tp
LoginForm | {
    State {
        @required @email @label("メールアドレス")
        email: ""
        emailError: ""
        @required @min(8) @label("パスワード")
        password: ""
        passwordError: ""
    }
}
```
