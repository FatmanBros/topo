# 会話に付いて
返答は全て**日本語**ですること

# Development Guidelines

## Git Workflow

修正作業は git worktree を使用して実施すること。

```bash
# 新しいブランチで作業する場合
git worktree add ../topo-feature feature-branch

# 作業完了後
git worktree remove ../topo-feature
```

## ページ設計: Feature-based Structure

ページは機能単位でディレクトリにまとめる。各コンポーネントは自身の Store を持ち、init で独立してデータ取得する。

```
demo/pages/
└── dashboard/
    ├── index.tp          # Page定義（エントリポイント）
    ├── template.tp       # DashboardTemplate（レイアウトのみ）
    ├── components/       # ページ固有コンポーネント（ルートにならない）
    │   ├── stat-cards.tp
    │   ├── recent-activity.tp
    │   └── quick-actions.tp
    ├── projects/         # サブルート: /dashboard/projects
    │   └── index.tp
    ├── team/             # サブルート: /dashboard/team
    │   └── index.tp
    └── mocks/
        ├── stats.json
        └── activities.json
```

### ルーティング規則

- `index.tp` → ルートエントリポイント
- `template.tp`, `store.tp`, `layout.tp` → ルートにならない
- `components/` 配下 → ルートにならない（共有コンポーネント用）
- その他のディレクトリの `index.tp` → サブルート

### コンポーネント + Store パターン

各コンポーネントは自身の Store と init を持ち、独立してデータ取得する：

```topo
// components/stat-cards.tp
import { StatCard } from "../../../components/atoms/stat-card.tp"

StatCards | {
    State {
        stats: []
        isLoading: true
    }

    Actions {
        Load
        SetStats(stats)
    }

    Reducers {
        on Load { isLoading: true }
        on SetStats(stats) { stats: stats }
    }

    Effects {
        on Load {
            try {
                stats: await http.get("/api/stats")
                dispatch: SetStats(stats)
            } catch(e) {
                dispatch: SetStats([])
            }
        }
    }
}

StatCardsGrid -> {
    init: StatCards.Load
    style: "grid grid-cols-1 md:grid-cols-4 gap-6"
    children: StatCards.stats.for(stat => {
        StatCard({ label: stat.label, value: stat.value })
    })
}
```

### template.tp（レイアウトのみ）

```topo
import { DashboardLayout } from "../../components/templates/dashboard-layout.tp"
import { StatCardsGrid } from "./components/stat-cards.tp"
import { RecentActivityCard } from "./components/recent-activity.tp"

DashboardTemplate -> DashboardLayout([Header, StatCardsGrid, Content])
```

### index.tp（シンプルなPage定義）

```topo
import { DashboardTemplate } from "./template.tp"

Page -> {
    children: DashboardTemplate
}
```

## コンポーネント設計: Atomic Design

コンポーネントは Atomic Design パターンに従って構成する。
再利用性を考慮し、すでにあるものは極力再利用すること。
特定用途にとらわれず、使い勝手のよい構成・命名を考慮すること。

```
demo/components/
├── atoms/           # 最小単位のUI要素（type: text, button等はここだけ）
│   ├── button.tp
│   ├── heading.tp
│   ├── text.tp
│   └── card.tp
├── molecules/       # atomsを組み合わせた機能単位
│   ├── project-card.tp
│   ├── member-card.tp
│   └── progress-bar.tp
├── organisms/       # molecules/atomsを組み合わせた独立セクション
│   ├── login-form.tp
│   └── dashboard-sidebar.tp
└── templates/       # ページレイアウト
    └── dashboard-layout.tp
```

### Atomic Design 原則（厳守）

1. **`type: text`, `type: button` 等のプリミティブは atoms のみ**
   - pages/components で直接 `type: text` を書くのは禁止
   - 必ず `Text`, `Heading`, `Button` 等の atoms を import して使う

2. **pages は molecules/organisms のみ使用**
   - atoms を直接使うのは molecules/organisms の責務
   - pages/components は molecules/organisms を組み合わせる

3. **共有コンポーネントは demo/components/ に配置**
   - 複数ページで使うものは共有化
   - ページ固有のものは pages/{page}/components/ に配置（ただし atoms は使わない）

### 命名規則

- **atoms**: 単一機能を表す名前 (`Button`, `Text`, `Card`)
- **molecules**: 機能グループを表す名前 (`ProjectCard`, `MemberCard`, `ProgressBar`)
- **organisms**: セクション名 (`LoginForm`, `DashboardSidebar`)
- **templates**: `{機能}Layout` (`DashboardLayout`)

## フォーム実装パターン

フォームは Store と Component を組み合わせて実装する。

### 基本構造

```topo
// Store定義（バリデーション付き）
LoginFormCard | {
    State {
        @key("email")      // フィールドキー
        @required          // 必須バリデーション
        @email             // メール形式バリデーション
        email: ""

        @key("password")
        @required
        @minLength(8)      // 最小文字数
        password: ""

        @hidden            // UIに表示しないフィールド
        isSubmitting: false
    }

    Commands {
        Submit             // フォーム送信コマンド
        Reset              // リセットコマンド
    }

    Actions {
        SetEmail(value)
        SetPassword(value)
    }

    Reducers {
        on SetEmail(value) { email: value }
        on SetPassword(value) { password: value }
    }
}

// フォームコンポーネント
LoginForm(props) -> {
    type: "form"
    onSubmit: LoginFormCard.Submit
    children: [...LoginFormCard.Fields, props.trigger]  // Fieldsは自動生成
}
```

### バリデーションアノテーション

| アノテーション | 説明 |
|--------------|------|
| `@key("name")` | フィールドのname属性 |
| `@required` | 必須フィールド |
| `@email` | メール形式 |
| `@minLength(n)` | 最小文字数 |
| `@maxLength(n)` | 最大文字数 |
| `@pattern("regex")` | 正規表現パターン |
| `@hidden` | UI非表示（内部状態用） |

### 自動生成される要素

- `Store.Fields`: State の `@key` 付きフィールドから入力要素を自動生成
- `$Store.fieldName`: フィールド値の参照
- `Store.Action`: アクションディスパッチ

### Store からのフォーム自動生成

Store の State 定義から、対応する入力フィールドが自動生成される:

```topo
// Store定義
ContactForm | {
    State {
        @key("name")
        @required
        name: ""

        @key("email")
        @required
        @email
        email: ""

        @key("message")
        @required
        @minLength(10)
        message: ""
    }
    // ... Actions, Reducers
}

// フォームで使用
Form -> {
    type: "form"
    onSubmit: ContactForm.Submit
    children: [
        ...ContactForm.Fields,   // ← State から自動生成された入力フィールド
        SubmitButton({ text: "送信" })
    ]
}
```

自動生成では以下が行われる:

1. `@key` アノテーション付きの State フィールドごとに `<input>` 要素を生成
2. `@email` → `type="email"`、`@hidden` → `type="hidden"` など型を自動設定
3. `@required`, `@minLength` などのバリデーション属性を付与
4. `data-field="StoreName.fieldName"` 属性でフィールドを識別
5. 入力時に対応する `Set{Field}` アクションを自動ディスパッチ
