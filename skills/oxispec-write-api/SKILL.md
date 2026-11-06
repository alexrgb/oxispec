---
name: oxispec-write-api
description: "Describe an HTTP API as requirements titled as calls, make the skeleton of its OpenAPI description from them, and compare a real openapi file with them. Use when asked to design or document an API, or to see whether the spec and the API file agree."
metadata:
  generated-by: oxispec
  content-hash: "365f27f5c22bf684"
---

# Write an API as requirements

Describe an HTTP API as requirements, so that the skeleton of its OpenAPI description, and a check of the real one, come from them. Ask before you write, and write nothing the person has not agreed to.

## Steps

1. Search first: `oxispec search "<resource>"`. Extend what exists.
2. Ask which resources there are and, for each operation, the method, the path, what it does and every status it can answer, with the reason for each. Ask what is not decided yet. Do not invent paths, codes or behavior.
3. Show the list in the chat: `GET /orders/{id}: fetch an order`, with `200` and `404` and what each means. Let the person change it.
4. Write one requirement block per operation, titled as the call:

   ```
   ### Requirement: GET /orders/{id}
   The API SHALL provide `GET /orders/{id}`: fetch an order.

   #### Scenario: Found {#S1}
   - **WHEN** GET /orders/{id} is called with an order that exists
   - **THEN** the API responds with 200: The order
   ```

   Make the node with `oxispec new requirement <domain> "<title>"` and put the blocks in it. The domain becomes the tag.
5. Run `oxispec check`, then `oxispec docs openapi` to see the skeleton.
6. If the project keeps an `openapi.yaml` in the code, or the service serves its description at an `http://` address (`--against http://localhost:8080/v3/api-docs`), run `oxispec docs openapi --against openapi.yaml`. A call or status code that only one side has is for the person to settle: either the requirement or the file is out of date. Say which one you think it is, and change nothing until they decide.

## Rules

- The requirements state operations and responses. Schemas, parameter types and security are not in them. Never add those to the skeleton by hand; they belong in the real description.
- Do not edit the generated skeleton. Change the requirements and make it again.
- Every operation needs a scenario for each status it can answer, written as `the API responds with <code>: <meaning>`.
