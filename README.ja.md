# Topo

[English](./README.md)

ネストヘルを解消する UI フレームワーク

## 概要

Topo は Rust 製の UI フレームワークです。宣言的な DSL を JavaScript にコンパイルし、React/Vue のようなネスト構造なしで Angular ライクなリアクティブ開発を実現します。

```tp
// ネストされた JSX ではなく、フラットな宣言
Header -> { type: text, content: "Hello" }
Button(label) -> { type: button, content: label, style: "px-4 py-2 bg-blue-500" }

App -> {
  children: [Header, Button("Click me")]
}
```

## 特徴

- **フラット DSL** - ネストヘルなし。モジュールレベルでコンポーネントを宣言し、参照で合成
- **4つの定義演算子**
  - `->` コンポーネント（UI 定義）
  - `|` ストア（NgRx スタイルの状態管理）
  - `::` API サービス（REST クライアント自動生成）
  - `{}` メソッド（ロジック/関数）
- **ファイルベースルーティング** - `pages/users/[id].tp` → `/users/:id`
- **バリデーションアノテーション** - `@required`, `@email`, `@minLength(8)`
- **Tailwind CSS** - ビルトインサポート
- **i18n** - 国際化対応

## インストール

```bash
# クローンしてビルド
git clone https://github.com/yourname/topo.git
cd topo
cargo build --release

# PATH に追加
export PATH="$PATH:$(pwd)/target/release"
```

## クイックスタート

```bash
# 新規プロジェクト作成
topo new my-app
cd my-app

# 開発サーバー起動
topo dev
```

## CLI コマンド

| コマンド | 説明 |
|---------|------|
| `topo new <name>` | 新規プロジェクト作成 |
| `topo init` | 現在のディレクトリで初期化 |
| `topo build` | JavaScript にコンパイル |
| `topo dev` | ファイル監視付き開発サーバー |
| `topo start` | ビルドして配信 |
| `topo test` | E2E テスト実行（Playwright） |

## 使用例

### コンポーネント

```tp
LoginButton -> {
  type: button
  content: t("sign_in")
  click: Auth.Login
  style: "px-4 py-2 bg-blue-600 text-white rounded"
}
```

### ストア

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

### API サービス

```tp
AuthApi :: {
  rest: "/api/auth"
  login: post("/login")
}
```

## プロジェクト構成

```
my-app/
├── topo.config.json    # 設定ファイル
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

## ライセンス

MIT
