# Phase 1: Headless Indexing Core - Context

**Gathered:** 2026-05-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Entregar um núcleo Rust headless que:

1. **Escaneia** uma pasta raiz `.md` recursivamente, respeitando lista de ignore (PRD §6.6 / RF-53 / IDX-01).
2. **Parseia** cada arquivo em dois estágios — segmenter line-based (TAB-indent + 2-space continuation, fence-aware) seguido por CommonMark/GFM por bloco — e extrai `[[link]]`, `#tag`, `#[[tag composta]]` apenas de nós de texto da AST (PRS-01..04, LNK-01..02).
3. **Constrói** índice SQLite com `files`, `pages`, `blocks` (incluindo `raw` + `byte_offset` + `byte_length` + `hash`), `tags`, `refs`, e FTS5 external-content por bloco (IDX-02, IDX-05, SCH-01 schema).
4. **Reindexa incrementalmente** por `mtime`+`hash`, com comando explícito `reindex` para reconstrução total (IDX-03, IDX-04).
5. **Preserva** block properties (`key:: value`) e Logseq drawers (`:LOGBOOK:`/`:END:`) parsed em slot estruturado por bloco, sem reformatar nem dropar (PRS-05, PRS-06).
6. **Expõe** um CLI `foliom` com subcomandos `index`, `reindex`, `search`, `dump-tree`, `inventory` — esse último (IDX-08) gera o relatório de patterns Logseq sobre a base real, gatekeeping o sign-off do parser.
7. **Garante** o CI gate de round-trip byte-idêntico (ACPT-01) contra o **corpus sintético committed** em `crates/core/tests/fixtures/logseq-synthetic/` (CI público, sem PII) e — opcionalmente, quando presente localmente — contra a base real `data-folder-sample/Logseq/` (gitignored, PII). ESTE TESTE É ESCRITO ANTES DE QUALQUER STORAGE / INDEXER / WATCHER e fica green daqui em diante. Ver D-08 para detalhes.
8. **Roda** parser + scanner verdes em Linux, macOS, Windows CI (ACPT-04), com path normalization NFC + forward-slash na fronteira de storage (IDX-07).

**Fora de Phase 1 (vai para phases posteriores):** servidor HTTP, renderer markdown, watcher de filesystem, write-back (byte-splice), CodeMirror, undo/redo, autocomplete, dark mode, journal navigation, packaging Tauri.

</domain>

<decisions>
## Implementation Decisions

### CLI shape e output
- **D-01:** **Binário único + subcomandos.** Um único executável `foliom` com subcomandos `index <root>`, `reindex`, `search <query>`, `dump-tree <page>`, `inventory <root>`. Padrão git/cargo, instalação simples, ergonomia familiar.
- **D-02:** **Output humano por padrão, `--json` opt-in.** Subcomandos `search`, `dump-tree`, `inventory` imprimem tabelas/snippets legíveis no terminal; passam para JSON estruturado em stdout quando recebem `--json`. CI gates e o frontend M1 (Phase 2) consomem o modo JSON; humanos consomem o modo padrão.

### Tag e link semantics (resolve PRD §12.1)
- **D-03:** **`#tag` e `[[página]]` são a MESMA entidade.** `#Crypto` e `[[Crypto]]` resolvem para a mesma `pages.id`. Modelo Logseq. A tabela `refs` mantém o campo `type ∈ {tag, page-link}` para preservar a distinção sintática na origem (e permitir renderização diferenciada como chip vs link), mas o `target_page_id` aponta para a mesma linha.
- **D-04:** **Páginas não-resolvidas existem em `pages` sem `file_id`.** Quando `#tag` ou `[[link]]` aponta para uma página que ainda não existe no disco, cria-se uma linha em `pages` com `file_id = NULL` ("unresolved page"). Quando o arquivo for criado depois (Phase 3 — write-back ou edição externa), o indexer apenas preenche o `file_id`. Backlinks resolvem desde o dia 1; a UI pode listar "unresolved pages" como afordance de criação.

### Data model granularity
- **D-05:** **Block properties parseadas em slot estruturado.** Cada `block` carrega `properties: Vec<(key, value)>` armazenado como JSON em uma coluna `properties` da tabela `blocks` (ou tabela auxiliar `block_props(block_id, key, value)` — decisão fica com o planner; ambas são válidas). Round-trip permanece byte-estável porque o write-back sempre usa `byte_offset/byte_length` do arquivo original. O index "enxerga" os pairs, habilitando query futura por `alias::`, `template::`, etc.
- **D-06:** **`:LOGBOOK:`/`:END:` drawers preservados como opaque blob anexado ao bloco pai.** Não são parseados em pairs (estrutura interna não importa para v1); ficam num campo `drawers: Vec<RawDrawer>` por bloco. Importante: a leitura preserva todas as linhas do drawer no `raw` do bloco, e os `byte_offset/byte_length` cobrem o drawer integralmente — assim o write-back via byte-splice nunca reordena/normaliza/perde drawer content.

### Workspace structure
- **D-07:** **Cargo workspace desde já.** Layout inicial:
  ```
  Cargo.toml                    # workspace manifest
  crates/
    core/                       # parser + storage + scanner + indexer + (futuro) watcher
      Cargo.toml                # pure lib, no HTTP, no UI
    cli/                        # binário `foliom`
      Cargo.toml                # depends on core, exposes subcommands
  ```
  Phase 2 adiciona `crates/server/` (axum) sem refactor; Phase 5 adiciona `crates/desktop/` (Tauri). Recomendação direta do ARCHITECTURE.md §1.

### Reference corpus para ACPT-01
- **D-08 (revised 2026-05-21):** **Round-trip CI gate tem dois corpora.**
  - **Primário (committed):** `crates/core/tests/fixtures/logseq-synthetic/` — 10 arquivos sintéticos pequenos, cada um isolando um padrão da §6.6 (TAB-indent, 2-space continuation com code fence, block properties, `:LOGBOOK:` drawer, `[[link]]` / `#tag` / `#[[tag composta]]`, falsos positivos `#fff`/URL/heading, `%2F` namespace, deep nesting, journal `YYYY_MM_DD.md`). Sem PII. CI matrix Linux/macOS/Windows roda contra ele.
  - **Secundário (opt-in, never committed):** `data-folder-sample/Logseq/` no root do repo, **gitignored** porque contém PII real. Quando a pasta existe localmente, o teste `roundtrip_byte_identical_for_real_corpus_if_present` valida o segmenter contra a base real (≈620 arquivos). Em CI a pasta não existe e o teste imprime "skipping" e passa.
  - **Por quê os dois:** o sintético garante cobertura determinística dos padrões críticos em CI público sem depender de PII; o real garante que a base que o usuário usa hoje não corrompe na primeira edição quando ele rodar localmente. Phase 1 fecha quando ambos passam (sintético em CI + real localmente).
  - Performance gates (ACPT-02 cold start <2s, ACPT-03 RAM <300MB) continuam em Phase 2.

### Já travado pela pesquisa (não re-discutir no planner)
- **D-09:** Linguagem: **Rust 1.85+**, edition 2024 quando disponível, MSRV declarado.
- **D-10:** Parser: **`pulldown-cmark` 0.13** (event stream + `into_offset_iter()` para byte spans).
- **D-11:** SQLite: **`rusqlite` 0.39 com feature `bundled`** — FTS5 vem compilado.
- **D-12:** Estratégia de parsing: **two-stage** — Stage 1 segmenter line-based custom (TAB + 2-space continuation, code-fence-aware); Stage 2 pulldown-cmark por bloco. Validado contra `data-folder-sample/Logseq/journals/2023_11_09.md` durante research.
- **D-13:** Localização do DB: **fora da pasta de notas**, default `$XDG_DATA_HOME/foliom/<root-hash>.db` (Linux), `~/Library/Application Support/foliom/<root-hash>.db` (macOS), `%LOCALAPPDATA%\foliom\<root-hash>.db` (Windows). `<root-hash>` = primeiros 16 chars do BLAKE3 do path absoluto da pasta de notas.
- **D-14:** Schema de `blocks` MATERIALIZADO (resolve PRD §12.3) com **ambos** `raw TEXT` (para FTS e leitura barata) **e** `byte_offset INTEGER` + `byte_length INTEGER` (para write-back por splice na Phase 3) — esses dois pares coexistem por design.
- **D-15:** Path normalization na fronteira: todos os paths armazenados em `files.path` são **NFC + forward-slash relativos** ao root. Conversão para o path nativo da plataforma só acontece quando vai para `std::fs`.
- **D-16:** Hashing: **BLAKE3** (crate `blake3`) para `files.hash` e `blocks.hash`. Mais rápido que SHA-256 e suficiente como cache-key.
- **D-17:** Walker: **`walkdir` 2.5+** com `filter_entry` para aplicar lista de ignore (`logseq/`, `assets/`, `draws/`, `whiteboards/`, `bak/`, `.recycle/`, `version-files/` + `:hidden` do `config.edn` se presente).
- **D-18:** Logging: **`tracing` + `tracing-subscriber`** com `RUST_LOG`-style filtering; default `info` em CLI release, `debug` em testes.
- **D-19:** Erros: **`thiserror`** nas crates lib (core), **`anyhow`** no binário cli.
- **D-20:** Migrations: **`rusqlite_migration`** — schema versionado via `user_version`, primeira migration cria todo o schema da Phase 1.
- **D-21:** Test runner: **`cargo-nextest`** para CI matrix; `insta` para snapshot tests; `criterion` para benchmarks (linha de base de cold start, mesmo que ACPT-02/03 só sejam gates em Phase 2).

### Claude's Discretion
Áreas onde o planner pode decidir sem voltar:
- Estrutura interna de módulos dentro de `crates/core/` (sub-módulos `scanner`, `parser`, `indexer`, `storage`, `query`).
- Naming das tabelas auxiliares vs colunas JSON (ex: `block_props` table vs `blocks.properties JSON`).
- Concorrência: parse pode rodar paralelo via `rayon` ou single-thread; benchmark decide. Storage writes ficam single-writer.
- Estratégia exata de transacionalidade do indexer (batch por arquivo vs single big tx).
- Mensagens de erro humanas e seus exit codes.
- Estrutura da feature `--json` (serde-driven structs explícitos vs `serde_json::Value` ad-hoc).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level
- `.planning/PROJECT.md` — Core Value, requirements ativas, key decisions
- `.planning/REQUIREMENTS.md` — IDs canônicos (IDX-01..08, PRS-01..07, ACPT-01, ACPT-04 são desta phase)
- `.planning/ROADMAP.md` — Phase 1 goal e success criteria
- `PRD-outliner-markdown.md` §5 (Decisões fixadas), §6.1 (Ingestão e indexação), §6.6 (Compatibilidade Logseq), §8 (Modelo de Dados), §12 (Decisões em aberto — §12.1 e §12.3 resolvidas neste CONTEXT)

### Research (já feito, leitura obrigatória do planner)
- `.planning/research/SUMMARY.md` — síntese executiva + tensão "block storage model" resolvida
- `.planning/research/STACK.md` — Rust stack com versões verificadas via Context7
- `.planning/research/ARCHITECTURE.md` — module boundaries, two-stage parser, build order, schema details
- `.planning/research/PITFALLS.md` — riscos críticos M0 (lossy round-trip, watcher loop, TAB-quirk CommonMark, drawers, path normalization, DB-em-folder-sync)
- `.planning/research/FEATURES.md` — categorização table-stakes vs deferred (Phase 1 não toca features de UX, mas o planner verifica que nada de M1+ vazou para M0)

### Sample data (parser/CI gate target)
- `crates/core/tests/fixtures/logseq-synthetic/` — **corpus sintético committed** (10 arquivos, sem PII) que cobre todos os padrões da §6.6. Target primário do ACPT-01; roda no CI matrix Linux/macOS/Windows.
- `data-folder-sample/Logseq/` (gitignored — PII) — 620 arquivos reais (533 journals + 86 pages + Untitled.md). Target secundário opt-in do ACPT-01; roda só quando a pasta existe localmente.
- `data-folder-sample/Logseq/journals/2023_11_09.md` (gitignored) — exemplo canônico de code fence dentro de bullet com 2-space continuation; teste manual durante o spike do segmenter. A fixture sintética equivalente é `crates/core/tests/fixtures/logseq-synthetic/pages/02-fence-in-bullet.md`.
- `data-folder-sample/Logseq/logseq/config.edn` (gitignored) — fonte de `:hidden`, `:journal/file-name-format`, `:journal/page-title-format` quando presentes (Phase 1 só lê `:hidden`; demais ficam para Phase 2).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
Greenfield — não há código existente além do PRD, CLAUDE.md e research artifacts. O planner começa Phase 1 do zero.

### Established Patterns
- **Atomic commits via `gsd-sdk query commit`** — padrão do projeto, todos os commits da Phase 1 seguem.
- **Português nas user-facing messages, inglês em código/identifiers/docstrings** — convenção implícita no PRD e CLAUDE.md.

### Integration Points
- **CLI consumida por (a) usuário direto via terminal, (b) CI gate (ACPT-01) via JSON output, (c) frontend de Phase 2 via JSON output sobre HTTP.** O contrato JSON deve ser estável a partir da Phase 1 — schema documentado em código (structs serde com `#[serde(rename_all = "camelCase")]`).

</code_context>

<specifics>
## Specific Ideas

- **Inventory CLI é gatekeeper do sign-off do parser.** Rodar `foliom inventory <root> --json` produz contagens de: `alias::`, `id::`, `:LOGBOOK:`, `#[[...]]`, `%2F` em filename, `template::`, code-fence-inside-bullet (ocorrências), `SCHEDULED:`/`DEADLINE:`, files com block properties total, files com drawers total. Em CI roda contra `crates/core/tests/fixtures/logseq-synthetic/` (snapshot fixo). Localmente o usuário pode rodar contra `data-folder-sample/Logseq/` para sanidade contra base real.
- **Round-trip property test** (`ACPT-01`) é o primeiro arquivo de teste a ser escrito, antes de qualquer storage ou indexer. Para cada arquivo no corpus sintético committed (CI sempre) e — se presente localmente — em `data-folder-sample/Logseq/` (opt-in), ler bytes → parsear em blocks → fazer "splice no-op" → assertar buffer byte-idêntico ao original. Esse teste fica VERDE pra sempre.

</specifics>

<deferred>
## Deferred Ideas

Coisas que apareceram mas pertencem a outras phases (não perder):

- **`alias::` resolution em `[[link]]`** — preservar opaque agora (D-05 cobre o storage); decidir em pós-Phase-3 se v1 ou v1.1 (recomendação SUMMARY: v1.1).
- **TODO/DONE/DOING/LATER/NOW workflow markers** — preservar como texto agora; decisão de virar estado de bloco com filtro/agenda fica para v2 (SUMMARY recomenda checkbox render em v1.x, sem agenda).
- **`SCHEDULED:` / `DEADLINE:` timestamps** — preservados verbatim no `raw` do bloco; estado queryable é v2.
- **`config.edn` reading completo** — Phase 1 lê só `:hidden`. Demais campos (`:journal/page-title-format`, `:pages-directory`, `:journals-directory`) ficam para Phase 2 quando o renderer precisar formatar títulos.
- **Páginas auto-geradas Logseq** (`excalidraw-*.md`, `hls__*.md`) — tratar como páginas comuns na Phase 1; decisão de ocultar UI fica para Phase 2.
- **Performance benchmarks contra corpus sintético de 5k notas** — Phase 1 só roda criterion baseline; gate formal (ACPT-02, ACPT-03) é Phase 2.

</deferred>
