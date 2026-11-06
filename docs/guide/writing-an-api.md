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

An existing description can be brought in once with `oxispec import specs --from openapi --dir openapi.yaml`, which writes the requirements in this form. See [Moving from another tool](importing.md).
