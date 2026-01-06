# 残タスク

## 優先度: 高

### Angularライクな機能実装

- [ ] Guard
- [ ] Pipe
- [ ] Service の axios インスタンスのプロバイダ化
- [ ] インターセプターの実装
- [ ] Resolver（ルート遷移前のデータプリフェッチ）
- [ ] CanDeactivate（ページ離脱時の確認）
- [ ] Directive（カスタムディレクティブ）
- [ ] Async Validator（非同期バリデーション）

### DX

- [ ] LSP（IDE 補完・エラー表示）
- [ ] Error Boundary（エラーハンドリングのコンポーネント化）

### 構文

- [ ] コンポーネントエイリアス構文（`Alias(args) -> Base(args, defaultValue)`）

### 設定

- [ ] 環境変数の実装

### 外部ライブラリ

- [ ] JS ライブラリの import サポート（CDN / esm.sh 経由）
- [ ] topo.config.json での依存関係定義
- [ ] バンドラー統合（esbuild / Vite）

## 優先度: 中

### DX

- [ ] DevTools（ストア状態のデバッグ用ブラウザ拡張）
- [ ] Hot Module Replacement（状態保持したライブリロード）

### ビルド

- [ ] ビルド時のミニファイ
- [ ] Lazy Loading（ルートごとのコード分割）

### UI 機能

- [ ] Portal / Overlay（モーダル・トースト用）
- [ ] Form Array（動的フォームフィールド）

### デバッグ

- [ ] デバッグ実行でのブレークポイント設定

## 優先度: 低

### UI 機能

- [ ] Animation（状態遷移アニメーション）

### プロダクション

- [ ] PWA / Service Worker（オフライン対応）
- [ ] SSR Hydration（SSR 後のクライアント側ハイドレーション）
