## Unstable

Not all of Deno's features are ready for production yet. Features which are not
ready because they are still in draft phase are locked behind the `--unstable`
command line flag. Passing this flag does a few things:

- It enables the use of unstable APIs during runtime.
- It adds the
  [`lib.deno.unstable.d.ts`](https://github.com/denoland/deno/blob/master/cli/js/lib.deno.unstable.d.ts)
  file to the list of TypeScript definitions that are used for typechecking.
  This includes the output of `deno types`.

You should be aware that unstable APIs have probably **not undergone a security
review**, are likely to have **breaking API changes** in the future and are
**not ready for production**.
