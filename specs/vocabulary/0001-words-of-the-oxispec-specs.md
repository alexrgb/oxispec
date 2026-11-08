---
id: VOC-0001
type: vocabulary
status: accepted
---
# VOC-0001: Words of the OxiSpec specs

The variables that claims in the specs of OxiSpec itself talk about. A claim is a
line in a `claims` block of a requirement, such as `exit.refusal == 2`. The specs
say these values in words, in many places. The claims let `oxispec check` notice
when two requirements stop agreeing about one of them.

```vocabulary
# What a command exits with.
exit.refusal: {type: int}
exit.findings: {type: int}
# The size of a page of MCP resources.
mcp.resources_page_size: {type: int}
# Whether `--strict` is given, and whether a warning then fails the run.
strict: {type: bool}
warnings.fail: {type: bool}
# When `oxispec serve` starts.
listen.local: {type: bool}
behind_tls_proxy: {type: bool}
token.count: {type: int}
serve.starts: {type: bool}
```
