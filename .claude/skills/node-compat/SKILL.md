---
name: node-compat
description: Run a Node.js compatibility test, diagnose failures, and either fix the implementation, skip, or ignore the test. Use when asked to work on node compat tests.
argument-hint: <test-name>
allowed-tools: Bash Read Write Edit Glob Grep Agent
---

# Node Compat Test

Work on Node.js compatibility test `$ARGUMENTS`.

## Step 1: Build and run the test

```sh
./x build
./x test-compat $ARGUMENTS
```

If the test passes, report success and ensure test is specified in
`tests/node_compat/config.jsonc`.

## Step 2: Diagnose the failure

Read the test file to understand what it tests:

```sh
# Tests live under tests/node_compat/test/
```

Use `Grep` and `Read` to find the test source, then analyze:

1. **What Node.js API or behavior is being tested?**
2. **What is the actual error or assertion failure?**
3. **Where is the relevant Deno implementation?** Check `ext/node/` (polyfills,
   ops, internal bindings), `runtime/`, or `cli/`.

Read the corresponding Node.js docs and/or source code to understand the
expected behavior.

## Step 3: Classify the failure

Determine which category this failure falls into:

### A. Fixable bug

The Deno implementation is wrong or incomplete, but can be corrected. This
includes:

- Missing method/property on a polyfill
- Wrong return value or error type
- Missing event emission
- Incorrect argument handling
- Hard-to-fix but still fundamentally implementable behaviors

**Action:** Fix the implementation (Step 4).

### B. Inherent incompatibility

The test relies on Node.js internals or architecture that Deno fundamentally
cannot or will not support:

- `internalBinding()` calls to Node's C++ layer
- Node.js-specific CLI flags (`--inspect`, `--prof`, etc.)
- V8 internals exposed through Node-specific APIs
- Node.js-specific build/addon tooling (node-gyp internals)
- Tests for Node.js's own test infrastructure

**Action:** Ignore the test with a reason (Step 5).

### C. Not worth fixing

The test exercises an edge case or behavior that is technically possible but
provides negligible value:

- Extremely obscure error message wording differences
- Node.js-specific deprecation warnings
- Behavior that no real-world code depends on

**Action:** Ignore or skip the test with a reason (Step 5).

## Step 4: Fix the implementation

If the failure is fixable:

1. Locate the relevant code in `ext/node/` (or elsewhere).
2. Implement the fix. Match Node.js behavior - check Node.js docs and/or source
   code, not just what "seems right."
3. Use lazy-loaded imports where possible.
4. Use primordials for internal JS code to avoid prototype pollution.
5. Rebuild the source code with `./x build` - this is paramount, changes won't
   take effect until you do.
6. Re-run the test to verify the fix:

```sh
./x test-compat $ARGUMENTS
```

6. Once the test passes, make sure the test is listed in
   `tests/node_compat/config.jsonc` with an empty config:

```jsonc
"category/test-name.js": {}
```

The entries in `config.jsonc` are sorted alphabetically within their category.
Place the new entry in the correct position.

## Step 5: Skip or ignore the test

If the test cannot or should not be fixed, update
`tests/node_compat/config.jsonc`.

### Ignore (test should never run — inherent incompatibility)

```jsonc
"category/test-name.js": {
  "ignore": true,
  "reason": "Brief, specific explanation of why this can't work in Deno"
}
```

`"reason"` must be specified otherwise the lint step will fail!

### Platform-specific skip

If the test only fails on certain platforms:

```jsonc
"category/test-name.js": {
  "windows": false
}
```

### Expected failure (test runs but fails with known output)

If you want the test to run but expect a specific failure:

```jsonc
"category/test-name.js": {
  "exitCode": 1,
  "output": "[WILDCARD]specific error message[WILDCARD]",
  "reason": "Brief explanation of why this fails"
}
```

This is a good middle ground for tests that are generally compatible but have a
specific known issue. If a fix is ever done this assertion will notify the
implementer to update the config.

### Writing good reasons

Reasons should be specific and actionable. Good examples:

- "Tests Node.js internal C++ binding (internalBinding('zlib').Zlib) which is
  not implemented in Deno"
- "requires `deno --interactive` flag (not yet implemented)"
- "URL.createObjectURL does not throw ERR_INVALID_ARG_TYPE for non-Blob
  arguments"

Bad examples:

- "Not supported" (too vague)
- "Doesn't work" (says nothing)
- "Node-specific" (which part?)

## Step 6: Verify

Re-run the test one final time to confirm the outcome matches expectations:

```sh
./x test-compat $ARGUMENTS
```

## Config reference

The full schema for `config.jsonc` entries is in
`tests/node_compat/schema.json`.

## PR title conventions

When opening a PR for node-compat work, use the appropriate prefix:

- `test:` — when the PR only updates `tests/node_compat/config.jsonc` to skip,
  ignore, or otherwise reclassify tests without changing implementation code.
- `fix(ext/node):` — when the PR actually fixes the implementation so a
  previously failing test now passes (typically changes under `ext/node/` plus
  enabling the test in `config.jsonc`).
