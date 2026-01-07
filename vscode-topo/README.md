# Topo Language Support for VS Code

VS Code拡張機能 - Topo言語のフルサポート

## Features

- **シンタックスハイライト** - コンポーネント、ストア、API定義を色分け表示
- **コード補完** - コンポーネント、props、Tailwindクラスの自動補完
- **自動インポート** - 未インポートのコンポーネントを自動追加
- **フォーマッター** - propsの適切な改行、インデント調整
- **リンター** - 構文エラー、未使用インポートの検出
- **ホバー情報** - コンポーネントのシグネチャ、パラメータ表示
- **定義へジャンプ** - コンポーネント定義への移動
- **スニペット** - よく使うパターンのテンプレート

## Requirements

`topo-lsp` バイナリが必要です:

```bash
# Cargoでインストール
cargo install --path . --bin topo-lsp

# または、プロジェクトでビルド
cargo build --release --bin topo-lsp
```

## Installation

### 開発モード

```bash
cd vscode-topo
npm install
npm run compile
```

VS Codeで `F5` を押して拡張機能をデバッグ実行

### パッケージング

```bash
npm run package
```

生成された `.vsix` ファイルを VS Code でインストール

## Configuration

```json
{
  "topo.lsp.path": "/path/to/topo-lsp",
  "topo.format.maxLineLength": 100,
  "topo.tailwind.enabled": true
}
```

## Snippets

| Prefix | Description |
|--------|-------------|
| `comp` | コンポーネント定義 |
| `compc` | 子要素付きコンポーネント |
| `alias` | コンポーネントエイリアス |
| `store` | ストア定義 |
| `api` | APIサービス定義 |
| `imp` | インポート文 |
| `text` | テキストコンポーネント |
| `btn` | ボタンコンポーネント |
| `input` | 入力コンポーネント |
| `field` | フォームフィールド |
| `flex` | Flexコンテナ |
| `grid` | Gridコンテナ |

## License

MIT
