# Snapshot Example

This example roughly follows the blog post
[Roll Your Own JavaScript Runtime: Part 3][blog] to create a `JsRuntime` with an
embedded startup snapshot.

That blog post and the two that preceded it were no longer accurate. By
including this example in the repository, it will continually be built, so it
will hopefully stay up-to-date.

## Running

The example can be run by changing to the `core/examples/snapshot` directory and
running `cargo run`.

## Differences

Differences from those blog posts:

- The `create_snapshot()` API has changed in various ways.
- New API features for extensions:
  - `#[op2]` ([read more][op2])
  - `extension!(...)` macro replaces `Extension::builder()`
  - ESM-based extensions.

Missing features vs. those blog posts:

- Does not implement [TsModuleLoader], to keep this example more concise.

[blog]: https://deno.com/blog/roll-your-own-javascript-runtime-pt3#creating-a-snapshot-in-buildrs
[op2]: https://github.com/denoland/deno_core/tree/main/ops/op2#readme
[TsModuleLoader]: https://deno.com/blog/roll-your-own-javascript-runtime-pt2#supporting-typescript
