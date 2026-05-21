# Foliom

## What This Is

Foliom é um app de notas **local-first** em estilo *outliner* (inspirado em Logseq/Roam) que usa arquivos `.md` em disco como fonte canônica e constrói uma rede de conhecimento via `#tags` e `[[links]]`. Entrega inicial é uma web app servida por um backend local; versão desktop (Tauri/Wails) virá depois reaproveitando a mesma UI.

## Core Value

**Cold start rápido e baixo uso de memória mesmo em grafos grandes, sem injetar metadados nos arquivos `.md`.** Essa é a dor primária que mata a experiência do Logseq hoje — se Foliom não resolver isso, não tem razão de existir.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Indexação incremental por `mtime`/`hash` com SQLite como cache derivado (descartável)
- [ ] Carregamento lazy de conteúdo — só o que está visível vai para memória
- [ ] Backend local servindo UI web em `localhost` (file IO + SQLite + watcher)
- [ ] Parser markdown bloco-a-bloco preservando indentação por TAB e continuação multi-linha
- [ ] Editor outliner: um bloco em edição (markdown cru) por vez; demais renderizados read-only
- [ ] Comandos de teclado: Enter/Shift+Enter/Tab/Shift+Tab/Backspace/setas
- [ ] Reconhecimento de `[[página]]`, `#tag` e `#[[tag composta]]` com extração via AST CommonMark/GFM
- [ ] Backlinks por página/tag via query no índice de referências
- [ ] Busca full-text via SQLite FTS5
- [ ] Watcher do sistema de arquivos com debounce + proteção contra auto-trigger
- [ ] Serialização árvore → markdown de bullets indentados preservando block properties opacos
- [ ] Compatibilidade de leitura com base Logseq existente (TAB, `key:: value`, `#[[tag]]`, journals `YYYY_MM_DD.md`, pastas ignoradas)
- [ ] Empacotamento desktop com binário leve (alvo: footprint < Electron equivalente)

### Out of Scope

- Compatibilidade com plugins do Logseq — fora do escopo da v1.
- Referências a blocos específicos `((uuid))` e IDs de bloco injetados no `.md` — quebra a portabilidade pretendida.
- Edição WYSIWYG com round-trip HTML → markdown — fonte da verdade é sempre o texto cru.
- Sync próprio ou colaboração em tempo real — delegado a Syncthing/git/Dropbox.
- App mobile — single-user desktop/web na v1.
- Org-mode — apenas markdown na v1.

## Context

- **Origem:** dor pessoal com Logseq (file-based) que reconstrói o grafo DataScript inteiro em memória no startup → lento em grafos grandes e RAM-hungry; Electron piora a sensação.
- **Base de notas real:** existe em `data-folder-sample/Logseq/` — 533 journals (`YYYY_MM_DD.md`) + 86 pages, com convenções concretas (indentação TAB, block properties `key:: value`, `#[[tag composta]]`, multi-linha por continuação 2-espaços, ausência total de `((uuid))` refs). PRD seção 6.6 lista requisitos de compatibilidade derivados dessa inspeção.
- **Usuário-alvo:** técnico, single-user, mantém base markdown e valoriza velocidade, controle dos arquivos e portabilidade. Confortável em editar `.md` por fora (git, VS Code, Syncthing).
- **PRD canônico:** `PRD-outliner-markdown.md` consolida visão, decisões fixadas, requisitos funcionais (RF-01 a RF-56), RNFs, modelo de dados, milestones M0–M4 e decisões em aberto (Seção 12).

## Constraints

- **Tech stack:** A decidir após research — candidatos são Rust (comrak/pulldown-cmark + rusqlite + notify + Tauri) ou Go (goldmark + go-sqlite3 + fsnotify + Wails). Frontend leve (Svelte/Solid/React) + CodeMirror 6 para edição.
- **Performance:** Cold start em poucos segundos com 5.000+ notas; RAM em repouso substancialmente menor que Logseq na mesma base.
- **Portabilidade:** `.md` gerados precisam abrir corretamente em Obsidian/VS Code/editor externo. Zero metadado proprietário injetado pelo Foliom.
- **Single-user, local-first:** Sem servidor, sem auth, sem multi-tenant. Sync delegado a ferramentas externas.
- **Compatibilidade:** Abrir base Logseq existente (~600 arquivos) sem corromper conteúdo na primeira edição.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Codinome do projeto = `foliom` | Alinha com o nome do diretório existente | ✓ Fixado |
| Arquivos `.md` são fonte canônica; SQLite é cache derivado descartável | Portabilidade + reconstrução sem perda | ✓ Fixado (PRD §5.1) |
| Reindexação incremental por `mtime`+`hash` | Custo de cold start independente do tamanho do grafo | ✓ Fixado (PRD §5.2) |
| Indexar tudo, carregar conteúdo sob demanda (lazy) | Baixo uso de RAM | ✓ Fixado (PRD §5.3) |
| Backend local nativo desde o dia 1, UI web em localhost | Contorna sandbox do browser + viabiliza desktop trivial via Tauri/Wails | ✓ Fixado (PRD §5.4) |
| Linking em nível de documento; sem ID de bloco no `.md` | Mantém arquivos limpos e portáveis | ✓ Fixado (PRD §5.6) |
| Stack backend (Rust vs Go) | Trade-off footprint vs produtividade | — Pendente (research) |
| Semântica de `#tag` vs `[[página]]` (mesma entidade ou dimensão separada?) | Afeta modelo de dados e UX de backlinks | — Pendente (Seção 12.1 do PRD) |
| Persistência dos blocos (derivar runtime vs materializar no índice) | Afeta complexidade do SQLite e custo de re-render | — Pendente (Seção 12.3 do PRD) |
| Política de block properties Logseq (`id::`, `alias::`, `template::`) | v1 preserva opacos; interpretar `alias::` afeta resolução de `[[link]]` | — Pendente (Seção 12.8 do PRD) |
| Workflow markers (`TODO`/`DONE`/`SCHEDULED:`) | v1 ignora como texto ou trata como estado de bloco? | — Pendente (Seção 12.9 do PRD) |
| Escopo de GFM (tabelas, code fence highlight, callouts) | Define quality bar do renderer M1 | — Pendente (Seção 12.5 do PRD) |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-05-21 after initialization*
