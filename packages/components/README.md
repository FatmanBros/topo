# @topo-ui/components

Atomic Design component library for topo-ui.

## Installation

```bash
npm install @topo-ui/components
```

## Usage

```topo
import { Button, Card, Text } from "@topo-ui/components"

MyPage -> {
    Card({
        children: [
            Text({ content: "Hello World" }),
            Button({ text: "Click me" })
        ]
    })
}
```

## Components

### Atoms
- Button, Text, Heading, Card, Badge, Input, etc.

### Molecules
- StatCard, FormField, NavItem, etc.

### Organisms
- LoginForm, Sidebar, Header, etc.

### Templates
- DashboardLayout, AuthLayout, etc.

## License

MIT
