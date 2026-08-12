---
name: review-pr
description: Review a Deno runtime pull request for correctness, tests, security, and conventions. Use when asked to review a PR or when a PR number/URL is provided for review.
argument-hint: <pr-number-or-url>
allowed-tools: Bash(gh *) Bash(git *) Read Glob Grep Agent
---

# Deno PR Reviewer

Review PR `$ARGUMENTS` on the `denoland/deno` repository.

## Step 1: Gather PR context

Fetch the PR metadata, diff, and comments:

```!
gh pr view $ARGUMENTS --json number,title,body,author,labels,state,reviewDecision,commits,files,isDraft,createdAt,url
```

```!
gh pr diff $ARGUMENTS
```

```!
gh pr view $ARGUMENTS --comments --json comments
```

```!
gh pr checks $ARGUMENTS --json name,state,conclusion 2>/dev/null || echo "No checks found"
```

## Step 2: Gate checks

Before reviewing code, check these gates. If any fail, flag them prominently at
the top of your review and do not approve.

1. **CI status** — All checks must pass. Point the author to specific failing
   checks. Known flaky tests (labeled `ci-test-flaky`) can be re-run.
2. **PR title format** — Must follow `type(scope): description`. Types: `feat`,
   `fix`, `perf`, `refactor`, `chore`, `docs`, `test`, `revert`, `BREAKING`.
   Scope examples: `ext/node`, `ext/fetch`, `cli`, `lsp`, `runtime`.
3. **No force pushes** — PRs are squash-merged. Authors should push new commits,
   not rewrite history.
4. **Focused scope** — No drive-by cleanups or unrelated changes. Those belong
   in separate PRs.
5. **AI disclosure** — If the PR looks AI-generated (boilerplate-heavy, generic
   comments, suspiciously broad) but has no disclosure, ask about it.
6. **Linked issue (external contributors)** — If the PR author is not a
   `denoland` org member, the PR must link to an issue. If there is no linked
   issue, request changes and ask the author to open an issue and discuss the
   change first.

## Step 3: Code review

Read every changed file in the diff. Use the repo tools (`Read`, `Grep`, `Glob`)
to understand surrounding context when needed.

### Rust code

- **Correctness**: Edge cases handled? No `.unwrap()` on user-controlled data?
- **Error handling**: Proper error types, meaningful messages, no swallowed
  errors.
- **Performance**: No unnecessary allocations/copies, no blocking in async code.
- **Safety**: No `unsafe` without strong justification. No command injection,
  path traversal, or permission bypasses.
- **Permissions**: New capabilities must go through Deno's permission system.
  Watch `ext/node/` especially — Node.js APIs sometimes assume full access.
- **Dependencies**: New Cargo deps need strong justification. Prefer existing
  deps or stdlib.

### JavaScript/TypeScript code

- **Node.js compatibility** (`ext/node/`): Does the implementation match Node.js
  behavior? Check against Node.js docs and/or source code.
- **Primordials**: Internal JS should use primordials
  (`globalThis.__bootstrap.primordials`) to avoid prototype pollution. Built-in
  methods must not be called on user-controlled objects without primordial
  wrappers.
- **Web standards**: Web API implementations should follow the relevant spec.
  WPT coverage is preferred.
- **Lazy loading**: All code should use lazy-loaded imports where possible to
  reduce startup cost.

### Tests

- Every bug fix needs a test that would have caught the bug. Every feature needs
  happy-path + edge-case tests.
- Prefer unit tests over spec tests over integration tests. Only use spec tests
  when the behavior requires CLI-level validation.
- Spec tests live in `tests/specs/` using `__test__.jsonc`. Use `[WILDCARD]` for
  non-deterministic output, `[UNORDERED_START]`/`[UNORDERED_END]` for
  non-deterministic ordering.
- Tests must be deterministic — no race conditions, timing deps, or port
  conflicts.

### Security-sensitive areas

Pay extra attention to changes in:

- `runtime/permissions.rs` and permission checks throughout
- `ext/net/`, `ext/fs/` — network and filesystem access
- `ext/node/` — needs its own permission checks
- `cli/tools/compile.rs` — standalone binary compilation
- Any code that shells out or processes user-controlled paths/URLs

## Step 4: PR-type-specific checks

Apply additional checks based on the PR type:

- **Node.js compat** (`ext/node/`): Verify behavior against Node.js docs and/or
  source code, not just what "seems right". New polyfills must be registered in
  `ext/node/polyfills/01_require.js`.
- **Performance**: Must include before/after benchmarks or a clear argument for
  the improvement. Watch for correctness regressions.
- **Dependency updates**: Check changelog for breaking changes. Prioritize
  security updates.
- **WPT changes**: Verify passes are real, not just skipped assertions.
  Expectation file updates must match actual results. Suggest `ci-wpt-test`
  label if not present.
- **CI/release tooling**: Flag for `@bartlomieju` review — do not approve these
  yourself.

## Step 5: Write your review

Post a review using `gh pr review`. Structure:

1. **Summary** (1-2 sentences): What the PR does and your overall assessment.
2. **Gate issues** (if any): Blocking problems that must be fixed.
3. **Code comments**: Specific, actionable feedback referencing exact files and
   lines. Use `nit:` prefix for non-blocking suggestions. Suggest fixes when
   possible, not just "this is wrong."
4. **Verdict**: Approve, request changes, or comment.

### Tone

- Direct: "This needs a test" not "It would be wonderful if we could add a test
  here."
- Kind: Thank contributors, especially first-timers. Assume good intent.
- Helpful: If rejecting, show what a good version looks like.
- Brief: If the contributor clearly knows what they're doing, keep it tight.

### Posting the review

Prefer inline comments on specific lines where possible. Use a single review
with both a summary body and inline comments:

```
gh api repos/denoland/deno/pulls/{number}/reviews -f event=COMMENT -f body="summary" -f comments='[{"path":"file.rs","line":42,"body":"comment"}]'
```

Use `event=APPROVE` or `event=REQUEST_CHANGES` as appropriate instead of
`COMMENT`.

For simple reviews without inline comments, fall back to:

```
gh pr review $ARGUMENTS --comment --body "review text"
```

### Merge readiness

You do NOT have merge permissions. When a PR is ready:

- For first-time contributors: comment
  `@bartlomieju LGTM, needs maintainer signoff (first-time contributor)`
- For regular contributors: comment `@bartlomieju this is ready to merge`

## Rules

- Never approve a PR with failing CI.
- Never approve PRs that bypass the permission system.
- Never approve large architectural changes without flagging for maintainer
  discussion.
- Do not bikeshed style if it passes the linter.
- Do not request changes for things automated checks already enforce.
- Always confirm with the user before posting any review comments to GitHub.
