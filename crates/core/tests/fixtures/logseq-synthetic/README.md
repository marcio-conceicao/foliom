# Synthetic Logseq Corpus

Pequena coleção de arquivos `.md` que cobre todos os padrões da §6.6 do PRD e os anti-patterns identificados em `.planning/research/PITFALLS.md`. Esses arquivos são o **target primário** do round-trip CI gate (ACPT-01): qualquer parser que feche o ciclo `read → segment → splice-noop → write` byte-idêntico aqui valida que respeita os contratos sem precisar de uma base Logseq real (que tem PII).

## Padrões cobertos

| Fixture | Padrão |
|---------|--------|
| `pages/01-simple-bullets.md` | TAB-indented bullets, hierarquia simples |
| `pages/02-fence-in-bullet.md` | Code fence multi-linha dentro de bullet com 2-space continuation (caso canônico de `journals/2023_11_09.md`) |
| `pages/03-block-properties.md` | `collapsed::`, `id::`, `alias::`, `template::`, `logseq.order-list-type::` |
| `pages/04-logbook-drawer.md` | `:LOGBOOK:` / `:END:` drawers + `SCHEDULED:` timestamps + workflow markers (`TODO`/`DONE`) |
| `pages/05-links-and-tags.md` | `[[página]]`, `#tag`, `#[[tag composta]]` inline mid-sentence |
| `pages/06-hex-url-heading-not-tag.md` | Falsos positivos que NÃO devem virar tag: hex color, URL fragment, code-fence content |
| `pages/Parent%2FChild.md` | Filename com `%2F` encoding (Logseq namespace) |
| `pages/08-empty-and-deep.md` | Bullets vazios, hierarquia profunda (depth 5) |
| `pages/09-combo.md` | Combinação: drawer + property + fence + tags + workflow marker no mesmo bloco |
| `journals/2024_03_15.md` | Página de journal com formato `YYYY_MM_DD.md` |

## Convenções

- LF line endings (`.gitattributes` força).
- Indentação por TAB literal, continuação multi-linha com 2 espaços (convenção Logseq).
- Não tem PII — cada arquivo é minúsculo e propositalmente artificial.

## Como adicionar uma fixture

1. Identificar o padrão que falta cobertura.
2. Criar arquivo pequeno (5–30 linhas) isolando aquele padrão.
3. Adicionar uma linha à tabela acima.
4. O round-trip test pega automaticamente — não precisa registrar manualmente.

## Real corpus opt-in

Se você tiver uma base Logseq real localmente em `data-folder-sample/` (ignorada pelo git), o round-trip test também roda contra ela como verificação extra. Em CI esse passo é skipped silenciosamente. Veja `crates/core/tests/roundtrip.rs` para detalhes.
