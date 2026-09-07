<div align="center">
  <img src="./assets/opentake-logo.png" alt="OpenTake" width="128" />

  <h1>OpenTake</h1>

  <p><strong>エージェントネイティブな動画制作エンジン</strong></p>

  <p>
    <a href="README.md">English</a> &nbsp;|&nbsp;
    <a href="README.zh-CN.md">中文</a> &nbsp;|&nbsp;
    <a href="README.ja.md">日本語</a>
  </p>
</div>

**ソースバージョン：`1.0.0-beta.6`。** 公開状況とダウンロードは [GitHub Releases](https://github.com/appergb/OpenTake/releases) を参照してください。[実行計画](docs/plans/active/2026-09-06-public-beta.md)、[バージョンノート](docs/releases/1.0.0-beta.6.md)、[文書とソースの確認記録](docs/documentation-sync-2026-09-06.md)に実装と検証を記録しています。

ソースには、複数のメディアプレビュータブ、フォルダー/フラット/グループ表示、temporal compositor、透明 Motion 出力、ProRes 4444 書き出しが含まれます。Text パネルはテキストクリップを追加し、Effect パネルは既存の効果を保持してプリセットを追加します。Sticker は画像/Lottie 素材の読み込み、選択、プレビュー、タイムライン配置に対応しています。単体テストと macOS パッケージの GUI 操作は[検証記録](docs/audit/2026-09-06/public-beta-validation.md)を参照してください。

意味検索は固定バージョンの約 1.5 GB のモデルを使用し、チェックサム検証とオフラインインストールに対応しています。macOS と Windows で実際の画像・テキスト推論とランキングを検証しました。Windows は固定入力を指定する Tract エンジンを使用します。macOS パッケージではモデルのダウンロード、索引作成、中国語検索も実行済みです。利用前にモデルをインストールしてください。[モデル監査記録](docs/audit/2026-09-06/semantic-search-model.md)に詳細があります。

- [プロジェクトについて](#-プロジェクトについて)
- [なぜOpenTakeか](#-なぜopentakeか)
- [競合との違い](#-競合との違い)
- [主な機能](#-主な機能)
- [対応プラットフォーム](#-対応プラットフォーム)
- [Rustワークスペース](#-rustワークスペース)
- [アーキテクチャ](#-アーキテクチャ)
- [ドキュメント](#-ドキュメント)
- [クイックスタート](#-クイックスタート)
- [バージョン履歴](#-バージョン履歴)
- [コミュニティ](#-コミュニティ)
- [ライセンス](#-ライセンス)

---

## 📖 プロジェクトについて

**OpenTake** は、**Rust + Tauri 2** で構築された**クロスプラットフォームの動画制作エンジン**です。macOS / Windows / Linux の 3 プラットフォームを対象とし、プロの映像編集ワークフローに AI Agent を深く統合することを目的としています。

> 🌟 **革新的な点**: Agent に長大なスキルドキュメントを読ませるのではなく、OpenTake は**編集ガイダンス（Context Signal）を Agent に能動的に送信**します——各トラックの役割、各クリップの適切な編集方法、各段階で適用すべきルールを、ソフトウェアが Agent に直接伝えます。

### ポジショニング

OpenTake は CapCut / DaVinci Resolve / Final Cut Pro の代替品ではありません。**AI Agent ワークフロー向けに設計された動画エンジン**です。従来の編集ソフトが「人間が使うため」に設計されているのに対し、OpenTake は「人間と Agent が共に使うため」に設計されています。タイムライン、プレビュー、キーフレームシステムはすべて MCP プロトコルでネイティブに操作可能です。

---

## 🎯 なぜOpenTakeか

| 課題 | 従来の手法 | OpenTakeの手法 |
|:--|:--|:--|
| Agentが素材の編集方法を知らない | Agentが自らスキルドキュメントを読む | ソフトウェアがContext Signalを発信 — 「このトラックはA-roll、トーキングヘッドのリズムでカット」 |
| クロスプラットフォームに3つのコードベースが必要 | macOS: Swift/AVFoundation、Windows: C++/DirectShow | Rustの単一コードベース、FFmpeg + wgpu、各プラットフォームを個別検証 |
| 自分のAIキーを使いたい | ベンダーのクラウドサービスにロックイン | BYOK — fal.ai / Replicate / OpenAI に直接接続、バックエンド不要、運用コストゼロ |
| Agentはチャットできるが操作できない | CLI Agentがテキスト出力を読むだけ | 能力に応じた MCP Server — Agentが直接 add_clips / split_clip / set_keyframes を実行 |
| 動画タイプごとに毎回プロンプトを書き直す | 「あなたは製品レビューを編集しています…」を毎回繰り返す | ワークフロープラグインシステム: レビュー/チュートリアル/ゲーム/ウェディング、各タイプの手法を事前パッケージ化 |
| 新しいツールの学習コストが高い | 複雑なUI、長いオンボーディング | Agentが代わりに操作 — 「このインタビューを3分のハイライトに編集して」と言うだけ |

---

## ⚡ 競合との違い

OpenTake は Rust が保持するタイムライン、コマンドによる undo、認証 MCP、Context Signal、ローカル/BYOK メディア処理を組み合わせます。[能力台帳](docs/capabilities/CAPABILITY-LEDGER.md)は実装と検証を区別し、他の編集ソフトとの全機能同等性を保証するものではありません。

---

## ✨ 主な機能

### 🧠 Agent Context Signal システム

> Agentにドキュメントを読ませるのではなく、ソフトウェアが編集ガイダンスをAgentにプッシュする。

すべてのMCPツールレスポンスに `context_signal` が付随：
- **自動ジャンル判定**: トーキングヘッド / Vlog / モンタージュ / インタビュー / ショートドラマ / 長編
- **トラック役割アノテーション**: A-roll / B-roll / ナレーション / BGM / SFX / テキスト
- **リアルタイムルールチェック**: ブレスポイントルール、B-roll 5つの注意、クロック理論、ピーク検出ペーシング

ナレッジソース: [ClipSkills](https://github.com/appergb/ClipSkills) — 12巻のプロ編集ナレッジベース（MITライセンス）。

📖 [Context Signal 設計](docs/modules/opentake-agent/AGENT-CONTEXT-SIGNAL.md)

### 🔌 Agent ツールサーフェス

OpenTake は互換 Agent ツールを提供し、メディア・生成・provider の実行可能性に
応じて動的に公開します。利用できない機能は fail closed になります。

| グループ | 主要ツール |
|:--|:--|
| 読取 / 内省 | `get_timeline`, `get_media`, `inspect_media`, `search_media` |
| タイムライン編集 | `add_clips`, `split_clip`, `set_clip_properties`, `set_keyframes`, `ripple_delete_ranges` |
| 生成 / インポート | `generate_video`, `generate_image`, `generate_audio`, `import_media` |
| ライブラリ | `create_folder`, `move_to_folder`, `rename_media` |
| リソース | `models/video`, `models/image` |

公式 Codex / ChatGPT は現在のプロジェクトに紐づく認証済み loopback エンドポイントをターンごとに使用します。Beta 5 では外部 MCP クライアントの明示的なペアリング、認証情報の失効、再起動後の設定保持も追加しました。旧未認証エンドポイントは無効のままです。[MCP 実装](docs/modules/opentake-agent/mcp-server.md)を参照してください。

### 🎬 クロスプラットフォームメディアエンジン

| 機能 | 技術 |
|:--|:--|
| コーデック | FFmpeg (`ffmpeg-next`) |
| コンポジター | wgpu カスタムコンポジター |
| 音声再生 | cpal |
| 文字起こし | whisper-rs |
| 意味検索 | SigLIP2（固定 revision のモデル導入と macOS の実 Rust 推論を検証済み） |

### 🌐 BYOK AI生成

**Bring Your Own Key**: fal.ai / Replicate / OpenAI に直接接続。バックエンド不要、運用コストゼロ。

### 📋 ワークフロープラグインシステム

レビュー / チュートリアル / ゲーム / ウェディング / トーキングヘッド — 各ジャンルのプロ編集手法を JSON + Markdown プラグインとしてパッケージ化。

📖 [ワークフロープラグイン設計](docs/modules/opentake-agent/WORKFLOW-PLUGIN-SYSTEM.md)

---

## 🖥️ 対応プラットフォーム

| プラットフォーム | 検証範囲 |
|:--|:--|
| macOS | 主な開発環境。日付付きのネイティブ/パッケージ検証はリリース記録を参照。Intel と Apple Silicon は個別に検証します。 |
| Windows | ビルド/インストーラーの対象。候補コミットの CI と実機インストール/UI 検証が必要です。 |
| Linux | ビルド対象。新しい配布物やネイティブ GUI の検証完了は宣言していません。 |
| Headless core | Rust ライブラリとテストは GUI なしで利用可能。メディア/GPU/ブラウザー機能には実行環境が必要です。 |

---

## 🦀 Rustワークスペース

```
crates/
├── opentake-process-tree # Cross-platform child-process lifecycle
├── opentake-domain     # Timeline / Track / Clip / Keyframe
├── opentake-ops        # OverwriteEngine / RippleEngine / SnapEngine
├── opentake-project    # プロジェクト永続化 / バンドル / エクスポート
├── opentake-media      # FFmpeg / サムネイル / 波形 / 文字起こし / 意味検索
├── opentake-render     # wgpuコンポジター + テキストラスタライザ
├── opentake-motion     # Lottie / Web モーショングラフィックス
├── opentake-agent      # MCP Server + Agent Chat + Context Signal
├── opentake-gen        # 生成AIクライアント (fal.ai / Replicate / OpenAI)
├── opentake-core       # セッション管理 / DI / イベントバス
└── src-tauri           # Tauri 2 デスクトップシェル
```

---

## 🏗️ アーキテクチャ

```
┌──────────────────────────────────────────────────────┐
│ React + TypeScript フロントエンド                       │
│ TimelineView · Preview · Inspector · MediaPanel       │
│ Zustand: 読取専用Timelineミラー + UI専用状態            │
└────────────────────┬─────────────────────────────────┘
                     │ Tauri invoke + event
┌────────────────────▼─────────────────────────────────┐
│ 🦀 Rust Core — 真実の源                               │
│  opentake-domain / ops / project / render / media    │
│  opentake-agent / gen / core                          │
│         ▲                          │                 │
│   ターン単位の認証 MCP     呼出    ▼                 │
│   In-app Agent Chat   FFmpeg + wgpu + cpal + whisper │
└──────────────────────────────────────────────────────┘
```

📖 [アーキテクチャ詳細](docs/architecture/ARCHITECTURE.md)

---

## 📚 ドキュメント

| ドキュメント | 内容 |
|:--|:--|
| [ARCHITECTURE.md](docs/architecture/ARCHITECTURE.md) | アーキテクチャ、レイヤリング、クレートレイアウト |
| [ROADMAP.md](docs/architecture/ROADMAP.md) | Phase 0–10 ロードマップ |
| [MODULE-PORT-MAP.md](docs/architecture/MODULE-PORT-MAP.md) | 20上流モジュール移植仕様 |
| [AGENT-CONTEXT-SIGNAL.md](docs/modules/opentake-agent/AGENT-CONTEXT-SIGNAL.md) | Agent Context Signal 設計 |
| [WORKFLOW-PLUGIN-SYSTEM.md](docs/modules/opentake-agent/WORKFLOW-PLUGIN-SYSTEM.md) | ワークフロープラグインシステム |
| [ADVANCED-FEATURES.md](docs/architecture/ADVANCED-FEATURES.md) | CapCut対比の高度機能 |
| [CAPCUT-GAP.md](docs/architecture/CAPCUT-GAP.md) | CapCutとの33項目ギャップ分析 |
| [DECISIONS.md](DECISIONS.md) | 技術選定 / ライセンス ADR |
| [PORT-1TO1-GAP.md](docs/architecture/PORT-1TO1-GAP.md) | 1:1移植ギャップ分析 |

---
---

## 🔗 上流リファレンス

編集ロジックの移植時は、オリジナルの Palmier Pro Swift ソースと比較してください：

```bash
# OpenTake の兄弟ディレクトリに上流を clone
cd ..  # from OpenTake-generation/
git clone https://github.com/palmier-io/palmier-pro.git palmier-pro-upstream
cd OpenTake-generation
```

ディレクトリ構成：

```
PRIMARY-CN/
├── OpenTake-generation/       # このリポジトリ
└── palmier-pro-upstream/      # 上流 Swift ソース (GPL-3.0)
```

比較用キーファイル：

| モジュール | 上流 (Swift) | OpenTake (Rust/TS) |
|:--|:--|:--|
| Timeline モデル | `Sources/PalmierPro/Models/Timeline.swift` | `crates/opentake-domain/src/timeline.rs` |
| Clip モデル | `Sources/PalmierPro/Models/Timeline.swift` (Clip struct) | `crates/opentake-domain/src/clip.rs` |
| Clip レンダラー | `Sources/PalmierPro/Timeline/ClipRenderer.swift` | `web/src/components/timeline/clipRenderer.ts` |
| Timeline ジオメトリ | `Sources/PalmierPro/Timeline/TimelineGeometry.swift` | `web/src/lib/geometry.ts` |
| Snap エンジン | `Sources/PalmierPro/Timeline/SnapEngine.swift` | `web/src/lib/snap.ts` |
| 編集操作 | `Sources/PalmierPro/Editor/ViewModel/EditorViewModel+ClipMutations.swift` | `crates/opentake-ops/src/ops/` |
| MCP ツール | `Sources/PalmierPro/Agent/Tools/ToolExecutor+Timeline.swift` | `crates/opentake-agent/src/tools/` |

> 上流ディレクトリは OpenTake の .gitignore で除外されています。各コラボレーターが個別に clone してください。
---

## 🚀 クイックスタート

```bash
git clone https://github.com/appergb/OpenTake.git
cd OpenTake

cargo build
cargo test
cargo clippy

cd web && pnpm install && pnpm build
cd .. && cargo tauri dev
```


---

## 📋 バージョン履歴

| バージョン | 日付 | マイルストーン |
|:--|:--|:--|
| `0.1.0-dev` | 2026-06 | Phase 0+1: Cargo workspace + Domain models + Edit ops |
| `1.0.0-beta.1` | 2026-08-01 | 初回インストール可能 Beta：ローカル編集、Agent、Motion、レビュー可能な AI ワークフロー |
| `1.0.0-beta.2` | 2026-08-03 | 公式 Codex ログイン、原子的タイムライン操作、認証 MCP、操作性の強化 |
| `1.0.0-beta.3` | 2026-08-09 | Space 再生、HEVC ネイティブプレビューと配布パイプライン |
| `1.0.0-beta.4` | 2026-08-10 | 時間/トランジションの保存、書き出し整合性とアップデーター |
| `1.0.0-beta.5` | 2026-08-14 | 外部 MCP ペアリング、Agent 会話順序と Motion Studio |
| `1.0.0-beta.6` | [バージョン記録](docs/releases/1.0.0-beta.6.md) | 透明 Motion、ProRes 4444、複数プレビュー、メディア表示、Text/Effect/Sticker、実モデル意味検索 |
| *(planned)* `1.0.0` | TBD | Phase 10: フルリリース |

📖 [完全なロードマップ](docs/architecture/ROADMAP.md)

---

## 🌍 コミュニティ

| Discord (English) | Discord (中文) | WeChat |
|:--:|:--:|:--:|
| [![Discord EN](https://img.shields.io/badge/Join-EN-5865F2?logo=discord&logoColor=white)](https://discord.gg/opentake) | [![Discord CN](https://img.shields.io/badge/加入-中文-5865F2?logo=discord&logoColor=white)](https://discord.gg/opentake-cn) | TBD |

---

## 謝辞

| プロジェクト | ライセンス | 用途 |
|:--|:--|:--|
| [Palmier Pro](https://github.com/palmier-io/palmier-pro) | GPL-3.0 | 編集ロジックとドメインモデル |
| [ClipSkills](https://github.com/appergb/ClipSkills) | MIT | 編集ナレッジベース |
| [FFmpeg](https://ffmpeg.org) | LGPL-2.1+ | メディアコーデック |
| [Tauri](https://tauri.app) | MIT / Apache 2.0 | デスクトップフレームワーク |
| [wgpu](https://wgpu.rs) | MIT / Apache 2.0 | GPUレンダリング |
| [whisper.cpp](https://github.com/ggerganov/whisper.cpp) | MIT | 文字起こし |
| [rmcp](https://github.com/nicholasxuu/rmcp) | MIT | MCP server SDK |

---

## 📜 ライセンス

Copyright (C) 2026 OpenTake contributors

OpenTakeはフリーソフトウェアです。**GNU General Public License version 3 (GPLv3)** の条件の下で再配布・改変できます。

本プログラムは [Palmier Pro](https://github.com/palmier-io/palmier-pro) (Copyright (C) 2026 Palmier, Inc.) に基づいており、同じくGPLv3で配布されています。[NOTICE](NOTICE) を参照してください。

---

<div align="center">
  <sub>Built with 🦀 Rust + 💙 Open Source</sub>
</div>
