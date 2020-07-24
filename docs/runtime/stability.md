## Stability

As of Deno 1.0.0, the `Deno` namespace APIs are stable. That means we will
strive to make code working under 1.0.0 continue to work in future versions.

However, not all of Deno's features are ready for production yet. Features which
are not ready, because they are still in draft phase, are locked behind the
`--unstable` command line flag.

```shell
deno run --unstable mod_which_uses_unstable_stuff.ts
```

Passing this flag does a few things:

- It enables the use of unstable APIs during runtime.
- It adds the
  [`lib.deno.unstable.d.ts`](https://doc.deno.land/https/raw.githubusercontent.com/denoland/deno/master/cli/dts/lib.deno.unstable.d.ts)
  file to the list of TypeScript definitions that are used for type checking.
  This includes the output of `deno types`.

You should be aware that many unstable APIs have **not undergone a security
review**, are likely to have **breaking API changes** in the future, and are
**not ready for production**.

### Standard modules

Deno's standard modules (https://deno.land/std/) are not yet stable. We
currently version the standard modules differently from the CLI to reflect this.
Note that unlike the `Deno` namespace, the use of the standard modules do not
require the `--unstable` flag (unless the standard module itself makes use of an
unstable Deno feature).
