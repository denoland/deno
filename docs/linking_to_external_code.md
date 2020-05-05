# Linking to third party code

In the [Getting Started](../getting_started) section, we saw that Deno could
execute scripts from URLs. Like browser JavaScript, Deno can import libraries
directly from URLs. This example uses a URL to import an assertion library:

```ts
import { assertEquals } from "https://deno.land/std/testing/asserts.ts";

assertEquals("hello", "hello");
assertEquals("world", "world");

console.log("Asserted! 🎉");
```

Try running this:

```shell
$ deno run test.ts
Compile file:///mnt/f9/Projects/github.com/denoland/deno/docs/test.ts
Download https://deno.land/std/testing/asserts.ts
Download https://deno.land/std/fmt/colors.ts
Download https://deno.land/std/testing/diff.ts
Asserted! 🎉
```

Note that we did not have to provide the `--allow-net` flag for this program,
and yet it accessed the network. The runtime has special access to download
imports and cache them to disk.

Deno caches remote imports in a special directory specified by the `$DENO_DIR`
environmental variable. It defaults to the system's cache directory if
`$DENO_DIR` is not specified. The next time you run the program, no downloads
will be made. If the program hasn't changed, it won't be recompiled either. The
default directory is:

- On Linux/Redox: `$XDG_CACHE_HOME/deno` or `$HOME/.cache/deno`
- On Windows: `%LOCALAPPDATA%/deno` (`%LOCALAPPDATA%` = `FOLDERID_LocalAppData`)
- On macOS: `$HOME/Library/Caches/deno`
- If something fails, it falls back to `$HOME/.deno`

## FAQ

### But what if `https://deno.land/` goes down?

Relying on external servers is convenient for development but brittle in
production. Production software should always bundle its dependencies. In Deno
this is done by checking the `$DENO_DIR` into your source control system, and
specifying that path as the `$DENO_DIR` environmental variable at runtime.

### How can I trust a URL that may change?

By using a lock file (using the `--lock` command line flag) you can ensure
you're running the code you expect to be. You can learn more about this
[here](./integrity_checking).

### How do you import to a specific version?

Simply specify the version in the URL. For example, this URL fully specifies the
code being run: `https://unpkg.com/liltest@0.0.5/dist/liltest.js`. Combined with
the aforementioned technique of setting `$DENO_DIR` in production to stored
code, one can fully specify the exact code being run, and execute the code
without network access.

### It seems unwieldy to import URLs everywhere.

> What if one of the URLs links to a subtly different version of a library?

> Isn't it error prone to maintain URLs everywhere in a large project?

The solution is to import and re-export your external libraries in a central
`deps.ts` file (which serves the same purpose as Node's `package.json` file).
For example, let's say you were using the above assertion library across a large
project. Rather than importing `"https://deno.land/std/testing/asserts.ts"`
everywhere, you could create a `deps.ts` file that exports the third-party code:

```ts
export {
  assert,
  assertEquals,
  assertStrContains,
} from "https://deno.land/std/testing/asserts.ts";
```

And throughout the same project, you can import from the `deps.ts` and avoid
having many references to the same URL:

```ts
import { assertEquals, runTests, test } from "./deps.ts";
```

This design circumvents a plethora of complexity spawned by package management
software, centralized code repositories, and superfluous file formats.
