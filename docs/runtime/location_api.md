## Location API

Deno supports the
[`location`](https://developer.mozilla.org/en-US/docs/Web/API/Window/location)
global from the web. Please read on.

### Location flag

There is no "web page" whose URL we can use for a location in a Deno process. We
instead allow users to emulate a document location by specifying one on the CLI
using the `--location` flag. It can be a `http` or `https` URL.

```ts
// deno run --location https://example.com/path main.ts

console.log(location.href);
// "https://example.com/path"
```

You must pass `--location <href>` for this to work. If you don't, any access to
the `location` global will throw an error.

```ts
// deno run main.ts

console.log(location.href);
// error: Uncaught ReferenceError: Access to "location", run again with --location <href>.
```

Setting `location` or any of its fields will normally cause navigation in
browsers. This is not applicable in Deno, so it will throw in this situation.

```ts
// deno run --location https://example.com/path main.ts

location.pathname = "./foo";
// error: Uncaught NotSupportedError: Cannot set "location.pathname".
```

### Extended usage

On the web, resource resolution (excluding modules) typically uses the value of
`location.href` as the root on which to base any relative URLs. This affects
some web APIs adopted by Deno.

#### Fetch API

```ts
// deno run --location https://api.github.com/ --allow-net main.ts

const response = await fetch("./orgs/denoland");
// Fetches "https://api.github.com/orgs/denoland".
```

The `fetch()` call above would throw if the `--location` flag was not passed,
since there is no web-analogous location to base it onto.

#### Worker modules

```ts
// deno run --location https://example.com/index.html --allow-net main.ts

const worker = new Worker("./workers/hello.ts", { type: "module" });
// Fetches worker module at "https://example.com/workers/hello.ts".
```

### Only use if necessary

For the above use cases, it is preferable to pass URLs in full rather than
relying on `--location`. You can manually base a relative URL using the `URL`
constructor if needed.

The `--location` flag is intended for those who have some specific purpose in
mind for emulating a document location and are aware that this will only work at
application-level. However, you may also use it to silence errors from a
dependency which is frivolously accessing the `location` global.
