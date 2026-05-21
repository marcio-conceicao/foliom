# PRD — Outliner de Notas Local-First (Markdown)

> **Codinome do projeto:** `[definir]`
> **Versão:** 0.1 (rascunho inicial)
> **Data:** 21/05/2026
> **Autor:** `[preencher]`
> **Status:** Discovery / pré-implementação

---

## 1. Visão

Um app de notas **local-first** no estilo *outliner* (inspirado em Logseq/Roam), que usa arquivos `.md` em disco como **fonte canônica** e constrói uma rede de conhecimento por meio de `#tags` e `[[links]]`. O objetivo central é ser **rápido e leve**, corrigindo a principal dor da experiência atual com o Logseq: startup lento e alto uso de memória causados por carregar e parsear todo o grafo em memória a cada abertura.

A entrega inicial é uma aplicação **web servida por um backend local**; uma versão **desktop** virá depois reaproveitando a mesma UI.

---

## 2. Problema

- O Logseq (versão file-based) reconstrói o grafo inteiro em memória (DataScript) no startup → lento em grafos grandes e pesado em RAM.
- O Electron agrava a sensação de "peso" (Chromium + Node embutidos).
- Quero manter meus arquivos `.md` **legíveis, portáveis e editáveis por fora** (git, VS Code, Syncthing) — algo que a versão DB do Logseq sacrificou.

---

## 3. Objetivos e Não-objetivos

### 3.1 Objetivos (v1)
- **Cold start rápido**, idealmente independente do tamanho total do grafo.
- **Baixo uso de memória**: não manter todo o conteúdo carregado em RAM.
- `.md` permanece a **fonte da verdade**, limpo e sem metadados injetados.
- **Edição externa suportada**: mudanças feitas fora do app são refletidas.
- **Linking** por `#tag` e `[[página]]`, com **backlinks**.
- **Busca full-text** rápida.

### 3.2 Não-objetivos (v1 — explicitamente fora de escopo)
- ❌ Compatibilidade com plugins do Logseq.
- ❌ Referências a blocos específicos (`((uuid))`) e IDs de bloco injetados no `.md`.
- ❌ Edição WYSIWYG com round-trip (reconstruir markdown a partir de HTML renderizado).
- ❌ Sync próprio ou colaboração em tempo real (delegado a Syncthing/git/Dropbox).
- ❌ App mobile.

---

## 4. Usuário-alvo

Usuário técnico, **single-user**, que mantém uma base de notas em Markdown e valoriza velocidade, controle dos arquivos e portabilidade. Confortável em apontar o app para uma pasta e em editar os arquivos por fora quando quiser.

---

## 5. Decisões de Arquitetura (fixadas)

1. **Arquivos canônicos, índice derivado.** Os `.md` são a verdade. O SQLite é um **cache/índice derivado** — pode ser apagado e reconstruído a qualquer momento sem perda de dados.
2. **Reindexação incremental.** Por arquivo, guardar `mtime` + `hash` do conteúdo. No startup, fazer `stat` em todos os arquivos (barato) e **reparsear apenas os que mudaram**. Custo de abertura ≈ "stat de N arquivos + parse dos poucos sujos".
3. **Indexar tudo, carregar conteúdo sob demanda (lazy).** Metadados/tags/links/texto vão para o índice; o **conteúdo só é carregado em memória para o que está aberto/visível**.
4. **Backend local desde o dia 1.** Um processo nativo é dono do file IO, do SQLite, do watcher e do parsing, e serve a UI web em `localhost`. Isso contorna o sandbox do browser (que não lê pastas locais livremente) e torna a versão desktop trivial (envelopar a mesma UI).
5. **Watcher com proteção contra loop.** Observar o sistema de arquivos com *debounce* e **ignorar as próprias escritas** (rastrear o hash recém-gravado) para não disparar reparse em laço a cada save.
6. **Linking em nível de documento.** `#tag` e `[[página]]` resolvem para páginas/documentos. Como não há referência a bloco, **nenhum ID é escrito nos arquivos** — a hierarquia já é representada pela indentação dos bullets no próprio `.md`.

---

## 6. Requisitos Funcionais

### 6.1 Ingestão e indexação
- **RF-01** — Apontar o app para uma pasta raiz e varrer recursivamente os `.md`.
- **RF-02** — Construir índice SQLite com arquivos, páginas, tags, referências e texto (FTS).
- **RF-03** — Na abertura, reindexar incrementalmente (apenas arquivos com `mtime`/`hash` alterados).
- **RF-04** — Reconstrução total do índice sob demanda (comando "reindexar").

### 6.2 Editor (outliner)
- **RF-10** — Toda a página é uma árvore de **blocos** (bullets aninhados).
- **RF-11** — **No máximo um bloco em edição por vez.** O bloco em foco mostra **markdown cru** (textarea); os demais ficam **renderizados e read-only**.
- **RF-12** — Transição **render → edição** ao focar/clicar no bloco; **edição → render** no `blur` ou ao pressionar `Enter`, reparseando o `raw` daquele bloco.
- **RF-13** — A fonte da verdade do bloco é sempre o **texto cru**; o HTML é projeção descartável. **Nunca** reconstruir markdown a partir de HTML.
- **RF-14** — **Bloco ≠ linha:** `Enter` cria um bloco irmão; `Shift+Enter` insere quebra de linha **dentro** do mesmo bloco (um bloco pode ser multilinha, ex.: code fence).
- **RF-15** — Comandos de árvore: `Tab`/`Shift+Tab` = indent/outdent (altera hierarquia); `Backspace` no início do bloco = fundir com o anterior; setas `↑`/`↓` nas bordas = navegar para bloco vizinho entrando em edição.
- **RF-16** — Parsing **por bloco** (cada bloco é um mini-documento markdown independente).

### 6.3 Linking e navegação
- **RF-20** — Reconhecer `[[página]]` e `#tag` e renderizá-los como links navegáveis.
- **RF-21** — Extrair tags/links a partir da **AST CommonMark/GFM**, considerando **apenas nós de texto** — ignorar headings ATX (`# Título`), blocos de código, cores hex (`#fff`) e URLs.
- **RF-22** — Página de tag/página lista os **backlinks** (blocos que a referenciam), via query no índice de referências.
- **RF-23** — Clicar em link/tag navega para a página correspondente; criar página inexistente ao primeiro uso, se aplicável.

### 6.4 Busca
- **RF-30** — Busca full-text via **SQLite FTS5**, sem exigir conteúdo em memória.
- **RF-31** — Resultados com trecho/contexto e navegação direta ao bloco.

### 6.5 Sincronização com o disco
- **RF-40** — Detectar mudanças externas nos arquivos e atualizar o índice e a UI.
- **RF-41** — Persistir edições serializando a árvore de blocos de volta para **markdown de bullets indentados**, preservando o conteúdo intacto.

### 6.6 Compatibilidade com bases Logseq existentes

Derivado da inspeção da base real em `data-folder-sample/Logseq/` (533 journals + 86 pages). O objetivo **não** é compatibilidade total com Logseq — é abrir a base existente sem corromper conteúdo na primeira edição.

- **RF-50 — Convenção de indentação:** ler e escrever bullets aninhados com **TAB** (`\t`), não espaços. Esta é a convenção real da base; misturar quebra a hierarquia.
- **RF-51 — Continuação multi-linha de bloco:** linhas que seguem um bullet com **2 espaços** de continuação (sob o `- `) pertencem ao mesmo bloco. Caso-teste obrigatório do parser: code fence (```` ``` ````) dentro de um bullet.
- **RF-52 — Tag composta:** reconhecer `#[[tag com espaços]]` além de `#tag`. Renderizar **inline como pílula clicável** no meio do texto (não só no início/fim do bloco).
- **RF-53 — Pastas ignoradas na varredura:** `logseq/`, `assets/`, `draws/`, `whiteboards/`, `bak/`, `.recycle/`, `version-files/`. Adicionalmente, respeitar a lista `:hidden` do `config.edn` se presente.
- **RF-54 — Block properties opacos (`key:: value`):** preservar como texto literal no `.md` (não quebrar na re-serialização). Não é necessário interpretá-los na v1 — basta não destruí-los. Inclui: `id::`, `collapsed::`, `alias::`, `template::`, `logseq.order-list-type::`, `file::`, etc.
- **RF-55 — Título de página de journal:** o nome de arquivo é `YYYY_MM_DD.md` mas o título exibido é a data formatada por extenso (ex.: "May 21st, 2026"). Formatter configurável; default em inglês compatível com `:journal/page-title-format` do Logseq.
- **RF-56 — Filename de página comum:** o nome de arquivo (sem `.md`) é o nome da página. Suportar espaços, acentos e `&` no filename.

---

## 7. Requisitos Não-Funcionais

- **RNF-01 — Performance:** cold start na casa de poucos segundos mesmo com milhares de arquivos; abertura de página perceptivelmente instantânea.
- **RNF-02 — Memória:** consumo aproximadamente proporcional ao que está aberto, não ao tamanho total do grafo.
- **RNF-03 — Portabilidade dos dados:** os `.md` gerados devem ser legíveis e válidos em qualquer editor de markdown.
- **RNF-04 — Robustez:** apagar o índice nunca causa perda de dados (reconstruível a partir dos arquivos).
- **RNF-05 — Footprint do desktop:** binário e RAM significativamente menores que uma app Electron equivalente.

---

## 8. Modelo de Dados (índice SQLite — derivado)

Esquema mínimo de partida (sujeito a refino):

- `files(id, path, mtime, hash)`
- `pages(id, name, file_id, ...)`
- `blocks(id, page_id, parent_id, order, raw, ...)`  *(opcional na v1 se a árvore for derivada em runtime)*
- `tags(id, name)`
- `refs(source_id, target, type)`  — `type ∈ { tag, page-link }`
- `notes_fts(...)` — tabela **FTS5** para busca textual

> **Backlink de X** = `SELECT source_id FROM refs WHERE target = X`.
> A hierarquia dos blocos é representada pela indentação no `.md`; o índice apenas a espelha para consultas.

---

## 9. Stack Técnica (proposta)

- **Backend/núcleo:** **Rust** (`comrak` ou `pulldown-cmark` para parsing, `rusqlite` + FTS5, `notify` para o watcher). *Alternativa:* **Go** (`goldmark`, `go-sqlite3`, `fsnotify`) se a prioridade for produtividade.
- **Frontend (web):** framework leve (**Svelte** ou **Solid**; React se quiser ecossistema) + **CodeMirror 6** para o bloco em edição + um renderer de Markdown para os blocos inativos.
- **Desktop (fase posterior):** **Tauri** (combina com backend Rust e é leve — o ponto é justamente fugir do Electron). *Alternativa:* Wails (Go).

---

## 10. Fases / Milestones

- **M0 — Núcleo de indexação (headless):** varredura da pasta, parser de markdown, índice SQLite, reindex incremental por `mtime`/`hash`. *Testável via CLI/teste, sem UI.*
- **M1 — Leitura na UI web:** servir páginas renderizadas (read-only), navegação por `[[link]]`/`#tag`, backlinks, busca FTS.
- **M2 — Editor outliner:** máquina de estados render ↔ edição por bloco, comandos de teclado (Enter/Shift+Enter/Tab/Backspace/setas), serialização árvore → `.md`.
- **M3 — Sync com disco:** watcher com debounce e proteção contra auto-trigger; refletir edições externas.
- **M4 — Empacotamento desktop:** envelopar a UI no Tauri reaproveitando o backend.

---

## 11. Métricas de Sucesso

- Tempo de cold start com um grafo de referência (ex.: 5.000+ notas) abaixo de um alvo definido.
- Uso de RAM em repouso substancialmente menor que o Logseq na mesma base.
- Edição externa (ex.: `git pull`, salvar no VS Code) refletida no app sem reabrir.
- `.md` gerados abrem corretamente em Obsidian/editor externo (validação de portabilidade).

---

## 12. Decisões em Aberto (resolver antes/durante M0)

1. **Semântica da hashtag:** `#tag` é a mesma entidade que `[[página]]` (modelo Logseq) ou uma dimensão de classificação separada?
2. **Caret no clique:** ao entrar em edição via clique num bloco renderizado, o cursor cai na posição clicada ou no fim do bloco? *(v1 pode aceitar "fim do bloco".)*
3. **Persistência dos blocos:** a árvore é derivada em runtime a partir do `.md` (sem tabela `blocks`) ou materializada no índice?
4. **Convenção de arquivos por página:** um arquivo por página? Suporte a `journals/` (`YYYY_MM_DD.md`) e `pages/` como no Logseq? Org-mode fica fora?
5. **Escopo de markdown suportado:** GFM completo? Tabelas, code fences com **syntax highlighting + numeração de linha**, callouts?
6. **Estratégia de sync de longo prazo:** confirmar que fica delegada a ferramentas externas (Syncthing/git) na v1.
7. **Formatter de título de journal:** default ("May 21st, 2026") e como expor a configuração ao usuário. Ler de `config.edn` quando presente?
8. **Block properties (`key:: value`):** v1 preserva opaco (RF-54). Decidir quando/se interpretar `alias::` (afeta resolução de `[[link]]`) e `id::` (referência futura a bloco — hoje fora de escopo).
9. **Workflow markers (`TODO`/`DONE`/`DOING`/`LATER`/`NOW`):** ignorar como texto na v1 ou tratar como estado do bloco (com filtro/agenda)? `SCHEDULED:`/`DEADLINE:` com timestamps `<YYYY-MM-DD Day>` entram junto?
10. **Macros/embeds (`{{cloze}}`, `{{renderer ...}}`, `{{video ...}}`):** renderizar como texto literal vs ignorar vs suporte mínimo (ex.: `{{video}}`)?
11. **Páginas auto-geradas pelo Logseq** (`excalidraw-*.md`, `hls__*.md`): tratar como páginas comuns, ocultar por padrão, ou ignorar na varredura?

---

*Este documento consolida as decisões de design discutidas e serve como ponto de partida para desenvolvimento assistido por IA. Os itens da Seção 12 devem ser fixados conforme o projeto avança.*
