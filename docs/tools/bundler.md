## Bundling

`deno bundle [URL]` will output a single JavaScript file, which includes all
dependencies of the specified input. For example:

```
> deno bundle https://deno.land/std@$STD_VERSION/examples/colors.ts colors.bundle.js
Bundling "colors.bundle.js"
Emitting bundle to "colors.bundle.js"
9.2 kB emitted.
```

If you omit the out file, the bundle will be sent to `stdout`.

The bundle can just be run as any other module in Deno would:

```
deno run colors.bundle.js
```

The output is a self contained ES Module, where any exports from the main module
supplied on the command line will be available. For example, if the main module
looked something like this:

```ts
export { foo } from "./foo.js";

export const bar = "bar";
```

It could be imported like this:

```ts
import { foo, bar } from "./lib.bundle.js";
```

Bundles can also be loaded in the web browser. The bundle is a self-contained ES
module, and so the attribute of `type` must be set to `"module"`. For example:

```html
<script type="module" src="website.bundle.js"></script>
```

Or you could import it into another ES module to consume:

```html
<script type="module">
  import * as website from "website.bundle.js";
</script>
```
