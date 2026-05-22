# Phase 2: Read-Only Web UI - Context

**Gathered:** 2026-05-21 (auto mode — recommended choices applied from research)
**Status:** Ready for planning

<domain>
## Phase Boundary

Entregar a primeira interface visível do Foliom: um app web read-only que serve a UI em `http://localhost:<port>` consumindo o índice SQLite que Phase 1 construiu. O usuário:

1. **Inicia** o servidor local com `foliom serve <root>` — o mesmo binário ganha um novo subcomando que faz `index/reindex` automático no startup, depois sobe axum em uma porta livre e abre o browser default (best-effort).
2. **Navega** páginas/journals via `[[link]]`, `#tag`, `#[[multi-word tag]]` renderizados como chips clicáveis inline mid-sentence (LNK-01).
3. **Vê backlinks** em cada página, listando blocos que a referenciam, queryados via `refs` (LNK-03).
4. **Encontra qualquer coisa** via palette `Ctrl/Cmd+K` que faz FTS5 search com snippet highlight (SCH-01..03).
5. **Lê journals formatados** com título por extenso ("May 21st, 2026") e navegador de journals com "open today" (LNK-05, LNK-06).
6. **Zooma em um bloco** via URL fragment `#block=<indent-path>` (LNK-07).
7. **Vê o markdown renderizado** com GFM, syntax highlighting em code fences, guias de indentação entre bullets, dark mode default-OS (UI-01..04).
8. **Sente que é rápido** — cold start <2s em 5.000 notas (ACPT-02), RSS idle <300MB (ACPT-03), conteúdo carregado lazy (só visível em memória).

**Fora de Phase 2 (vai para Phase 3+):** edição (CodeMirror, undo/redo, autocomplete, page rename), watcher, write-back via byte-splice, Tauri packaging.

</domain>

<decisions>
## Implementation Decisions

### Backend (extension of Phase 1 core)
- **D-22 (Phase 2):** **Novo subcomando `foliom serve <root>` no binário existente** (não cria binário separado). O subcomando: (a) chama `indexer::reindex(Incremental)` no startup, (b) abre Db wrapper persistente, (c) sobe axum em `127.0.0.1:<port>` (random free port ou `--port N` flag, default 7345), (d) imprime `Foliom serving at http://127.0.0.1:7345 — Ctrl+C to stop`, (e) opcionalmente abre browser default via crate `open = "5"` (atrás de flag `--open`).
- **D-23:** **axum 0.7 + tower-http** para CORS (irrelevante aqui — same-origin) e static asset serving. O frontend Svelte compilado vai embutido no binário via `rust-embed = "8"` (uma única dep, copy at compile-time, sem dependência de `dist/` em runtime). Em dev mode (`cargo run`), axum delega `/` para `http://localhost:5173` (Vite dev server) via reverse proxy ou redirect — escolher no plan.
- **D-24:** **REST + JSON, sem SSE/WebSocket em Phase 2.** Read-only não precisa de push. SSE entra em Phase 4 (Disk Sync). Routes propostas (planner pode ajustar):
  - `GET /api/pages` → lista de páginas (name, file_id, is_resolved, is_journal)
  - `GET /api/pages/{name}` → conteúdo da página (blocks árvore + properties + drawers; rendered HTML server-side OR raw para render client-side — D-26 decide)
  - `GET /api/pages/{name}/backlinks` → lista de blocos que referenciam essa página
  - `GET /api/journals/today` → redirect 302 para `/api/pages/{YYYY_MM_DD}`
  - `GET /api/journals?from=&to=` → lista de journal pages para navegação
  - `GET /api/search?q=&limit=` → FTS5 hits com snippet + page name + block_id
  - `GET /api/page-titles` → autocomplete-ready list para search palette
- **D-25:** **Concorrência:** single-threaded `tokio::current_thread` é suficiente. Read-only + single user. Storage queries são síncronas via `rusqlite` envoltas em `tokio::task::spawn_blocking` no handler boundary.

### Frontend
- **D-26:** **Markdown rendering acontece no frontend, não no backend.** O backend serve `raw` markdown por bloco (com properties/drawers já parseados em estrutura); o frontend renderiza com `markdown-it` per-block. Razão: (a) preserva a regra "raw é fonte da verdade" para preparar Phase 3 (mesmo bloco renderizado read-only ↔ editado em CodeMirror), (b) reduz payload (markdown < HTML rendered), (c) keeps backend simpler (no HTML escaping concerns).
- **D-27:** **Svelte 5 com runes (`$state`, `$derived`)** + **Vite 5** + **TypeScript 5.5+**. Layout monorepo: workspace ganha `frontend/` no root (não dentro de `crates/`) com seu próprio `package.json`. Build artifact em `frontend/dist/` é embutido em `crates/cli` via `rust-embed` (D-23).
- **D-28:** **Router client-side:** `svelte-spa-router` (hash-based: `/#/pages/Foo`, `/#/journals/2024-03-15`, `/#/search`). Hash routing evita configurar fallback no axum e mantém URLs estáveis quando refresh. Block zoom usa fragment dentro do hash: `/#/pages/Foo#block=2.1` (zoom é decoração; não usa svelte-spa-router para o fragment).
- **D-29:** **Markdown-it inline rules custom** para `[[page]]`, `#tag`, `#[[tag composta]]`. Configurado uma vez na app, render per-block. Output: `<a class="page-link" data-page="Foo">Foo</a>` e `<span class="tag" data-tag="Foo">#Foo</span>`. CSS faz o chip styling.
- **D-30:** **Code fence highlighting:** **Prism** (`prismjs = "1.29"`) com languages comuns (rust, python, js, ts, sh, sql, json, yaml, html, css) carregadas estaticamente. Line numbers via `prism-line-numbers` plugin. Decisão sobre Shiki/starry-night: descartadas — Prism é menor (~30KB minified com top 10 langs vs Shiki ~200KB+ pelos themes).
- **D-31:** **Dark mode:** CSS variables + `prefers-color-scheme` media query default; toggle via `localStorage('theme')` override stored como `'light' | 'dark' | 'auto'`. Não usar `Tailwind` ou outro framework CSS — vanilla CSS + custom-properties cabe perfeitamente, mantém bundle pequeno.
- **D-32:** **State management:** Svelte stores built-in (`writable`, `derived`). Nenhum store externo (Pinia/Zustand/Redux). Stores principais:
  - `currentPage: writable<PageDetail | null>` — atualiza no route change
  - `sidebarPages: writable<PageSummary[]>` — fetched uma vez no startup, refreshed on demand
  - `theme: writable<'light' | 'dark' | 'auto'>` — bound to localStorage
  - `searchPalette: writable<{ open: boolean; query: string }>` — controla `Ctrl+K`
- **D-33:** **Renderer per-block, não per-page.** Cada bloco vai num componente Svelte que recebe `{ raw: string, properties: [k,v][], drawers: [], depth: number, children: Block[] }` e renderiza HTML via `{@html markdownIt.render(stripped_raw)}` (após strip do prefixo `\t*- ` que veio do segmenter). Properties/drawers nunca rendered visíveis — só preservados como data attributes.
- **D-34:** **Block folding (EDT-08 Phase 2 scope):** ícone collapse/expand no bullet à esquerda. Estado UI-only por default; quando o user explicitamente persiste, faz PUT/PATCH... espera, Phase 2 é read-only. Reescolho: **UI-only fold em Phase 2** (sem persistência); persistência via `collapsed::` property fica para Phase 3 quando há write-back.

### Performance gates
- **D-35:** **5k-note generated corpus** vive em `crates/core/benches/fixtures/synthetic-5k/` (gitignored — gerado on-demand via `cargo run --bin foliom-bench-gen`). O gerador é um helper bin em `crates/cli/src/bin/bench-gen.rs` que produz 5000 `.md` files com estrutura realista (mix de journals + pages + variadas profundidades + tamanhos). Criterion benchmark (`crates/core/benches/cold_start.rs`) mede `Db::open + reindex(Full)` time contra esse corpus. ACPT-02 e ACPT-03 viram CI assertion (com tolerância de 50% sobre o target devido a CI variance vs reference laptop).
- **D-36:** **Lazy loading:** o frontend só fetcha `/api/pages/{name}` para a página corrente; backlinks, refs e search são queries separadas. Sidebar lista só nomes (não conteúdo). 5k notas → sidebar é ~5000 strings de nome, ~50KB JSON — aceitável carregar de uma vez.

### Já travado / canônico desde Phase 1
- **D-37:** Backend HTTP usa `RelativePath` para todos os paths externos (URLs encodam `/Parent%2FChild`, decodam ao bater no DB). `RelativePath::from_filesystem` continua sendo o boundary de NFC + forward-slash.
- **D-38:** Db é `Send + Sync`-compatible apenas via `Mutex<Connection>` ou pool. Phase 2 single-writer + few-readers → `Arc<Mutex<Db>>` no `axum::State` é suficiente; pool fica para Phase 3 se contention aparecer.

### Claude's Discretion (planner pode decidir sem voltar)
- Estrutura interna de módulos em `crates/cli/src/serve/` (handlers, middleware, embed).
- Naming dos endpoints (`/api/pages/{name}` vs `/api/page/{name}` etc.).
- Forma exata dos structs de response JSON (com `#[serde(rename_all = "camelCase")]`).
- Layout exato do CSS (sidebar largura, cores, espaçamento).
- Mecânica de scroll/anchor para `#block=` deep links.
- Como o `--open` flag detecta o browser em cada OS.
- Granularidade dos benchmarks Criterion (1 grande vs N small).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level
- `.planning/PROJECT.md` — Core Value, requirements ativas
- `.planning/REQUIREMENTS.md` — IDs canônicos (LNK-01..03,05..07, SCH-01..03, UI-01..04, EDT-08, ACPT-02, ACPT-03, IDX-04 partial)
- `.planning/ROADMAP.md` — Phase 2 goal e success criteria
- `PRD-outliner-markdown.md` §6.3 (Linking/navegação), §6.4 (Busca), §12.1 (resolvido — D-03 do Phase 1), §12.5 (escopo GFM — code-fence highlight YES, callouts NO)

### Phase 1 contracts (consumidos pelo backend HTTP)
- `crates/core/src/parser/segment.rs` — `RawBlock { raw, byte_offset, byte_length, properties, drawers, depth }`
- `crates/core/src/parser/ast.rs` — `extract_refs(raw) -> Vec<ExtractedRef>`
- `crates/core/src/path.rs` — `RelativePath` (NFC + forward-slash boundary)
- `crates/core/src/storage/mod.rs` — `Db::open`, `Db::version`, schema with `blocks/pages/refs/tags/blocks_fts`
- `crates/core/src/indexer/mod.rs` — `reindex(db, root, ReindexMode)` returns `IndexStats`
- `crates/core/src/inventory.rs` — `InventoryReport` (não usado em Phase 2 mas referência de contrato JSON com serde)
- `crates/core/src/scanner/mod.rs` — `scanner::walk` (Phase 2 usa indiretamente via reindex)

### Research (project-level)
- `.planning/research/SUMMARY.md` — Stack lock, módulo boundaries
- `.planning/research/STACK.md` — Svelte 5 + CM6 + axum + Tauri (CM6/Tauri ficam Phase 3+)
- `.planning/research/ARCHITECTURE.md` §3 (REST+SSE pattern; SSE postponed para Phase 4), §5 (build order — Phase 2 = mutation NÃO, só query layer)
- `.planning/research/FEATURES.md` — Phase 2 entrega features categorizadas como "Read-only M1 set"
- `.planning/research/PITFALLS.md` — Pitfall 6 (path normalization cross-platform), Pitfall 9 (lazy-loading regressões em CI), Pitfall 12 (markdown-it custom rules)

### Sample data (development)
- `crates/core/tests/fixtures/logseq-synthetic/` — corpus sintético para testes de integração end-to-end (frontend + backend)
- `data-folder-sample/Logseq/` (gitignored — PII) — base real para dogfooding local com 620 arquivos
- `image.png` (gitignored — PII) — screenshot de referência da expectativa de UX (chips inline, code fence rendering, indentation guides)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets (Phase 1)
- **`Db` struct** já é `Send + Sync` via `Mutex<Connection>` internamente — pode entrar direto no `axum::State` envolvido em `Arc`.
- **`indexer::reindex`** já existe e é idempotente — `foliom serve <root>` chama no startup sem condicionais especiais.
- **`extract_refs`** já distingue `Tag` vs `PageLink`; o handler de página renderiza `#chip` vs `[[link]]` differently.
- **`RelativePath` + `Db::open` resolve DB location per-OS** — quando o usuário roda Foliom de Windows nativo vs WSL aponta DBs separados (D-13). Já documentado.
- **`crates/core/src/inventory.rs`** mostra o padrão de serde structs com `#[serde(rename_all = "camelCase")]` — replicar para os response types do HTTP.

### Established Patterns (a respeitar)
- **Português nas user-facing messages, inglês em código/identifiers.** O frontend SPA precisa decidir: i18n agora ou só Portuguese inicial? Decisão Claude's-discretion → **Português-only por enquanto**; i18n é v1.x.
- **CI matrix Linux/macOS/Windows** — o frontend tem que buildar nos três OSes também. Vite + Svelte é cross-platform out of the box, mas `npm install` em Windows com path longos pode ser frágil. Cuidar no CI workflow.
- **`gsd-sdk query commit`** para todos os commits atômicos.

### Integration Points
- Backend HTTP → Frontend: REST JSON via fetch. Em dev (`vite dev`), Vite proxy `/api/*` → `http://127.0.0.1:7345/api/*`. Em prod (binário Tauri-future), backend serve em `127.0.0.1:<port>` e Vite-built static está embutido via rust-embed.
- Frontend → Browser storage: só `localStorage` para `theme`. Sem cookies, sem IndexedDB.
- CI matrix: precisa do Node.js setup step ANTES do cargo test, porque o frontend `dist/` precisa estar buildado quando o `rust-embed` macro roda no `cargo build`. Encadear `npm ci && npm run build` → `cargo test --workspace`.

</code_context>

<specifics>
## Specific Ideas

- **Layout visual:** três zonas — sidebar esquerda (page list + journal navigator + recents + favorites), main content area (página atual: título + blocos), e sem painel direito em Phase 2 (Phase 3+ pode adicionar reference pane). Backlinks ficam embaixo do main content (collapsible section). Search palette é modal centralizado overlay.
- **Tipografia:** font default `system-ui` para chrome + `ui-monospace` para code. Sem font loading externo (perf).
- **Indentation guides:** linha vertical sutil (`border-left: 1px solid var(--guide-color)`) à esquerda de cada bullet aninhado, conforme o screenshot de referência (image.png) do PRD.
- **Chip styling para `#tag` e `[[page]]`:** pílulas com `border-radius: 4px`, background tom-on-tom, hover indica clickable.
- **Code fence rendering:** Prism com tema "tomorrow night" no dark mode, "github" no light mode; line numbers à esquerda; language label no canto superior direito (conforme image.png).
- **Search palette UX:** `Ctrl+K` (Mac: `Cmd+K`) abre modal com input focado; resultados streaming conforme tipa (debounce 150ms); `↑/↓` navega; `Enter` abre o resultado e fecha modal; `Esc` fecha sem navegar.
- **Tag/link unresolved:** chips de páginas que não existem em disco aparecem com style diferente (italic + opacity 0.6) — afordância visual para "isso vira página se você criar".

</specifics>

<deferred>
## Deferred Ideas

Capturados durante a sessão de discussão; não-objetivos da Phase 2:

- **Real-time updates via SSE** — Phase 4 (Disk Sync).
- **Edição via CodeMirror 6** — Phase 3 (Outliner Editor).
- **`collapsed::` block-property persistence** — Phase 3 (precisa de write-back).
- **Page rename + backlink rewrite (SNC-05)** — Phase 3.
- **Drag-and-drop block reorder** — v1.x (cut/paste cobre v1).
- **Slash commands em-block** — v1.x.
- **Command palette beyond Ctrl+K search** — v1.x.
- **i18n** (Portuguese-only em Phase 2) — v1.x.
- **Atalhos de teclado para navegação entre páginas** (J/K like) — v1.x.
- **Graph view** — anti-feature (research SUMMARY).
- **TODO/DONE como estado de bloco com filtro/agenda** — v2 (research SUMMARY).
- **Renderização de `{{cloze}}`, `{{renderer ...}}`, `{{video ...}}`** — v2.

</deferred>
