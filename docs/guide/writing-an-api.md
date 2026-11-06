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
