# topo Framework - 詳細設計書

## 概要

**topo** は「ネスト地獄を解消する」ことを目的としたUIフレームワーク。

- **実装言語**: Rust → JS出力
- **出力形式**: SPA / SSG / SSR 対応
- **ファイル拡張子**: `.tp`
- **設計思想**: フラット宣言 + 参照による組み立て + トポロジカルソートで順序解決

---

## 1. DSL構文仕様

### 1.1 基本構文（4種類の定義演算子）

| 構文 | 用途 | 例 |
|------|------|-----|
| `Name -> { }` | コンポーネント（UI定義） | `Button -> { type: button }` |
| `Name { }` | メソッド（ロジック） | `double { value * 2 }` |
| `Name :: { }` | APIサービス | `User :: { rest: "/api/users" }` |
| `Name \| { }` | ストア（状態管理） | `User \| { State, Actions, Reducers }` |

```tp
// コンポーネント定義（UI）
Button -> {
  type: button
  content: "Click"
}

// メソッド定義（ロジック）
double { value * 2 }

// APIサービス定義
User :: {
  rest: "/api/users"
}

// ストア定義（状態管理）
User | {
  State { items: [] }
  Actions { Load, LoadSuccess(items) }
}
```

- `Name`: PascalCase（コンポーネント/サービス/ストア名）またはcamelCase（メソッド名）
- `property: value`: プロパティ定義（コロン記法）
- **同名連携**: `User ::` と `User |` を同名で定義すると自動的に連携される

### 1.2 コメント

```tp
// 単一行コメント

/*
  複数行コメント
*/
```

### 1.3 値の型

| 型 | 例 |
|----|-----|
| 文字列 | `"Hello World"` |
| 数値 | `42`, `3.14` |
| 真偽値 | `true`, `false` |
| 配列 | `[A, B, C]` |
| 参照 | `$Store.value` |
| アクション参照 | `Store.Action` |
| null | `null` |

### 1.4 予約キーワード

```
Store, Actions, Reducers, Effects, on, if, else, for, import, export
```

---

## 2. コンポーネント定義

### 2.1 基本形

```tp
ComponentName -> {
  type: primitive_type
  property: value
}
```

### 2.2 子要素の参照

```tp
Parent -> {
  children: [ChildA, ChildB, ChildC]
}
```

- `children`: 子コンポーネントを配列で参照
- 定義順序は自由（トポロジカルソートで解決）
- 同じコンポーネントを複数箇所で再利用可能

### 2.3 レイアウトプロパティ

| プロパティ | 値 | 説明 |
|-----------|-----|------|
| `align` | `horizontal` / `vertical` | 子要素の並び方向（デフォルト: horizontal） |
| `order` | `before X` / `after X` | 順序の上書き |
| `style` | Tailwind classes | スタイル指定 |

### 2.4 コンポーネントの再利用

```tp
// 定義
PrimaryButton -> {
  type: button
  style: "px-4 py-2 bg-blue-500 text-white rounded"
}

// 複数箇所で参照可能
FormA -> {
  children: [Input, PrimaryButton]
}

FormB -> {
  children: [TextArea, PrimaryButton]
}
```

### 2.5 プロパティの上書き（インスタンス化）

```tp
// ベース定義
BaseButton -> {
  type: button
  style: "px-4 py-2 rounded"
}

// 派生（上書き）
SubmitButton -> {
  extends: BaseButton
  content: "Submit"
  style: "px-4 py-2 rounded bg-green-500"
}
```

---

## 3. UIプリミティブ一覧

### 3.1 基本要素

| type | 説明 | 主要プロパティ |
|------|------|----------------|
| `text` | テキスト表示 | `content`, `value` |
| `button` | ボタン | `content`, `click`, `disabled` |
| `textbox` | テキスト入力 | `value`, `placeholder`, `change` |
| `textarea` | 複数行入力 | `value`, `placeholder`, `rows` |
| `checkbox` | チェックボックス | `checked`, `change`, `label` |
| `radio` | ラジオボタン | `checked`, `change`, `name`, `label` |
| `select` | ドロップダウン | `value`, `options`, `change` |
| `image` | 画像 | `src`, `alt`, `width`, `height` |
| `link` | リンク | `href`, `content`, `target` |
| `container` | 汎用コンテナ | `children`, `align`, `style` |

### 3.2 フォーム要素

| type | 説明 | 主要プロパティ |
|------|------|----------------|
| `form` | フォーム | `submit`, `children` |
| `label` | ラベル | `for`, `content` |
| `slider` | スライダー | `value`, `min`, `max`, `step` |
| `toggle` | トグルスイッチ | `checked`, `change` |
| `datepicker` | 日付選択 | `value`, `format`, `change` |
| `timepicker` | 時間選択 | `value`, `format`, `change` |
| `file` | ファイル選択 | `accept`, `multiple`, `change` |

### 3.3 データ表示

| type | 説明 | 主要プロパティ |
|------|------|----------------|
| `list` | リスト | `items`, `render`, `key` |
| `table` | テーブル | `columns`, `data`, `sortable` |
| `badge` | バッジ | `content`, `variant` |
| `avatar` | アバター | `src`, `name`, `size` |
| `progress` | プログレスバー | `value`, `max` |
| `spinner` | ローディング | `size` |

### 3.4 レイアウト

| type | 説明 | 主要プロパティ |
|------|------|----------------|
| `container` | コンテナ | `children`, `align` |
| `grid` | グリッド | `columns`, `gap`, `children` |
| `stack` | スタック | `direction`, `gap`, `children` |
| `divider` | 区切り線 | `orientation` |
| `spacer` | スペーサー | `size` |

### 3.5 フィードバック

| type | 説明 | 主要プロパティ |
|------|------|----------------|
| `alert` | アラート | `content`, `variant`, `dismissible` |
| `toast` | トースト | `content`, `duration`, `position` |
| `tooltip` | ツールチップ | `content`, `position` |
| `modal` | モーダル | `open`, `onClose`, `children` |
| `drawer` | ドロワー | `open`, `position`, `children` |

### 3.6 ナビゲーション

| type | 説明 | 主要プロパティ |
|------|------|----------------|
| `tabs` | タブ | `items`, `active`, `change` |
| `accordion` | アコーディオン | `items`, `multiple` |
| `breadcrumb` | パンくず | `items` |
| `pagination` | ページネーション | `total`, `current`, `change` |
| `menu` | メニュー | `items`, `children` |

---

## 4. APIサービス（:: 構文）

### 4.1 REST API 自動生成

```tp
User :: {
  rest: "/api/users"
}
```

`rest:` を指定すると、以下のメソッドが自動生成される：

| メソッド | HTTP | パス | 引数 |
|----------|------|------|------|
| `getAll()` | GET | `/api/users` | なし |
| `getById(id)` | GET | `/api/users/:id` | id |
| `create(data)` | POST | `/api/users` | data |
| `update(id, data)` | PUT | `/api/users/:id` | id, data |
| `delete(id)` | DELETE | `/api/users/:id` | id |

### 4.2 カスタムエンドポイント

```tp
User :: {
  rest: "/api/users"

  // カスタム追加
  search: get("/search")
  activate: post("/:id/activate")
  bulkDelete: delete("/bulk")
  upload: post("/:id/avatar", multipart: true)
}
```

### 4.3 HTTPメソッド関数

| 関数 | HTTP | 例 |
|------|------|-----|
| `get(path)` | GET | `get("/search")` |
| `post(path)` | POST | `post("/create")` |
| `put(path)` | PUT | `put("/:id")` |
| `patch(path)` | PATCH | `patch("/:id")` |
| `delete(path)` | DELETE | `delete("/:id")` |

### 4.4 リクエスト設定

```tp
User :: {
  rest: "/api/users"

  // 共通ヘッダー
  headers: {
    "Content-Type": "application/json"
  }

  // 認証トークン（自動付与）
  auth: $Auth.token

  // タイムアウト（ミリ秒）
  timeout: 5000
}
```

---

## 5. ストア（| 構文）

### 5.1 基本構造

```tp
Counter | {
  State {
    count: 0
    loading: false
    error: null
  }

  Actions {
    Increment
    Decrement
    Reset
    SetCount(value: number)
  }

  Reducers {
    on Increment { count: count + 1 }
    on Decrement { count: count - 1 }
    on Reset { count: 0 }
    on SetCount(value) { count: value }
  }
}
```

- `$Counter.count` で状態を参照
- `Counter.Increment` でアクションをディスパッチ

### 5.2 Actions（アクション定義）

```tp
Actions {
  // パラメータなし
  Increment
  Decrement

  // パラメータあり
  SetCount(value: number)
  LoadSuccess(items: array)
  LoadFailure(error: string)
}
```

### 5.3 Reducers（状態変更）

```tp
Reducers {
  on Increment { count: count + 1 }

  on SetCount(value) { count: value }

  on LoadSuccess(items) {
    items: items
    loading: false
  }

  on LoadFailure(error) {
    error: error
    loading: false
  }
}
```

- `on ActionName { }` で対応するアクションを処理
- イミュータブル更新（内部で自動処理）

### 5.4 Effects（副作用）

```tp
Effects {
  on Load {
    try {
      items: await getAll()      // 同名APIから自動参照
      dispatch: LoadSuccess(items)
    }
    catch(e) {
      dispatch: LoadFailure(e.message)
    }
  }

  on Create(data) {
    await create(data)
    dispatch: Load               // リロード
  }
}
```

- 非同期処理（API呼び出し等）
- `dispatch:` で別のアクションを発行

### 5.5 Selectors（派生状態）

```tp
Selectors {
  doubleCount { count * 2 }
  isPositive { count > 0 }
  isEmpty { items.length == 0 }
}
```

- 既存の状態から派生した値を計算
- `$Counter.doubleCount` で参照
- メモ化される

---

## 6. 同名連携（API + ストア）

### 6.1 基本パターン

```tp
// APIサービス
User :: {
  rest: "/api/users"
}

// ストア（同名で自動連携）
User | {
  State {
    items: []
    current: null
    loading: false
    error: null
  }

  Actions {
    Load
    LoadSuccess(items)
    LoadFailure(error)
    Get(id)
    GetSuccess(user)
    Create(data)
    Update(id, data)
    Delete(id)
  }

  Reducers {
    on Load { loading: true, error: null }
    on LoadSuccess(items) { items: items, loading: false }
    on LoadFailure(error) { error: error, loading: false }
    on GetSuccess(user) { current: user }
  }

  Effects {
    on Load {
      try {
        items: await getAll()        // User:: から自動参照
        dispatch: LoadSuccess(items)
      }
      catch(e) {
        dispatch: LoadFailure(e.message)
      }
    }

    on Get(id) {
      user: await getById(id)        // 自動参照
      dispatch: GetSuccess(user)
    }

    on Create(data) {
      await create(data)             // 自動参照
      dispatch: Load
    }

    on Update(id, data) {
      await update(id, data)         // 自動参照
      dispatch: Load
    }

    on Delete(id) {
      await delete(id)               // 自動参照
      dispatch: Load
    }
  }
}
```

### 6.2 同名連携のメリット

- `User::` で定義したAPIメソッドが `User|` の Effects 内で直接呼べる
- `User.getAll()` → `getAll()` と省略可能
- API + Store が1セットで管理される
- 命名の一貫性が保たれる

### 6.3 TypeScript連携

```tp
// user.tp
User :: {
  rest: "/api/users"
}

User | {
  // ...
}
```

```ts
// user.ts（同名ファイルで自動連携）
import type { User } from './user.tp'

// 型定義
export interface UserEntity {
  id: number
  name: string
  email: string
}

// カスタムロジック
export function validateUser(user: UserEntity): boolean {
  return user.name.length > 0 && user.email.includes('@')
}

// Effects内で使える関数
export async function onUserCreated(user: UserEntity) {
  console.log('User created:', user)
  // 追加のロジック
}
```

```tp
// user.tp 内で使用
User | {
  Effects {
    on Create(data) {
      user: await create(data)
      onUserCreated(user)           // TSファイルから自動インポート
      dispatch: Load
    }
  }
}
```

---

## 7. イベント / バインディング

### 7.1 イベントハンドラ（コロン記法）

```tp
Button -> {
  type: button
  click: Counter.Increment              // アクションをディスパッチ
  mouseenter: Counter.SetHover(true)    // パラメータ付き
  mouseleave: Counter.SetHover(false)
}
```

### 7.2 利用可能なイベント

| イベント | 説明 |
|----------|------|
| `click` | クリック |
| `dblclick` | ダブルクリック |
| `mouseenter` | マウスが入った |
| `mouseleave` | マウスが出た |
| `focus` | フォーカス |
| `blur` | フォーカス外れ |
| `change` | 値変更 |
| `input` | 入力中 |
| `submit` | フォーム送信 |
| `keydown` | キー押下 |
| `keyup` | キー離す |

### 7.3 値バインディング

```tp
Input -> {
  type: textbox
  value: $Form.username        // 単方向バインド（表示）
  change: Form.SetUsername     // 変更時にアクション
}

// 双方向バインド（シンタックスシュガー）
Input -> {
  type: textbox
  bind: $Form.username         // value + change を自動設定
}
```

### 7.4 条件付き表示

```tp
ErrorMessage -> {
  type: text
  content: $Form.error
  visible: $Form.error != null   // 条件式
}

// または if を使用
ErrorMessage -> {
  type: text
  content: $Form.error
  if: $Form.hasError
}
```

### 7.5 リスト表示

```tp
TodoList -> {
  type: list
  items: $Todos.items
  key: "id"
  render: TodoItem              // 各アイテムに適用するコンポーネント
}

TodoItem -> {
  type: container
  children: [TodoCheckbox, TodoText]
}

TodoCheckbox -> {
  type: checkbox
  checked: $item.completed      // $item で現在のアイテムを参照
  change: Todos.Toggle($item.id)
}

TodoText -> {
  type: text
  content: $item.text
}
```

---

## 8. ファイルベースルーティング

### 8.1 ディレクトリ構造

```
src/
├── pages/
│   ├── index.tp          → /
│   ├── about.tp          → /about
│   ├── users/
│   │   ├── index.tp      → /users
│   │   ├── [id].tp       → /users/:id (動的)
│   │   └── [id]/
│   │       └── posts.tp  → /users/:id/posts
│   └── [...slug].tp      → /* (キャッチオール)
├── components/
│   └── shared.tp
├── stores/
│   └── counter.tp
└── layouts/
    └── default.tp
```

### 8.2 動的ルート

```tp
// pages/users/[id].tp

// URLパラメータは $route.params で取得
UserDetail -> {
  type: container
  children: [UserName, UserPosts]
}

UserName -> {
  type: text
  content: $route.params.id    // /users/123 → "123"
}
```

### 8.3 レイアウト

```tp
// layouts/default.tp
DefaultLayout -> {
  children: [Header, PageContent, Footer]
}

Header -> {
  type: container
  style: "fixed top-0 w-full"
  children: [Logo, Nav]
}

PageContent -> {
  type: container
  children: [$page]            // $page = ルーティングされたページ
}

Footer -> {
  type: container
  children: [Copyright]
}
```

```tp
// pages/about.tp
layout: default              // 使用するレイアウトを指定

AboutPage -> {
  children: [Title, Description]
}
```

### 8.4 ルーティング用特殊変数

| 変数 | 説明 |
|------|------|
| `$route.path` | 現在のパス |
| `$route.params` | URLパラメータ |
| `$route.query` | クエリパラメータ |
| `$page` | 現在のページコンポーネント |

### 8.5 ナビゲーション

```tp
NavLink -> {
  type: link
  href: "/about"
  content: "About"
}

// プログラムによるナビゲーション
GoButton -> {
  type: button
  content: "Go to Users"
  click: Router.Navigate("/users")
}

// パラメータ付き
UserLink -> {
  type: link
  href: "/users/{$user.id}"    // テンプレート記法
  content: $user.name
}
```

---

## 9. AST構造

### 9.1 ノードタイプ

```rust
enum AstNode {
    // トップレベル
    Program { body: Vec<Declaration> },

    // 宣言（4種類の定義演算子に対応）
    Component { name: String, body: ComponentBody },        // Name -> { }
    Method { name: String, body: Expression },              // Name { }
    ApiService { name: String, body: ApiServiceBody },      // Name :: { }
    Store { name: String, body: StoreBody },                // Name | { }

    // APIサービス本体
    ApiServiceBody {
        rest: Option<String>,
        endpoints: Vec<Endpoint>,
        headers: Option<Object>,
        auth: Option<Expression>,
        timeout: Option<u32>,
    },

    // エンドポイント
    Endpoint {
        name: String,
        method: HttpMethod,  // GET, POST, PUT, PATCH, DELETE
        path: String,
        options: Option<Object>,
    },

    // ストア本体
    StoreBody {
        state: StateBlock,
        actions: ActionsBlock,
        reducers: ReducersBlock,
        effects: Option<EffectsBlock>,
        selectors: Option<SelectorsBlock>,
    },

    // コンポーネント本体
    ComponentBody { properties: Vec<Property>, children: Option<ChildrenRef> },

    // プロパティ
    Property { key: String, value: Expression },

    // 式
    Expression {
        Literal(Value),
        Reference { store: String, path: Vec<String> },    // $Store.path
        ActionRef { store: String, action: String, args: Vec<Expression> },
        ApiCall { service: String, method: String, args: Vec<Expression> },
        Array(Vec<Expression>),
        BinaryOp { left: Box<Expression>, op: Operator, right: Box<Expression> },
        Await(Box<Expression>),
    },
}

enum HttpMethod {
    GET, POST, PUT, PATCH, DELETE
}
```

### 9.2 パース例

入力:
```tp
Counter -> {
  type: text
  value: $Count.value
  style: "text-xl"
}
```

AST:
```json
{
  "type": "Component",
  "name": "Counter",
  "body": {
    "properties": [
      { "key": "type", "value": { "Literal": "text" } },
      { "key": "value", "value": { "Reference": { "store": "Count", "path": ["value"] } } },
      { "key": "style", "value": { "Literal": "text-xl" } }
    ],
    "children": null
  }
}
```

---

## 10. JS出力仕様

### 10.1 出力構造

```
dist/
├── index.html
├── app.js              // メインバンドル
├── stores/
│   └── counter.js      // Store定義
├── components/
│   └── *.js            // コンポーネント
└── assets/
    └── style.css       // Tailwind CSS
```

### 10.2 ランタイム

```js
// 生成されるコード例
import { createStore, dispatch, ref } from 'topo-runtime';

// Store
const Counter = createStore('Counter', {
  count: 0,
  loading: false
});

// Reducers
Counter.on('Increment', (state) => ({
  ...state,
  count: state.count + 1
}));

// Component
function CounterDisplay() {
  const count = ref(() => Counter.state.count);

  return {
    type: 'text',
    value: count,
    style: 'text-6xl font-mono'
  };
}

// Event binding
function IncrementBtn() {
  return {
    type: 'button',
    content: '+',
    onclick: () => dispatch('Counter', 'Increment'),
    style: 'px-6 py-2 bg-green-500'
  };
}
```

### 10.3 ランタイムAPI

```js
// topo-runtime
export function createStore(name, initialState);
export function dispatch(store, action, payload?);
export function ref(selector);  // リアクティブ参照
export function effect(fn);     // 副作用
export function computed(fn);   // 計算プロパティ
export function mount(component, element);  // DOMにマウント
```

### 10.4 SSG/SSR出力

```js
// SSG: 静的HTML生成
topo build --mode ssg

// SSR: サーバーサイド
topo build --mode ssr

// SPA: クライアントのみ
topo build --mode spa
```

---

## 11. プロジェクト構造

### 11.1 Rustプロジェクト

```
topo/
├── Cargo.toml
├── src/
│   ├── main.rs              // CLI エントリポイント
│   ├── lib.rs               // ライブラリエントリ
│   ├── lexer/
│   │   ├── mod.rs
│   │   └── token.rs         // トークン定義
│   ├── parser/
│   │   ├── mod.rs
│   │   ├── ast.rs           // AST定義
│   │   └── grammar.rs       // パース規則
│   ├── analyzer/
│   │   ├── mod.rs
│   │   ├── resolver.rs      // 名前解決
│   │   ├── validator.rs     // 検証
│   │   └── sorter.rs        // トポロジカルソート
│   ├── codegen/
│   │   ├── mod.rs
│   │   ├── js.rs            // JS生成
│   │   ├── html.rs          // HTML生成
│   │   └── css.rs           // CSS生成
│   └── cli/
│       ├── mod.rs
│       ├── build.rs         // ビルドコマンド
│       ├── dev.rs           // 開発サーバー
│       └── new.rs           // プロジェクト生成
├── runtime/
│   └── js/
│       ├── package.json
│       └── src/
│           ├── index.ts     // ランタイム本体
│           ├── store.ts
│           ├── reactive.ts
│           └── router.ts
└── tests/
    ├── lexer_test.rs
    ├── parser_test.rs
    └── fixtures/
        └── *.tp
```

### 11.2 CLIコマンド

```bash
# プロジェクト作成
topo new my-app

# 開発サーバー起動
topo dev

# ビルド
topo build
topo build --mode ssg
topo build --mode ssr

# 型チェック
topo check

# フォーマット
topo fmt
```

### 11.3 設定ファイル

```toml
# topo.toml
[project]
name = "my-app"
version = "0.1.0"

[build]
mode = "spa"          # spa | ssg | ssr
output = "dist"
minify = true

[dev]
port = 3000
open = true

[style]
framework = "tailwind"
config = "tailwind.config.js"
```

---

## 12. 実装フェーズ

### Phase 1: MVP（カウンターアプリが動く）
1. Lexer（字句解析）
2. Parser（構文解析）
3. 基本AST
4. JS Codegen（基本）
5. 最小ランタイム
6. CLI基本コマンド

### Phase 2: 実用レベル
1. 全UIプリミティブ
2. Effects
3. Selectors
4. ファイルベースルーティング
5. 開発サーバー（HMR）

### Phase 3: 本番対応
1. SSG/SSR
2. 最適化（Tree-shaking, Minify）
3. IDE拡張（LSP）
4. エラーメッセージ改善
5. ドキュメント

---

## 付録A: カウンターアプリ（シンプル）

```tp
// src/stores/counter.tp

Counter | {
  State {
    count: 0
  }

  Actions {
    Increment
    Decrement
    Reset
  }

  Reducers {
    on Increment { count: count + 1 }
    on Decrement { count: count - 1 }
    on Reset { count: 0 }
  }

  Selectors {
    isZero { count == 0 }
  }
}
```

```tp
// src/pages/index.tp

Header -> {
  type: text
  content: "Counter App"
  style: "text-2xl font-bold mb-4"
}

CounterDisplay -> {
  type: text
  value: $Counter.count
  style: "text-6xl font-mono"
}

DecrementBtn -> {
  type: button
  content: "-"
  click: Counter.Decrement
  disabled: $Counter.isZero
  style: "px-6 py-2 bg-red-500 text-white rounded disabled:opacity-50"
}

IncrementBtn -> {
  type: button
  content: "+"
  click: Counter.Increment
  style: "px-6 py-2 bg-green-500 text-white rounded"
}

ResetBtn -> {
  type: button
  content: "Reset"
  click: Counter.Reset
  style: "px-6 py-2 bg-gray-500 text-white rounded"
}

ButtonGroup -> {
  align: horizontal
  style: "gap-4 mt-4"
  children: [DecrementBtn, IncrementBtn, ResetBtn]
}

App -> {
  style: "p-8 flex flex-col items-center"
  align: vertical
  children: [Header, CounterDisplay, ButtonGroup]
}
```

---

## 付録B: ユーザー管理アプリ（API連携）

```tp
// src/services/user.tp

// APIサービス定義
User :: {
  rest: "/api/users"

  // カスタムエンドポイント
  search: get("/search")
  activate: post("/:id/activate")
}

// ストア定義（同名で自動連携）
User | {
  State {
    items: []
    current: null
    loading: false
    error: null
  }

  Actions {
    Load
    LoadSuccess(items)
    LoadFailure(error)
    Get(id)
    GetSuccess(user)
    Create(data)
    Update(id, data)
    Delete(id)
  }

  Reducers {
    on Load { loading: true, error: null }
    on LoadSuccess(items) { items: items, loading: false }
    on LoadFailure(error) { error: error, loading: false }
    on GetSuccess(user) { current: user }
  }

  Effects {
    on Load {
      try {
        items: await getAll()
        dispatch: LoadSuccess(items)
      }
      catch(e) {
        dispatch: LoadFailure(e.message)
      }
    }

    on Get(id) {
      user: await getById(id)
      dispatch: GetSuccess(user)
    }

    on Create(data) {
      await create(data)
      dispatch: Load
    }

    on Update(id, data) {
      await update(id, data)
      dispatch: Load
    }

    on Delete(id) {
      await delete(id)
      dispatch: Load
    }
  }

  Selectors {
    isEmpty { items.length == 0 }
    hasError { error != null }
  }
}
```

```tp
// src/pages/users/index.tp

PageTitle -> {
  type: text
  content: "User Management"
  style: "text-2xl font-bold mb-4"
}

LoadingSpinner -> {
  type: spinner
  if: $User.loading
  style: "mx-auto"
}

ErrorAlert -> {
  type: alert
  content: $User.error
  variant: "error"
  if: $User.hasError
}

UserTable -> {
  type: table
  columns: ["ID", "Name", "Email", "Actions"]
  data: $User.items
  if: !$User.loading
}

AddUserBtn -> {
  type: button
  content: "Add User"
  click: Router.Navigate("/users/new")
  style: "px-4 py-2 bg-blue-500 text-white rounded"
}

RefreshBtn -> {
  type: button
  content: "Refresh"
  click: User.Load
  style: "px-4 py-2 bg-gray-500 text-white rounded"
}

ActionBar -> {
  align: horizontal
  style: "gap-4 mb-4"
  children: [AddUserBtn, RefreshBtn]
}

UserListPage -> {
  style: "p-8"
  align: vertical
  children: [PageTitle, ActionBar, LoadingSpinner, ErrorAlert, UserTable]
}
```
