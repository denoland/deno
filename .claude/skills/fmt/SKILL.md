---
name: fmt
description: Format all code in the repository. Run before opening a PR or committing changes.
user-invocable: true
allowed-tools: Bash(./x fmt)
---

# Format Code

Run the Deno code formatter:

```sh
./x fmt
```

If any files were changed, stage and report them.
