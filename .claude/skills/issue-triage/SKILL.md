---
name: issue-triage
description: Triage a Deno GitHub issue — reproduce bugs, classify, label, and comment with findings. Use when asked to triage an issue or when an issue number/URL is provided for triage.
argument-hint: <issue-number-or-url>
allowed-tools: Bash(gh *) Bash(deno *) Bash(git *) Bash(mktemp *) Bash(rm *) Bash(cat *) Read Write Glob Grep Agent
---

# Deno Issue Triage

Triage issue `$ARGUMENTS` on the `denoland/deno` repository.

## Step 1: Read the issue

```!
gh issue view $ARGUMENTS --repo denoland/deno --json number,title,body,author,labels,state,comments,createdAt,url
```

## Step 2: Classify the issue

Determine what kind of issue this is:

- **Bug report** — something isn't working as expected
- **Feature request / suggestion** — a new capability or enhancement
- **Question** — a usage question, not a bug
- **Duplicate** — already reported (search closed and open issues with
  `gh issue list --search "keywords" --state all`)
- **Invalid** — not actionable, not a Deno issue, or insufficient info

If the issue is clearly a question, duplicate, or invalid, skip reproduction and
go straight to Step 5.

## Step 3: Check for missing information

A valid bug report needs:

- Deno version (`deno --version` output)
- Operating system
- Reproduction steps or minimal reproduction code
- Expected behavior vs actual behavior

If any of these are missing, label as `needs info` and comment asking the author
to provide the missing details. Do not attempt reproduction without a clear
repro case.

## Step 4: Reproduce the bug

Only attempt reproduction for bug reports that have a clear repro case.

### Setup

Prefer running reproductions inside a Docker container for isolation. Fall back
to a local temp directory only if Docker is unavailable.

**Docker (preferred):**

```sh
# Run repro with latest canary
docker run --rm -v "$REPRO_DIR":/repro -w /repro denoland/deno:canary deno run repro.ts

# Run repro with a specific version (e.g., 2.1.4)
docker run --rm -v "$REPRO_DIR":/repro -w /repro denoland/deno:2.1.4 deno run repro.ts
```

Use `docker run --rm` so containers are cleaned up automatically. Mount the
repro files via `-v`.

**Local fallback (if Docker is not available):**

```sh
REPRO_DIR=$(mktemp -d)
deno run "$REPRO_DIR/repro.ts"
```

### Get Deno versions

Try to reproduce with both:

1. **The version from the issue** (if specified) — to confirm the bug exists
2. **Latest canary** — to check if it's already fixed

Use the appropriate Docker image tag for each version (e.g.,
`denoland/deno:2.1.4`, `denoland/deno:canary`).

If the issue specifies a particular Deno version and the bug does NOT reproduce
on canary, note that it may already be fixed. Check git log for relevant fixes.

### Run the reproduction

- Extract the reproduction code from the issue body
- Write it to a local temp directory
- Run it inside a Docker container (or locally as fallback) with the appropriate
  `deno` subcommand and flags
- Capture both stdout and stderr
- Compare actual output against the expected behavior described in the issue

If the reproduction involves specific npm packages, `deno.json` config, or
multi-file setups, recreate the full environment as described.

### Clean up

```sh
rm -rf "$REPRO_DIR"
```

### Record findings

Note:

- Does the bug reproduce on the reported version?
- Does the bug reproduce on canary?
- Any additional observations (e.g., different error message, partial fix,
  related issues)

## Step 5: Label the issue

Add labels based on your classification. Keep it minimal — usually a single area
label is sufficient. Do not over-label.

### Type labels (add one if applicable)

| Label        | When to use                                         |
| ------------ | --------------------------------------------------- |
| `bug`        | Confirmed bug — always add for verified bug reports |
| `feat`       | Accepted new feature                                |
| `suggestion` | Feature request not yet accepted                    |
| `question`   | Usage question                                      |
| `duplicate`  | Duplicate of another issue                          |
| `invalid`    | Not actionable                                      |
| `panic`      | Deno panics/crashes                                 |
| `regression` | Something that used to work but is now broken       |

### Area labels (add one that best matches)

| Label                                                                                                                                  | Area                             |
| -------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- |
| `node compat`                                                                                                                          | General Node.js compatibility    |
| `node API`                                                                                                                             | Specific `node:*` module APIs    |
| `ext/node`, `ext/fs`, `ext/net`, `ext/http`, `ext/fetch`, `ext/web`, `ext/crypto`, `ext/console`, `ext/url`, `ext/websocket`, `ext/kv` | Specific extension               |
| `cli`                                                                                                                                  | CLI behavior, flags, subcommands |
| `lsp`                                                                                                                                  | Language server                  |
| `runtime`                                                                                                                              | Runtime crate                    |
| `permissions`                                                                                                                          | Permission system                |
| `compile`                                                                                                                              | `deno compile`                   |
| `testing`                                                                                                                              | `deno test` and coverage         |
| `task runner`                                                                                                                          | `deno task`                      |
| `install`                                                                                                                              | `deno install` / `deno add`      |
| `tsc`                                                                                                                                  | TypeScript compiler              |
| `types`                                                                                                                                | TypeScript type issues           |
| `config`                                                                                                                               | `deno.json` configuration        |
| `node resolution`                                                                                                                      | Node/npm module resolution       |
| `publish`                                                                                                                              | `deno publish`                   |
| `lint`                                                                                                                                 | `deno lint`                      |
| `wasm`                                                                                                                                 | WebAssembly                      |

### Priority labels (add only when clearly warranted)

| Label           | When to use                                 |
| --------------- | ------------------------------------------- |
| `high priority` | Severe impact, blocks users, security issue |
| `quick fix`     | Obviously simple fix                        |

### Adding labels

```sh
gh issue edit $ARGUMENTS --repo denoland/deno --add-label "bug"
```

Remove the `needs triage` or `triage required 👀` label if present:

```sh
gh issue edit $ARGUMENTS --repo denoland/deno --remove-label "needs triage" --remove-label "triage required 👀"
```

## Step 6: Comment on the issue

Post a triage comment with your findings. Structure:

For **confirmed bugs**:

```
Confirmed on [version]. [Brief description of what you observed.]

[If tested on canary: "Also reproduces on canary." or "Does not reproduce on canary — may already be fixed."]
```

For **needs info**:

```
Thanks for reporting. Could you provide [missing info]? This will help us investigate.
```

For **duplicates**:

```
This looks like a duplicate of #XXXX. Closing in favor of that issue.
```

For **questions**:

```
This is a usage question rather than a bug. [Brief answer or pointer to docs.] Closing this — feel free to ask on https://discord.gg/deno if you have more questions.
```

### Posting the comment

```sh
gh issue comment $ARGUMENTS --repo denoland/deno --body "comment text"
```

Close the issue if it's a duplicate, question, or invalid:

```sh
gh issue close $ARGUMENTS --repo denoland/deno --reason "not planned"
```

## Rules

- Always confirm with the user before posting comments or modifying labels on
  GitHub.
- Do not close bug reports — only close duplicates, questions, and invalid
  issues.
- Keep labels minimal. One type label + one area label is usually enough.
- Be kind to reporters. Thank them, especially first-time reporters.
- If you cannot reproduce a bug, say so honestly — do not guess at causes.
- If the reproduction requires permissions or resources you don't have access
  to, note that and skip reproduction.
- Never dismiss an issue without investigation.
