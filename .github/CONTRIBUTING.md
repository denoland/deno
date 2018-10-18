# Contributing To Deno

Check [the roadmap](https://github.com/denoland/deno/blob/master/Roadmap.md)
before contributing.

Please don't make [the benchmarks](https://denoland.github.io/deno/) worse.

Ask for help in the issues or on the
[chat room](https://gitter.im/denolife/Lobby).

Progress towards future releases is tracked
[here](https://github.com/denoland/deno/milestones).

Docs are [here](https://github.com/denoland/deno/blob/master/Docs.md).

## Submitting a pull request

Before submitting, please make sure the following is done:

1. Ensure `./tools/test.py` passes.
2. Format your code with `./tools/format.py`.
3. Make sure `./tools/lint.py` passes.

## Changes to `third_party`

Changes to `third_party` including any changes in the `package.json` will impact
the [denoland/deno_third_party](https://github.com/denoland/deno_third_party)
repository as well.

## Adding Ops (aka bindings)

We are very concerned about making mistakes when adding new APIs. When adding an
Op to Deno, the counterpart interfaces on other platforms should be researched.
Please list how this functionality is done in Go, Node, Rust, and Python.

As an example, see how `deno.rename()` was proposed and added in
[PR #671](https://github.com/denoland/deno/pull/671).

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

### JSDoc style guide

- It is important that documentation is easily human readable, but there is also
  a need to provide additional styling information to ensure generated
  documentation is more rich text. Therefore JSDoc should generally follow
  markdown markup to enrich the text.
- While markdown supports HTML tags, it is forbidden in JSDoc blocks.
- Code string literals should be braced with the back-tick (\`) instead of
  quotes. For example:
  ```ts
  /** Import something from the `deno` module. */
  ```
- Do not document function arguments unless they are non-obvious of their intent
  (though if they are non-obvious intent, the API should be considered anyways).
  Therefore `@param` should generally not be used.
- Vertical spacing should be minimized whenever possible. Therefore single line
  comments should be written as:
  ```ts
  /** This is a good single line JSDoc */
  ```
  And not:
  ```ts
  /**
   * This is a bad single line JSDoc
   */
  ```
- Code examples should not utilise the triple-back tick (\`\`\`) notation or
  tags. They should just be marked by indentation, which requires a break before
  the block and 6 additional spaces for each line of the example. This is 4 more
  than the first column of the comment. For example:
  ```ts
  /** A straight forward comment and an example:
   *
   *       import { foo } from "deno";
   *       foo("bar");
   */
  ```
- Code examples should not contain additional comments. It is already inside a
  comment. If it needs further comments is not a good example.
