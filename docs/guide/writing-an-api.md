# Describing an API

An API is described as requirements, one block per operation, titled as the call. The skeleton of its OpenAPI description is made from them, and a description kept in the code can be checked against them.

## Write the operations

```bash
oxispec new requirement orders "Orders API"
```

Put one block per operation in the node:

```markdown
### Requirement: GET /orders/{id}
The API SHALL provide `GET /orders/{id}`: fetch an order.

#### Scenario: Found {#S1}
- **WHEN** GET /orders/{id} is called with an order that exists
- **THEN** the API responds with 200: The order

#### Scenario: Missing {#S2}
- **WHEN** the order does not exist
- **THEN** the API responds with 404: No such order
```

The title is a method and a path. Each scenario that says `the API responds with <code>` is a response, and the text after the colon is its description. The ID's domain (`REQ-ORDERS-0001` gives `orders`) is the tag. A block with any other title is an ordinary requirement and is left out. With an AI agent, ask it to design the API: the skill `oxispec-write-api` asks about each operation and every status before it writes.

## Make the skeleton

```bash
oxispec docs openapi --title "Shop" --version 1.0.0 --out openapi.yaml
```

The skeleton lists the paths, methods, tags, summaries and responses. A path parameter such as `{id}` is added as a required string. The requirements do not state schemas, parameter types or security, so the skeleton has none. Add those in the real description.

## Check a description kept in the code

```bash
oxispec docs openapi --against openapi.yaml
```

It prints each call that only the requirements or only the file has, and each call whose status codes differ, and exits 1 on any difference. Run it in CI next to `oxispec check`. It does not say which side is right. Decide, then change the requirement or the file.

### A running service

Most frameworks serve their description at an address: `/v3/api-docs` (Spring Boot with springdoc), `/openapi.json` (FastAPI), `/openapi/v1.json` (ASP.NET Core from .NET 9), `/api-json` (NestJS with `@nestjs/swagger`). Give that address instead of a file:

```bash
oxispec docs openapi --against http://localhost:8080/v3/api-docs
```

Only plain `http://` works, because no TLS library is linked. For an `https://` service, or one that needs a login, save the description with `curl` in the CI job and compare the file. A framework that writes the description at build time (springdoc's Maven plugin, `drf-spectacular`, `swag init`) needs no running service: compare the file it wrote.

## One operation, with what surrounds it

```bash
oxispec api show "GET /orders/{id}"      # or the requirement ID, REQ-ORDERS-0001
oxispec api show REQ-ORDERS-0001 --json
```

The card gives the summary, the requirement and its status, the PRD it refines, the decisions it was made by (`decided-by`), the tech debt that arises from it, the risks that arise from it or that it mitigates (with their level), the test plans that cover it, its responses, and how each scenario stands: `verified`, `no test` or `failing` (from the imported results). With the `tests` module off, the scenario states are unknown and say so.

To fill the card, link the requirement: `oxispec link REQ-ORDERS-0001 refines PRD-0001`, `... decided-by ADR-0004`; `oxispec new debt "..." --from REQ-ORDERS-0001`; `oxispec new risk "..." --from REQ-ORDERS-0001`; `oxispec link TP-0001 covers REQ-ORDERS-0001`.

## Breaking changes

Removing an operation, or one of its responses, is a breaking change in a release: `oxispec release prepare` suggests a major version (a minor one before 1.0) and lists it under Breaking in the notes. Adding an operation is a minor change. This works from the requirements, so it holds for an API that is described in them. Changes to schemas are not seen.

An existing description can be brought in once with `oxispec import specs --from openapi --dir openapi.yaml`, which writes the requirements in this form. See [Moving from another tool](importing.md).

## Search the operations

```bash
oxispec api index --openapi openapi.yaml      # build or update the index
oxispec api search "what happens when the card is declined" --top 3
oxispec api search "fetch an order" --json     # for a script or an assistant
```

The index has one chunk of text for each operation: its call and summary, its scenarios, the PRD, decisions, tech debt, risks and test plans around it (the same things as `oxispec api show`), and, with `--openapi`, what the file says about it (parameters, response descriptions, the operation ID). The file or an `http://` address is optional. The index is kept in `.oxispec/cache/api-index.json`, which is git-ignored and made again by `api index`; only the chunks whose text changed are embedded again. Run `api index` in CI or at deploy, after `oxispec check`, to feed an assistant that answers questions about the API.

How a text becomes a vector is the embedder. The **built-in** one needs no model and no network: it counts words (cut to their stems) and word pairs into 256 buckets. It finds what shares words with the question, so `card declined` finds the operation that mentions a declined card, but it does not know that `refund` and `give the money back` mean the same. For that, point the index at a service that embeds text, in `.oxispec/config.yaml`:

```yaml
api_index:
  embedder:
    url: http://localhost:11434/v1/embeddings   # Ollama, LM Studio, vLLM, llama.cpp
    model: nomic-embed-text
    api_key_env: EMBEDDINGS_KEY                  # optional: the name of a variable, never the key
```

The service must have the OpenAI-compatible embeddings API. Only plain `http://` works, which suits a model on your machine or in your network, and keeps the text of private specs there. A different embedder, or a different model, makes the next `api index` embed everything again, and `api search` refuses an index made by another embedder.

### A model on your machine

`oxispec` does not run a model and does not ship one, so the binary stays small. It asks a service that does. The model stays loaded in that service between your commands, so a search takes a moment and nothing is loaded each time. Two common ways to run one:

**Ollama.** Install it, pull an embedding model, and let it run in the background:

```bash
ollama pull nomic-embed-text
```

and set `url: http://localhost:11434/v1/embeddings` and `model: nomic-embed-text` as above. By default Ollama unloads a model after a few minutes without use, and the next request loads it again. To keep it loaded, set the keep-alive time of the service (the variable `OLLAMA_KEEP_ALIVE`, where `-1` means never unload) before it starts.

**llama.cpp, from a model file.** `llama-server` loads the file you name and serves it:

```bash
llama-server -m /path/to/model.gguf --embedding --port 8080
```

and set `url: http://localhost:8080/v1/embeddings`. The model stays loaded while the process runs. Use a model made for embeddings, not a chat model.

Flags and names change between versions of these programs, so check them against the version you have. What `oxispec` needs from the service is only the OpenAI-compatible `POST /v1/embeddings`, over plain `http://`. A quick check that the service answers, before you index:

```bash
curl -s http://localhost:11434/v1/embeddings -H 'Content-Type: application/json' \
  -d '{"model": "nomic-embed-text", "input": ["hello"]}'
```

The reply has `data[0].embedding`, a list of numbers. If it does, `oxispec api index` will work. If the service is not running when you index or search, `oxispec` stops with an error that names the address, and nothing in the index changes.

`oxispec` does not start or stop the service for you. The vectors are stored in `.oxispec/cache/api-index.json`, so the service is needed only to index and to embed the question when you search.

## Security

`oxispec check` warns (W023) about an operation that has no scenario responding with `401` or `403`, but only in a project where some other operation has one. A project that has not started to model who may call gets no warnings. Once it has, an operation that never says what happens to a caller who may not use it shows up, which is where broken access control tends to hide. An operation whose statement says it is `public` is left alone: `The API SHALL provide \`GET /health\`: a public check.`

This is a prompt to state the behavior, not a security test. It does not know whether the check exists in the code. Pair it with a scenario and a test, and record what you find in the [risk register](../risks.md). The skill `oxispec-review-security` walks an API through the OWASP API Security Top 10 and records risks for a person to rate.
