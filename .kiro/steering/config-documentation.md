# Configuration Documentation — Steering Rule

When any spec adds, removes, or modifies a configuration field (in `ActorConfig`, `GridConfig`, `WorldInitConfig`, or any other config struct):

1. **`example_config.toml`** — Update the example config file to include the new/changed field with a comment explaining its purpose and valid range.
2. **`README.md`** — Update the README if it documents configuration parameters.
3. **Spec requirements** — Include a documentation update requirement in the spec so it appears in the task list and is not forgotten during implementation.

This ensures configuration documentation stays in sync with the code at all times.
