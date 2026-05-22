# Foliom Frontend

SPA Svelte 5 + Vite + TypeScript que consome a API REST do backend Foliom.

## Requisitos

- **Node.js 20+** (testado em 20.x e 24.x)
- **npm 10+**

## Scripts

| Comando             | O que faz                                                        |
| ------------------- | ---------------------------------------------------------------- |
| `npm run dev`       | Sobe Vite em `http://localhost:5173` com HMR e proxy `/api → 7345`. |
| `npm run build`     | Produz `dist/` (consumido por `rust-embed` no plan 02-07).       |
| `npm run preview`   | Serve `dist/` localmente para smoke check do bundle produzido.   |
| `npm run test`      | Roda vitest com ambiente happy-dom.                              |
| `npm run check`     | Type-check via `svelte-check`.                                   |

## Fluxo de desenvolvimento

1. **Suba o backend** num terminal:
   ```bash
   cargo run -p foliom-cli -- serve crates/core/tests/fixtures/logseq-synthetic --port 7345
   ```
2. **Suba o frontend** noutro terminal:
   ```bash
   cd frontend && npm run dev
   ```
3. Abra `http://localhost:5173` — o Vite dev server reencaminha todas as chamadas `/api/*` para `http://127.0.0.1:7345/api/*`.

## Invariante de build

O `cargo build` do binário final (plan 02-07) **embute** `frontend/dist/` via `rust-embed`. Sempre que o frontend mudar, rode `npm run build` ANTES do `cargo build`. Em CI a sequência é sempre `npm ci && npm run build && cargo build`.

Para garantir que `cargo check` funcione em clones limpos antes do primeiro `npm run build`, mantemos `frontend/dist/.gitkeep` comitado — o macro `rust-embed` precisa de uma pasta existente.

## Stack

- **Svelte 5.37+** com runes (`$state`, `$derived`, `$effect`) — sem `export let`.
- **svelte-spa-router 5.x** para roteamento hash-based (`/#/pages/...`, `/#/journals/...`, `/#/search`).
- **markdown-it 14** para renderização per-block (chega no plan 02-04).
- **Prism 1.29** para syntax highlighting em code fences (plan 02-04).
- **vitest 2 + happy-dom** para smoke tests.
