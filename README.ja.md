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

**OpenTake** は、**Rust + Tauri 2** で構築された**クロスプラットフォームの動画制作エンジン**です。macOS / Windows / Linux の 3 プラットフォームで動作し、プロの映像編集ワークフローに AI Agent を深く統合することを目的としています。

> 🌟 **革新的な点**: Agent に長大なスキルドキュメントを読ませるのではなく、OpenTake は**編集ガイダンス（Context Signal）を Agent に能動的に送信**します——各トラックの役割、各クリップの適切な編集方法、各段階で適用すべきルールを、ソフトウェアが Agent に直接伝えます。

### ポジショニング

OpenTake は CapCut / DaVinci Resolve / Final Cut Pro の代替品ではありません。**AI Agent ワークフロー向けに設計された動画エンジン**です。従来の編集ソフトが「人間が使うため」に設計されているのに対し、OpenTake は「人間と Agent が共に使うため」に設計されています。タイムライン、プレビュー、キーフレームシステムはすべて MCP プロトコルでネイティブに操作可能です。

---

## 🎯 なぜOpenTakeか

| 課題 | 従来の手法 | OpenTakeの手法 |
|:--|:--|:--|
| Agentが素材の編集方法を知らない | Agentが自らスキルドキュメントを読む | ソフトウェアがContext Signalを発信 — 「このトラックはA-roll、トーキングヘッドのリズムでカット」 |
| クロスプラットフォームに3つのコードベースが必要 | macOS: Swift/AVFoundation、Windows: C++/DirectShow | Rustの単一コードベース、FFmpeg + wgpu、全プラットフォームで同一体験 |
| 自分のAIキーを使いたい | ベンダーのクラウドサービスにロックイン | BYOK — fal.ai / Replicate / OpenAI に直接接続、バックエンド不要、運用コストゼロ |
| Agentはチャットできるが操作できない | CLI Agentがテキスト出力を読むだけ | MCP Server 31ツール — Agentが直接 add_clips / split_clip / set_keyframes を実行 |
| 動画タイプごとに毎回プロンプトを書き直す | 「あなたは製品レビューを編集しています…」を毎回繰り返す | ワークフロープラグインシステム: レビュー/チュートリアル/ゲーム/ウェディング、各タイプの手法を事前パッケージ化 |
| 新しいツールの学習コストが高い | 複雑なUI、長いオンボーディング | Agentが代わりに操作 — 「このインタビューを3分のハイライトに編集して」と言うだけ |

---

## ⚡ 競合との違い

| 項目 | CapCut | DaVinci Resolve | Final Cut Pro | **OpenTake** |
|:--|:--|:--|:--|:--|
| **Agentネイティブ統合** | ❌ | ❌ | ❌ | ✅ MCP 31ツール + Context Signal |
| **クロスプラットフォーム** | ✅ macOS / Win | ✅ macOS / Win / Linux | ❌ macOSのみ | ✅ macOS / Win / Linux |
| **BYOK AI生成** | 有料テンプレート | ❌ | ❌ | ✅ fal.ai / Replicate / OpenAI に直接接続 |
| **ローカル文字起こし** | ❌ クラウドのみ | ❌ プラグイン必要 | ❌ プラグイン必要 | ✅ whisper-rs デバイス上推論 |
| **ローカル意味検索** | ❌ | ❌ | ❌ | ✅ SigLIP2 + Ort デバイス上 |
| **ワークフロープラグイン** | ❌ 固定テンプレート | ❌ | ❌ | ✅ JSON+MD コミュニティプラグイン |
| **オープンソース** | ❌ | ❌ | ❌ | ✅ GPL-3.0 |
| **Agent操作可能なキーフレーム** | ❌ | ❌ | ❌ | ✅ 全6 kfトラックをMCP経由で制御 |

---

## ✨ 主な機能

### 🧠 Agent Context Signal システム

> Agentにドキュメントを読ませるのではなく、ソフトウェアが編集ガイダンスをAgentにプッシュする。

すべてのMCPツールレスポンスに `context_signal` が付随：
- **自動ジャンル判定**: トーキングヘッド / Vlog / モンタージュ / インタビュー / ショートドラマ / 長編
- **トラック役割アノテーション**: A-roll / B-roll / ナレーション / BGM / SFX / テキスト
- **リアルタイムルールチェック**: ブレスポイントルール、B-roll 5つの注意、クロック理論、ピーク検出ペーシング

ナレッジソース: [ClipSkills](https://github.com/appergb/ClipSkills) — 12巻のプロ編集ナレッジベース（MITライセンス）。

📖 [Context Signal 設計](docs/AGENT-CONTEXT-SIGNAL.md)

### 🔌 MCP Server — 31ツール

`127.0.0.1:19789` で動作する完全なMCPサーバー。Agentがタイムラインを直接制御：

| グループ | 数 | 主要ツール |
|:--|:--:|:--|
| 読取 / 内省 | 7 | `get_timeline`, `get_media`, `inspect_media`, `search_media` |
| タイムライン編集 | 11 | `add_clips`, `split_clip`, `set_clip_properties`, `set_keyframes`, `ripple_delete_ranges` |
| 生成 / インポート | 5 | `generate_video`, `generate_image`, `generate_audio`, `import_media` |
| ライブラリ | 7 | `create_folder`, `move_to_folder`, `rename_media` |
| リソース | 2 | `models/video`, `models/image` |

### 🎬 クロスプラットフォームメディアエンジン

| 機能 | 技術 |
|:--|:--|
| コーデック | FFmpeg (`ffmpeg-next`) |
| コンポジター | wgpu カスタムコンポジター |
| 音声再生 | cpal |
| 文字起こし | whisper-rs |
| 意味検索 | candle / ort + SigLIP2 |

### 🌐 BYOK AI生成

**Bring Your Own Key**: fal.ai / Replicate / OpenAI に直接接続。バックエンド不要、運用コストゼロ。

### 📋 ワークフロープラグインシステム

レビュー / チュートリアル / ゲーム / ウェディング / トーキングヘッド — 各ジャンルのプロ編集手法を JSON + Markdown プラグインとしてパッケージ化。

📖 [ワークフロープラグイン設計](docs/WORKFLOW-PLUGIN-SYSTEM.md)

---

## 🖥️ 対応プラットフォーム

| プラットフォーム | 状態 | 備考 |
|:--|:--|:--|
| **macOS** (Apple Silicon + Intel) | ✅ 主要開発環境 | ネイティブARM64 + x86_64; Metal経由GPU (wgpu) |
| **Windows** (10/11 x86_64) | ✅ サポート | Vulkan / DX12 (wgpu); Tauri 2完全サポート |
| **Linux** (x86_64) | ✅ サポート | Vulkan; AppImage / deb |
| **バックエンド / ヘッドレス** | ✅ サポート | GUIなしで純Rustコア実行可能 |

---

## 🦀 Rustワークスペース

```
crates/
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
│   MCP Server (:19789)      呼出    ▼                 │
│   In-app Agent Chat   FFmpeg + wgpu + cpal + whisper │
└──────────────────────────────────────────────────────┘
```

📖 [アーキテクチャ詳細](docs/ARCHITECTURE.md)

---

## 📚 ドキュメント

| ドキュメント | 内容 |
|:--|:--|
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | アーキテクチャ、レイヤリング、クレートレイアウト |
| [ROADMAP.md](docs/ROADMAP.md) | Phase 0–10 ロードマップ |
| [MODULE-PORT-MAP.md](docs/MODULE-PORT-MAP.md) | 20上流モジュール移植仕様 |
| [AGENT-CONTEXT-SIGNAL.md](docs/AGENT-CONTEXT-SIGNAL.md) | Agent Context Signal 設計 |
| [WORKFLOW-PLUGIN-SYSTEM.md](docs/WORKFLOW-PLUGIN-SYSTEM.md) | ワークフロープラグインシステム |
| [ADVANCED-FEATURES.md](docs/ADVANCED-FEATURES.md) | CapCut対比の高度機能 |
| [CAPCUT-GAP.md](docs/CAPCUT-GAP.md) | CapCutとの33項目ギャップ分析 |
| [DECISIONS.md](DECISIONS.md) | 技術選定 / ライセンス ADR |
| [PORT-1TO1-GAP.md](docs/PORT-1TO1-GAP.md) | 1:1移植ギャップ分析 |

---
---

## 🔗 上流リファレンス

編集ロジックの移植時は、オリジナルの Palmier Pro Swift ソースと比較してください：

```bash
# OpenTake の兄弟ディレクトリに上流を clone
cd ..  # from OpenTake/
git clone https://github.com/palmier-io/palmier-pro.git palmier-pro-upstream
cd OpenTake
```

ディレクトリ構成：

```
PRIMARY-CN/
├── OpenTake/                  # このリポジトリ
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

> ⚠️ **現在の状態**: 初期設計段階。アーキテクチャ、ロードマップ、モジュール移植マップは完了。コード実装中。

---

## 📋 バージョン履歴

| バージョン | 日付 | マイルストーン |
|:--|:--|:--|
| `0.1.0-dev` | 2026-06 | Phase 0+1: Cargo workspace + Domain models + Edit ops |
| *(planned)* `1.0.0` | TBD | Phase 10: フルリリース |

📖 [完全なロードマップ](docs/ROADMAP.md)

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
