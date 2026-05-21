# Phase 1: Headless Indexing Core - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-21
**Phase:** 01-headless-indexing-core
**Areas discussed:** CLI shape, Tag semantics, Block properties model, Round-trip corpus, CLI output format, Unresolved tag handling, Workspace layout

---

## CLI Shape

| Option | Description | Selected |
|--------|-------------|----------|
| Binário único + subcomandos | `foliom index <root>`, `foliom reindex`, `foliom search`, `foliom dump-tree`, `foliom inventory` — padrão git/cargo | ✓ |
| Binários separados por verbo | `foliom-index`, `foliom-search`, etc. — favorece composição shell mas polui PATH | |
| Binário único + REPL | `foliom` abre prompt interativo — melhor para explorar, pior para scripts/CI | |

**User's choice:** Binário único + subcomandos (Recommended)
**Notes:** Família de comandos clara, instalação simples, ergonomia familiar.

---

## Tag Semantics (resolve PRD §12.1)

| Option | Description | Selected |
|--------|-------------|----------|
| Mesma entidade | `#Crypto` e `[[Crypto]]` apontam para a mesma página; backlinks se mesclam. Modelo Logseq | ✓ |
| Entidades separadas | Tag e página são coisas diferentes; backlinks separados. Mais expressivo, dobra complexidade da UI | |

**User's choice:** Mesma entidade (Recommended)
**Notes:** Refs schema mantém `type ∈ {tag, page-link}` para preservar a distinção sintática (renderização chip vs link), mas o `target_page_id` aponta para a mesma linha.

---

## Block Properties Model

| Option | Description | Selected |
|--------|-------------|----------|
| Parsed em `properties: Vec<(k,v)>` | Slot estruturado por bloco; permite query futura por property. Round-trip ainda byte-estável via byte_offset | ✓ |
| Raw text opaco | Linhas `key:: value` ficam dentro do `raw`, sem estrutura no índice. Mais simples mas reparse depois | |

**User's choice:** Parsed em `properties: Vec<(k,v)>` (Recommended)
**Notes:** Drawers (`:LOGBOOK:`/`:END:`) ficam como blob opaque anexado ao bloco pai, não parseados em pairs. Byte-offset cobre o drawer integralmente.

---

## Round-Trip Corpus para ACPT-01

| Option | Description | Selected |
|--------|-------------|----------|
| Só `data-folder-sample/Logseq/` | ~619 arquivos reais. Foco em correção; perf gate fica em Phase 2 | ✓ |
| Real + corpus sintético de 5k | Antecipa ACPT-02 (cold start <2s) em Phase 1; atrasa entrega | |

**User's choice:** Só `data-folder-sample/Logseq/` (Recommended)
**Notes:** Phase 1 prova correção contra a base que o usuário tem hoje; performance fica para Phase 2.

---

## CLI Output Format

| Option | Description | Selected |
|--------|-------------|----------|
| Humano por padrão, `--json` opt-in | Tabelas/snippets no terminal; JSON estruturado com `--json`. Padrão cargo/git | ✓ |
| JSON sempre, pretty no terminal | Output sempre JSON; pretty quando TTY. Consistente para CI, menos amigável | |
| Humano só | Sem JSON; CI script faz parsing de texto. Mais simples mas frágil | |

**User's choice:** Humano por padrão, `--json` opt-in (Recommended)
**Notes:** Contrato JSON estável a partir da Phase 1 — schema documentado em código (structs serde com `camelCase`).

---

## Unresolved Tag / Link Handling

| Option | Description | Selected |
|--------|-------------|----------|
| Cria linha em `pages` sem `file_id` | Backlinks resolvem; `file_id` preenchido quando arquivo é criado. Modelo Logseq | ✓ |
| Mantém só em `refs.target` como string | Sem linha em `pages`; backlinks via LEFT JOIN. Mais minimalista | |

**User's choice:** Cria linha em `pages` sem `file_id` (Recommended)
**Notes:** Habilita UI "unresolved pages" no Phase 2 sem refactor de schema.

---

## Cargo Workspace Layout

| Option | Description | Selected |
|--------|-------------|----------|
| Cargo workspace desde já | `crates/core/` + `crates/cli/`. Phase 2 adiciona `crates/server/` sem refactor | ✓ |
| Single crate, split depois | Um `Cargo.toml` no root com lib + bin. Mais rápido pra começar; reorganiza em Phase 2 | |

**User's choice:** Cargo workspace desde já (Recommended)
**Notes:** Recomendação direta do ARCHITECTURE.md §1; evita refactor entre M0 e M1.

---

## Claude's Discretion

- Estrutura interna de módulos dentro de `crates/core/`.
- Naming de tabelas auxiliares vs colunas JSON (`block_props` table vs `blocks.properties JSON`).
- Estratégia de concorrência (parse paralelo via rayon vs single-thread).
- Estratégia exata de transacionalidade do indexer.
- Mensagens de erro humanas e exit codes.

## Deferred Ideas

- `alias::` resolution em `[[link]]` → v1.1 (research recommendation).
- TODO/DONE workflow markers → preservar verbatim agora; estado de bloco fica v2.
- `SCHEDULED:`/`DEADLINE:` timestamps → verbatim no raw agora; queryable é v2.
- `config.edn` reading completo → Phase 1 lê só `:hidden`; demais campos em Phase 2.
- Páginas auto-geradas Logseq (`excalidraw-*`, `hls__*`) → tratar como comuns agora; ocultação UI em Phase 2.
- Performance benchmarks contra corpus sintético 5k → criterion baseline agora; gate formal em Phase 2.
