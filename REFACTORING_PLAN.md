# src/ リファクタリング計画

## 現状分析

### ファイルサイズ

| ファイル | 行数 | 状態 |
|---------|------|------|
| `main.rs` | 3,603 | **要分割** - 50以上の関数が混在 |
| `codegen/mod.rs` | 3,643 | **要分割** - JsCodegen/TsCodegen が同一ファイル |
| `parser/mod.rs` | 2,734 | 大きい - 将来的に分割検討 |
| `info_server.rs` | 1,566 | 中程度 |
| `link_analyzer.rs` | 1,139 | 中程度 |
| `ast/mod.rs` | 886 | 許容範囲 |
| `typecheck/mod.rs` | 637 | 許容範囲 |
| `lsp/completion.rs` | 624 | 許容範囲 |

### コード品質指標

- **Clippy 警告**: 19件
- **unwrap() 使用**: 57箇所
- **clone() 使用**: 189箇所

---

## Phase 1: Clippy 警告の修正

### 自動修正可能（11件）

```bash
cargo clippy --fix --bin "topo"
```

### 手動対応が必要（8件）

| 警告 | ファイル:行 | 修正内容 |
|------|------------|---------|
| `collapsible_if` | link_analyzer.rs:409 | ネストした if 文をマージ |
| `unnecessary_map_or` | parser/mod.rs:1390 | `map_or(false, ...)` → `is_some_and(...)` |
| `unnecessary_map_or` | codegen/mod.rs:329 | 同上 |
| `unnecessary_map_or` | main.rs:3194,3210,3212 | 同上 |
| `redundant_closure` | codegen/mod.rs:269 | `.map(\|p\| func(p))` → `.map(func)` |
| `redundant_closure` | main.rs:3339 | 同上 |
| `only_used_in_recursion` | codegen/mod.rs:2274,2674,3423,3459 | `&self` を削除してスタティック関数に |
| `only_used_in_recursion` | typecheck/mod.rs:466,519 | 同上 |
| `derivable_impls` | codegen/mod.rs:3195 | `#[derive(Default)]` に置換 |
| `ptr_arg` | main.rs:3284,3433 | `&PathBuf` → `&Path` |
| `manual_strip` | main.rs:3388 | `strip_suffix()` を使用 |
| `manual_pattern_char_comparison` | main.rs:3406, codegen/mod.rs:304 | `\|c\| c == '-' \|\| c == '_'` → `['-', '_']` |

---

## Phase 2: main.rs の分割

### 現在の構造

`main.rs` に以下の機能が混在:

- CLI 定義 (Cli, Commands)
- プロジェクト作成 (create_project, init_project, create_*_app)
- ビルド処理 (build_project, build_project_dev)
- インポート解決 (resolve_imports, resolve_import_path)
- HTML 生成 (generate_html, generate_html_ssg, generate_html_dev)
- サーバー (start_server, start_dev_server, serve_mock_api)
- テスト (run_tests, generate_playwright_test)
- デプロイ (generate_cloudflare_worker, generate_worker_js)
- ユーティリティ (capitalize, file_path_to_route, etc.)

### 提案する分割構造

```
src/
├── main.rs                  # CLI エントリポイントのみ（~100行）
├── cli/
│   ├── mod.rs               # pub mod 定義
│   ├── commands.rs          # Cli struct, Commands enum, InfoCommands
│   └── project.rs           # create_project, init_project, create_*_app
├── build/
│   ├── mod.rs
│   ├── builder.rs           # build_project, build_project_dev
│   ├── resolver.rs          # resolve_imports, resolve_import_path
│   ├── html.rs              # generate_html, generate_html_ssg, generate_html_dev
│   ├── tailwind.rs          # extract_tailwind_css, generate_tailwind_css_for_classes
│   └── minifier.rs          # minify_js, deduplicate_functions
├── server/
│   ├── mod.rs
│   ├── dev.rs               # start_dev_server
│   ├── static_server.rs     # start_server, safe_resolve_path, get_content_type
│   └── mock.rs              # serve_mock_api
├── test/
│   ├── mod.rs
│   ├── runner.rs            # run_tests, compile_test_files, create_test_setup
│   └── playwright.rs        # generate_playwright_test, target_to_selector, etc.
├── deploy/
│   ├── mod.rs
│   ├── routes.rs            # generate_routes, file_path_to_route
│   └── cloudflare.rs        # generate_cloudflare_worker, generate_worker_js, generate_wrangler_toml
└── utils/
    ├── mod.rs
    ├── path.rs              # find_project_root, find_tp_files, copy_dir_contents
    └── string.rs            # capitalize
```

### 移行手順

1. 新しいモジュールディレクトリを作成
2. 関数を機能単位で移動
3. `pub use` で既存の API を維持
4. `main.rs` を CLI エントリポイントのみに簡素化
5. テストが通ることを確認

---

## Phase 3: codegen/mod.rs の分割

### 現在の構造

- `JsCodegen` struct と実装 (~3,000行)
- `TsCodegen` struct と実装 (~500行)
- 共通ユーティリティ関数

### 提案する分割構造

```
src/codegen/
├── mod.rs               # pub mod 定義、共通トレイト/型（~100行）
├── js/
│   ├── mod.rs           # JsCodegen struct 定義
│   ├── component.rs     # コンポーネント生成
│   ├── store.rs         # Store 生成
│   ├── expression.rs    # 式の生成
│   └── statement.rs     # 文の生成
├── ts.rs                # TsCodegen（~500行、分割不要）
└── utils.rs             # capitalize_first 等の共通関数
```

### JsCodegen の責務分離

| ファイル | 責務 | 主要メソッド |
|---------|------|-------------|
| `component.rs` | コンポーネント生成 | `generate_component`, `generate_component_body` |
| `store.rs` | Store 生成 | `generate_store`, `generate_reducers`, `generate_effects` |
| `expression.rs` | 式の生成 | `generate_expression`, `generate_binary_expr` |
| `statement.rs` | 文の生成 | `generate_statement`, `generate_if_statement` |

---

## Phase 4: parser/mod.rs の分割（任意）

2,734行は許容範囲だが、将来的に分割する場合:

```
src/parser/
├── mod.rs               # Parser struct, parse() エントリポイント
├── expression.rs        # parse_expression, parse_binary_expression
├── statement.rs         # parse_statement, parse_if_statement
├── component.rs         # parse_component, parse_component_body
├── store.rs             # parse_store, parse_state, parse_actions
└── type_annotation.rs   # parse_type_annotation
```

---

## Phase 5: エラーハンドリング改善

### unwrap() の削減方針

#### 優先度 High: パブリック API

```rust
// ❌ 現在
pub fn parse(source: &str) -> Program {
    let result = parser.parse().unwrap();
    result
}

// ✅ 改善後
pub fn parse(source: &str) -> Result<Program, ParseError> {
    parser.parse()
}
```

#### 優先度 Medium: ファイル I/O

```rust
// ❌ 現在
let content = fs::read_to_string(path).unwrap();

// ✅ 改善後
let content = fs::read_to_string(path)
    .with_context(|| format!("Failed to read file: {}", path.display()))?;
```

#### 優先度 Low: 内部ロジック

```rust
// ❌ 現在
let first = chars.next().unwrap();

// ✅ 改善後（状況に応じて）
let first = chars.next().ok_or(Error::EmptyInput)?;
// または
let Some(first) = chars.next() else { return default; };
```

### clone() の削減方針

1. **借用で十分な場合**: `clone()` を削除して参照に
2. **Cow の活用**: 条件付きで所有権が必要な場合
3. **Arc/Rc の検討**: 複数箇所で共有が必要な場合

---

## 実行順序

```
Phase 1 (Clippy)
    ↓
Phase 2 (main.rs 分割)
    ↓
Phase 3 (codegen 分割)
    ↓
Phase 5 (エラーハンドリング) ← 段階的に実施
    ↓
Phase 4 (parser 分割) ← 任意
```

### 各フェーズの見積もり

| Phase | 作業量 | リスク | 効果 |
|-------|-------|-------|------|
| 1 | 小 | 低 | 中 |
| 2 | 大 | 中 | 高 |
| 3 | 中 | 中 | 高 |
| 4 | 中 | 中 | 中 |
| 5 | 大 | 低 | 中 |

---

## チェックリスト

### 各フェーズ完了時の確認

- [ ] `cargo build` が成功する
- [ ] `cargo test` が全て通る
- [ ] `cargo clippy` の警告がない（または減少している）
- [ ] 既存の API が壊れていない

### リファクタリング完了時の確認

- [ ] 500行を超えるファイルがない（または妥当な理由がある）
- [ ] 各モジュールが単一責任を持っている
- [ ] パブリック API が適切にエラーを返している
- [ ] 不要な `unwrap()`, `clone()` が削減されている
