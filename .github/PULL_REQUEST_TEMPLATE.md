<!--
Before s## Pull Request Title
- Ensure your PR title is clear and descriptive.
  - Example: `feat: add basic server functionality`

## Description
- Provide context for your changes:
  - Why are you adding or modifying these files?
  - How does this improve the project?

## Referencing Issues
- If this PR addresses an issue, reference it so it closes on merge:
  - Example: `fix #<issue-number>`

## Code Quality
- [ ] Run required formatting tools: `./tools/format.js`
- [ ] Run required linting tools: `./tools/lint.js`
- [ ] Ensure your code is clean and adheres to project standards

## Tests
- [ ] If applicable, verify your changes are covered by tests
- [ ] Run all tests: `cargo test` and confirm they pass

## PR Status
- [ ] If this PR is not ready for review, mark it as a **Draft**

## Maintainer Edits
- [ ] Allow edits by maintainers to help refine your PR if needed

---

Thank you for your contribution!ubmitting a PR, please read https://docs.deno.com/runtime/manual/references/contributing

1. Give the PR a descriptive title.

  Examples of good title:
    - fix(std/http): Fix race condition in server
    - docs(console): Update docstrings
    - feat(doc): Handle nested reexports

  Examples of bad title:
    - fix #7123
    - update docs
    - fix bugs

2. Ensure there is a related issue and it is referenced in the PR text.
3. Ensure there are tests that cover the changes.
4. Ensure `cargo test` passes.
5. Ensure `./tools/format.js` passes without changing files.
6. Ensure `./tools/lint.js` passes.
7. Open as a draft PR if your work is still in progress. The CI won't run
   all steps, but you can add '[ci]' to a commit message to force it to.
8. If you would like to run the benchmarks on the CI, add the 'ci-bench' label.
-->
