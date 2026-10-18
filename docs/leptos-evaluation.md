# Leptos (+ trunk) と RPoem 独自の UI 層 — 取り込みの評価 / Evaluation: adopting Leptos in RPoem

2026-10-19。**日本語 → English**

## 日本語

### 確認できた事実
- `open-vsc-claude` の生成テンプレート「Rust + Leptos(trunk)」は、**自動テストで通る**: `npm run test:leptos`(`scripts/e2e-leptos.js`)が、生成物の `trunk build` → 実 Chromium で項目を追加 → 一覧に出る、までを確認する。
  経路は 2 通りとも成功: ブラウザ → FastAPI サブグラフ(直接)、ブラウザ → **RPoem ゲートウェイ(`babc658` の中継)** → FastAPI サブグラフ。
- RPoem には**独自の UI 層が既にある**: `open-runo-view`(仮想 DOM・keyed 差分・hooks・SSR・DOM アプライヤ、約 1,300 行)、`open-runo-crossui`(1 回書いた画面から Windows/macOS/Linux/Android/Web 用を生成)、`apps/desktop-wasm`(`wasm-bindgen` + `web-sys` の直接実装)。
- エコシステムの方針(`open-easy-web/CLAUDE.md`): TypeScript/Node のチェーンは廃止、フロントは Rust → WASM(`wasm-bindgen` + `web-sys`)。**Yew / Leptos / Dioxus 等の重量級フレームワークは「強い理由がない限り採用しない」**。

### Leptos の利点(正直に)
- 成熟したリアクティビティ(シグナル)、SSR/ハイドレーション、型付きのコンポーネント、大きなエコシステム。アプリ側に JS/TS を一行も書かずに済む。
- 実際に動く(上のテスト)。

### 費用・懸念
- 依存が重い(初回ビルドは数分、Leptos 本体+`trunk` という追加ツール)。**実測(2026-10-19、open-upstream-watch のダッシュボード)**: release の WASM は約 0.79 MB(`opt-level = "s"` + LTO、`wasm-opt` 無し)、初回の trunk ビルドは約 3 分。
- 版ごとの API 変化(0.7 系で作成)。
- **`open-runo-view` と機能が重複**し、方針(重量級フレームワークを採らない)とぶつかる。

### 結論と推奨
1. **RPoem の本体(コア・画面)に Leptos を依存として入れない。** 方針と既存の `open-runo-view` に反するうえ、取り込む強い理由(`open-runo-view` では実現できないこと)は、まだ示されていない。
2. Leptos は **`open-vsc-claude` の生成テンプレートの「実験的」な選択肢**として残す(生成される利用者のプロジェクトの話で、RPoem の依存ではない)。自動テストで守る。
3. 「JS/TS を書かずに型付きで書ける」という利点は、**RPoem 自身の `open-runo-view` + `crossui` で得る**のが方針に沿う。次の一手の提案(Leptos 側の実測値は上のとおり得た): 同じ「項目の一覧と追加」アプリを `open-runo-view` で書いたテンプレート(`rust-runo-view`)を拡張に足し、Leptos 版と **WASM サイズ・ビルド時間・コード量**を並べて測る。差が大きい場合だけ、方針の見直しを所有者に諮る。
4. **所有者への確認事項**: 「重量級フレームワークを採らない」方針は維持でよいか。

## English

### Verified facts
- The generated **Rust + Leptos (trunk)** template of `open-vsc-claude` is now **covered by an automated test**: `npm run test:leptos` (`scripts/e2e-leptos.js`) runs `trunk build` on the generated project, adds an item in real Chromium and checks it is listed.
  Both paths pass: browser → FastAPI subgraph (direct) and browser → **RPoem gateway (the forwarding added in `babc658`)** → FastAPI subgraph.
- RPoem **already has its own UI layer**: `open-runo-view` (virtual DOM, keyed diff, hooks, SSR, DOM applier; ~1,300 lines), `open-runo-crossui` (write the screen once, generate Windows/macOS/Linux/Android/Web), and `apps/desktop-wasm` (hand-written `wasm-bindgen` + `web-sys`).
- Ecosystem policy (`open-easy-web/CLAUDE.md`): the TypeScript/Node chain is retired; the frontend is Rust → WASM (`wasm-bindgen` + `web-sys`); **heavy frameworks such as Yew / Leptos / Dioxus are not adopted "unless there is a strong reason"**.

### Merits of Leptos (honestly)
- Mature reactivity (signals), SSR/hydration, typed components, a large ecosystem; no JS/TS in the app code.
- It demonstrably works (the test above).

### Costs and concerns
- Heavy dependency tree (first build takes minutes; Leptos plus the extra `trunk` tool). **Measured (2026-10-19, the open-upstream-watch dashboard)**: the release WASM is about 0.79 MB (`opt-level = "s"` + LTO, no `wasm-opt`) and the first trunk build takes about 3 minutes.
- API churn between versions (written against 0.7).
- **Overlaps `open-runo-view`** and clashes with the policy against heavy frameworks.

### Conclusion and recommendation
1. **Do not add Leptos as a dependency of RPoem itself (core or UI).** It goes against the policy and the existing `open-runo-view`, and no strong reason (something `open-runo-view` cannot do) has been shown yet.
2. Keep Leptos as an **experimental option among `open-vsc-claude`'s generated templates** (it concerns the user's generated project, not an RPoem dependency), protected by the automated test.
3. Get the "typed, no JS/TS" merit through **RPoem's own `open-runo-view` + `crossui`**, which fits the policy. Proposed next step (the Leptos-side numbers are measured above): add a `rust-runo-view` template to the extension that implements the same "list and add items" app with `open-runo-view`, then compare it with the Leptos one on **WASM size, build time and code size**. Only if the gap is large should the policy be reconsidered with the owner.
4. **Open question for the owner**: does the "no heavy frameworks" policy stand?
