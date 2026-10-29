# Search

`oxispec search` finds nodes by the words in them, with [tantivy](https://github.com/quickwit-oss/tantivy) as the engine.

```bash
oxispec search session timeout
oxispec search "idle administrator" --type requirement --status draft
oxispec search REQ-AUTH-0001          # IDs and file paths are searchable too
oxispec search postgres --json -n 5
```

- **All words have to match.** A quoted phrase matches in order. A stray quote or colon does not make a query fail: what parses is searched.
- **Ranking** is BM25. An ID match counts most, then a title match, then the text.
- **English stemming.** `sessions` finds `session`. Other languages match on whole words only.
- **Filters.** `--type` and `--status` take exact values. Results show the ID, type, status, title, file and a snippet with the matched words in `**`.
- **Scope.** The working tree, as `check` sees it. Searching an old revision (`--at`) is not built yet.

## The index

The index is derived, and can be deleted at any time. It lives in `.oxispec/cache/search`, next to the [local index](format.md#local-index), and is brought up to date on every search: a node whose ID, type, status, title, owner or text changed is indexed again, a removed file is dropped, and nothing else is read. `--rebuild` starts over. A damaged index, or one written by a different tantivy version, is rebuilt without asking. With `--no-cache` the index is built in memory for that one search and nothing is written.

Measured on 20,000 requirement files (80 MB of text, macOS, release build):

| | Time |
|---|---|
| First search: parse every file, build both indexes | 4.5 s |
| Search with nothing changed | 0.35 s |
| Search by ID | 0.22 s |
| After editing 50 files | 0.5 s |
| `--rebuild` with a warm graph cache | 1.5 s |
| Size of the search index | 12 MB |

The first line is dominated by parsing the files, which `check` pays too. Search adds a second or two to it.

## Not built yet

Vector and hybrid search, other languages' stemmers, search at a past revision, and filtering by access rules. Those come with the MCP server and the server mode. The `TextIndex` trait from the design is not extracted yet: there is one implementation, and the trait will be cut from it when a second one (Postgres or OpenSearch) is added.
