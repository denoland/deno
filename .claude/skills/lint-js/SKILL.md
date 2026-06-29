---
name: lint-js
description: Lint JS/TS code only. Use before opening a PR when only JavaScript or TypeScript files were changed (no Rust).
user-invocable: true
allowed-tools: Bash(./x lint-js)
---

# Lint JS/TS Code

Run the JS/TS linter:

```sh
./x lint-js
```

If there are lint errors, fix them and re-run until clean.
