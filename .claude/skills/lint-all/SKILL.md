---
name: lint-all
description: Lint all code (Rust + JS/TS). Use before opening a PR when Rust code was changed.
user-invocable: true
allowed-tools: Bash(./x lint)
---

# Lint All Code

Run the full linter (Rust + JS/TS):

```sh
./x lint
```

If there are lint errors, fix them and re-run until clean.
