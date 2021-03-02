# Contributing

- Read the [style guide](./contributing/style_guide.md).

- Please don't make [the benchmarks](https://deno.land/benchmarks) worse.

- Ask for help in the [community chat room](https://discord.gg/deno).

- If you are going to work on an issue, mention so in the issue comments
  _before_ you start working on the issue.

- If you are going to work on a new feature, create an issue and discuss with
  other contributors _before_ you start working on the feature.

- Please be professional in the forums. We follow
  [Rust's code of conduct](https://www.rust-lang.org/policies/code-of-conduct)
  (CoC). Have a problem? Email ry@tinyclouds.org.

## Development

Instructions on how to build from source can be found
[here](./contributing/building_from_source.md).

## Submitting a Pull Request

Before submitting, please make sure the following is done:

1. Give the PR a descriptive title.

Examples of good PR title:

- fix(std/http): Fix race condition in server
- docs(console): Update docstrings
- feat(doc): Handle nested re-exports

Examples of bad PR title:

- fix #7123
- update docs
- fix bugs

2. Ensure there is a related issue and it is referenced in the PR text (You
   don't want to duplicate effort).
3. Ensure there are tests that cover the changes.
4. Ensure `cargo test` passes.
5. Ensure `./tools/format.js` passes without changing files.
6. Ensure `./tools/lint.js` passes.

That's it! Thank you for your contribution!

## After your pull request is merged

After your pull request is merged, you can safely delete your branch and pull
the changes from the main (upstream) repository:

- Delete the remote branch on GitHub either through the GitHub web UI or your
  local shell as follows:

  ```shell
  git push origin --delete my-fix-branch
  ```

- Check out the main branch:

  ```shell
  git checkout main -f
  ```

- Delete the local branch:

  ```shell
  git branch -D my-fix-branch
  ```

- Update your master with the latest upstream version:

  ```shell
  git pull --ff upstream main
  ```

## Adding Ops (aka bindings)

We are very concerned about making mistakes when adding new APIs. When adding an
Op to Deno, the counterpart interfaces on other platforms should be researched.
Please list how this functionality is done in Go, Node, Rust, and Python.

As an example, see how `Deno.rename()` was proposed and added in
[PR #671](https://github.com/denoland/deno/pull/671).

## Releases

Summary of the changes from previous releases can be found
[here](https://github.com/denoland/deno/releases).

## Documenting APIs

It is important to document public APIs and we want to do that inline with the
code. This helps ensure that code and documentation are tightly coupled
together.

### Utilize JSDoc

All publicly exposed APIs and types, both via the `deno` module as well as the
global/`window` namespace should have JSDoc documentation. This documentation is
parsed and available to the TypeScript compiler, and therefore easy to provide
further downstream. JSDoc blocks come just prior to the statement they apply to
and are denoted by a leading `/**` before terminating with a `*/`. For example:

```ts
/** A simple JSDoc comment */
export const FOO = "foo";
```

Find more at: https://jsdoc.app/
