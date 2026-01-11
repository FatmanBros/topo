
## ~~無名ストア（ファイル名からの自動命名）~~ [実装済み]

同一ファイル内で定義したストアのストア名省略機能:

```topo
// recent-activity.tp
| {  // ← ストア名省略。ファイル名から RecentActivity として扱う
    State {
        activities: []
        isLoading: true
    }
    Actions { Load, SetActivities(activities) }
    // ...
}

RecentActivityCard -> {
    init: Load  // ← 同一ファイル内なら RecentActivity.Load を省略可
    children: activities.for(...)  // ← RecentActivity.activities も省略可
}
```

実装済み:
- パーサー: `| { ... }` 形式（無名ストア）をサポート
- ファイル名 kebab-case/snake_case/camelCase/PascalCase → PascalCase 変換でストア名生成
- 同一ファイル内での参照時、ストア名プレフィックス省略可能

---

## Component 拡充計画

### 現在の構成

| レイヤー | 数 |
|---------|-----|
| atoms | 31 |
| molecules | 32 |
| organisms | 20 |

### Atoms（基本UI要素）

| コンポーネント | 説明 | 優先度 | 状態 |
|--------------|------|--------|------|
| avatar | ユーザーアイコン（画像/イニシャル） | 高 | [x] |
| spinner | ローディングインジケーター | 高 | [x] |
| checkbox | チェックボックス | 高 | [x] |
| radio | ラジオボタン | 高 | [x] |
| switch | トグルスイッチ | 高 | [x] |
| divider | 区切り線（水平/垂直） | 中 | [x] |
| tooltip | ツールチップ | 中 | [x] |
| link | スタイル付きリンク | 中 | [x] |
| image | レスポンシブ画像 | 中 | [x] |
| skeleton | スケルトンローダー | 中 | [x] |

### Molecules（機能単位）

| コンポーネント | 説明 | 優先度 | 状態 |
|--------------|------|--------|------|
| alert | 通知メッセージ（success/error/warning/info） | 高 | [x] |
| modal | モーダルダイアログ | 高 | [x] |
| dropdown | ドロップダウンメニュー | 高 | [x] |
| tabs | タブナビゲーション | 高 | [x] |
| breadcrumb | パンくずリスト | 高 | [x] |
| pagination | ページネーション | 高 | [x] |
| search-input | 検索入力（アイコン付き） | 中 | [x] |
| avatar-group | 複数アバター表示 | 中 | [x] |
| empty-state | データなし状態 | 中 | [x] |
| file-upload | ファイルアップロード | 中 | [x] |
| rating | 星評価 | 低 | [x] |
| stepper | ステップインジケーター | 低 | [x] |

### Organisms（独立セクション）

| コンポーネント | 説明 | 優先度 | 状態 |
|--------------|------|--------|------|
| data-table | データテーブル（ソート/フィルタ対応） | 高 | [x] |
| navbar | 汎用ナビゲーションバー | 高 | [x] |
| card-list | カードリスト（グリッド/リスト切替） | 中 | [x] |
| comment-section | コメント欄 | 中 | [x] |
| notification-center | 通知一覧 | 中 | [x] |
| user-menu | ユーザーメニュー（ドロップダウン） | 中 | [x] |
| filter-panel | フィルターパネル | 低 | [x] |
| timeline | タイムライン表示 | 低 | [x] |

---

## スタイルシステム設計（Design Token + Variant）

### ディレクトリ構成

```
demo/
├── theme/
│   ├── tokens.tp        # デザイントークン（色、サイズ、間隔）
│   ├── variants/
│   │   ├── button.tp    # Button用variant定義
│   │   ├── badge.tp     # Badge用variant定義
│   │   ├── input.tp     # Input用variant定義
│   │   └── ...
│   └── index.tp         # re-export
└── components/
    └── atoms/
        └── button.tp    # Token + Variant を使用
```

### 1. Design Tokens（tokens.tp）

```topo
// demo/theme/tokens.tp

// カラーパレット
Colors {
    // Primary
    primary: {
        50: "indigo-50"
        100: "indigo-100"
        500: "indigo-500"
        600: "indigo-600"
        700: "indigo-700"
    }
    // Secondary
    secondary: {
        50: "gray-50"
        100: "gray-100"
        500: "gray-500"
        600: "gray-600"
        700: "gray-700"
    }
    // Semantic
    danger: {
        50: "red-50"
        500: "red-500"
        600: "red-600"
        700: "red-700"
    }
    warning: {
        50: "amber-50"
        500: "amber-500"
        600: "amber-600"
    }
    success: {
        50: "green-50"
        500: "green-500"
        600: "green-600"
    }
    info: {
        50: "blue-50"
        500: "blue-500"
        600: "blue-600"
    }
    // Neutral
    neutral: {
        50: "gray-50"
        100: "gray-100"
        200: "gray-200"
        300: "gray-300"
        500: "gray-500"
        700: "gray-700"
        800: "gray-800"
        900: "gray-900"
    }
}

// サイズ
Sizes {
    xs: { py: "py-1", px: "px-2", text: "text-xs", h: "h-6" }
    sm: { py: "py-1.5", px: "px-3", text: "text-sm", h: "h-8" }
    md: { py: "py-2", px: "px-4", text: "text-base", h: "h-10" }
    lg: { py: "py-2.5", px: "px-5", text: "text-lg", h: "h-12" }
    xl: { py: "py-3", px: "px-6", text: "text-xl", h: "h-14" }
}

// 角丸
Radius {
    none: "rounded-none"
    sm: "rounded"
    md: "rounded-md"
    lg: "rounded-lg"
    xl: "rounded-xl"
    full: "rounded-full"
}

// シャドウ
Shadows {
    none: "shadow-none"
    sm: "shadow-sm"
    md: "shadow"
    lg: "shadow-lg"
}
```

### 2. Variant定義（variants/button.tp）

```topo
// demo/theme/variants/button.tp
import { Colors, Sizes, Radius } from "../tokens.tp"

ButtonVariants {
    // ベーススタイル（全variantに適用）
    base: "inline-flex items-center justify-center font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed"

    // Variant別スタイル
    variant: {
        primary: "bg-{Colors.primary.600} text-white hover:bg-{Colors.primary.700} focus:ring-{Colors.primary.500}"
        secondary: "bg-{Colors.secondary.100} text-{Colors.neutral.700} hover:bg-{Colors.secondary.200} focus:ring-{Colors.secondary.500}"
        outline: "border border-{Colors.neutral.300} bg-transparent text-{Colors.neutral.700} hover:bg-{Colors.neutral.50} focus:ring-{Colors.primary.500}"
        ghost: "bg-transparent text-{Colors.neutral.700} hover:bg-{Colors.neutral.100} focus:ring-{Colors.primary.500}"
        danger: "bg-{Colors.danger.600} text-white hover:bg-{Colors.danger.700} focus:ring-{Colors.danger.500}"
        link: "bg-transparent text-{Colors.primary.600} hover:underline p-0"
    }

    // サイズ別スタイル
    size: {
        xs: "{Sizes.xs.py} {Sizes.xs.px} {Sizes.xs.text} {Radius.md}"
        sm: "{Sizes.sm.py} {Sizes.sm.px} {Sizes.sm.text} {Radius.md}"
        md: "{Sizes.md.py} {Sizes.md.px} {Sizes.md.text} {Radius.lg}"
        lg: "{Sizes.lg.py} {Sizes.lg.px} {Sizes.lg.text} {Radius.lg}"
        xl: "{Sizes.xl.py} {Sizes.xl.px} {Sizes.xl.text} {Radius.xl}"
    }

    // 幅
    fullWidth: "w-full"
}
```

### 3. コンポーネント実装（atoms/button.tp）

```topo
// demo/components/atoms/button.tp
import { ButtonVariants } from "../../theme/variants/button.tp"

// ベースButton - 全てのpropsを受け取る
Button(props) -> {
    type: props.type || "button"
    content: props.children
    click: props.onClick
    disabled: props.disabled
    style: ButtonVariants.base
         + " " + ButtonVariants.variant[props.variant || "primary"]
         + " " + ButtonVariants.size[props.size || "md"]
         + (props.fullWidth ? " " + ButtonVariants.fullWidth : "")
         + (props.className ? " " + props.className : "")
}

// プリセットButton（よく使う組み合わせ）
PrimaryButton(props) -> Button({ ...props, variant: "primary" })
SecondaryButton(props) -> Button({ ...props, variant: "secondary" })
OutlineButton(props) -> Button({ ...props, variant: "outline" })
GhostButton(props) -> Button({ ...props, variant: "ghost" })
DangerButton(props) -> Button({ ...props, variant: "danger" })
LinkButton(props) -> Button({ ...props, variant: "link" })

// サイズプリセット
SmallButton(props) -> Button({ ...props, size: "sm" })
LargeButton(props) -> Button({ ...props, size: "lg" })
```

### 4. 使用例

```topo
// pages/example.tp
import { Button, PrimaryButton, DangerButton } from "../components/atoms/button.tp"

ExamplePage -> {
    children: [
        // 基本使用
        Button({ children: "デフォルト" }),

        // variant指定
        Button({ variant: "outline", children: "アウトライン" }),

        // size指定
        Button({ size: "lg", children: "大きいボタン" }),

        // 組み合わせ
        Button({
            variant: "danger",
            size: "sm",
            fullWidth: true,
            children: "削除"
        }),

        // プリセット使用
        PrimaryButton({ children: "送信" }),
        DangerButton({ size: "sm", children: "削除" }),

        // カスタムクラス追加
        Button({
            variant: "primary",
            className: "shadow-lg",
            children: "影付き"
        })
    ]
}
```

### 5. 各コンポーネントのVariant設計

| Component | Variants | Sizes |
|-----------|----------|-------|
| Button | primary, secondary, outline, ghost, danger, link | xs, sm, md, lg, xl |
| Badge | primary, secondary, success, warning, danger, info, outline | sm, md, lg |
| Input | default, error, success | sm, md, lg |
| Alert | success, warning, danger, info | - |
| Avatar | - | xs, sm, md, lg, xl |
| Spinner | primary, secondary, white | xs, sm, md, lg |
| Card | default, bordered, elevated | sm, md, lg |

### 6. 実装順序

1. [x] `theme/tokens.tp` - デザイントークン定義（Colors, ColorStyles, Sizes, Radius, Shadows, Transitions）
2. [x] `theme/variants/button.tp` - Button variant
3. [x] `atoms/button.tp` 改修 - 新システム適用
4. [x] 他のatomsへ展開（avatar, spinner, checkbox, radio, switch, divider, tooltip, link, image, skeleton）
5. [x] moleculesへ展開（alert, modal, dropdown, tabs, breadcrumb, pagination）

### 7. 構文サポート状況 [確認済み]

| 構文 | 状態 | 備考 |
|------|------|------|
| 文字列結合 `+` | ✅ サポート済み | BinaryOp Add |
| デフォルト値 `\|\|` | ✅ サポート済み | BinaryOp Or |
| 配列スプレッド `[...arr]` | ✅ サポート済み | Expression::Spread |
| ブラケットアクセス `obj[key]` | ✅ 新規実装 | Expression::IndexAccess |
| オブジェクトスプレッド `{ ...props }` | ✅ 新規実装 | ObjectMember::Spread |

### 8. 残検討事項

- **テンプレートリテラル**: `{Colors.primary.600}` の展開方法（未実装、文字列連結で代用可能）
