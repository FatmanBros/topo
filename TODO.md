
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
