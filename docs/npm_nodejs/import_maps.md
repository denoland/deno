## Using import maps

Deno supports [import maps](../linking_to_external_code/import_maps.md) which
allow you to supply Deno with information about how to resolve modules that
overrides the default behavior. Import maps are a web platform standard that is
increasingly being included natively in browsers. They are specifically useful
with adapting Node.js code to work well with Deno, as you can use import maps to
map "bare" specifiers to a specific module.

When coupled with Deno friendly [CDNs](./cdns.md) import maps can be a powerful
tool in managing code and dependencies without need of a package management
tool.

### Bare and extension-less specifiers

Deno will only load a fully qualified module, including the extension. The
import specifier needs to either be relative or absolute. Specifiers that are
neither relative or absolute are often called "bare" specifiers. For example
`"./lodash/index.js"` is a relative specifier and
`https://cdn.skypack.dev/lodash` is an absolute specifier. Where is `"lodash"`
would be a bare specifier.

Also Deno requires that for local modules, the module to load is fully
resolve-able. When an extension is not present, Deno would have to "guess" what
the author intended to be loaded. For example does `"./lodash"` mean
`./lodash.js`, `./lodash.ts`, `./lodash.tsx`, `./lodash.jsx`,
`./lodash/index.js`, `./lodash/index.ts`, `./lodash/index.jsx`, or
`./lodash/index.tsx`?

When dealing with remote modules, Deno allows the CDN/web server define whatever
semantics around resolution the server wants to define. It just treats a URL,
including its query string, as a "unique" module that can be loaded. It expects
the CDN/web server to provide it with a valid media/content type to instruct
Deno how to handle the file. More information on how media types impact how Deno
handles modules can be found in the
[Determining the type of file](../typescript/overview.md#determining-the-type-of-file)
section of the manual.

Node.js does have defined semantics for resolving specifiers, but they are
complex, assume unfettered access to the local file system to query it. Deno has
chosen not to go down that path.

But, import maps can be used to provide some of the ease of the developer
experience if you wish to use bare specifiers. For example, if we want to do the
following in our code:

```ts
import lodash from "lodash";
```

We can accomplish this using an import map, and we don't even have to install
the `lodash` package locally. We would want to create a JSON file (for example
**import_map.json**) with the following:

```json
{
  "imports": {
    "lodash": "https://cdn.skypack.dev/lodash"
  }
}
```

And we would run our program like:

```
> deno run --import-map ./import_map.json example.ts
```

If you wanted to manage the versions in the import map, you could do this as
well. For example if you were using Skypack CDN, you can used a
[pinned URL](https://docs.skypack.dev/skypack-cdn/api-reference/pinned-urls-optimized)
for the dependency in your import map. For example, to pin to lodash version
4.17.21 (and minified production ready version), you would do this:

```json
{
  "imports": {
    "lodash": "https://cdn.skypack.dev/pin/lodash@v4.17.21-K6GEbP02mWFnLA45zAmi/mode=imports,min/optimized/lodash.js"
  }
}
```

### Overriding imports

The other situation where import maps can be very useful is the situation where
you have tried your best to make something work, but have failed. For example
you are using an npm package which has a dependency on some code that just
doesn't work under Deno, and you want to substitute another module that
"polyfills" the incompatible APIs.

For example, let's say we have a package that is using a version of the built in
`"fs"` module that we have a local module we want to replace it with when it
tries to import it, but we want other code we are loading to use the standard
library replacement module for `"fs"`. We would want to create an import map
that looked something like this:

```ts
{
  "imports": {
    "fs": "https://deno.land/std@$STD_VERSION/node/fs.ts"
  },
  "scopes": {
    "https://deno.land/x/example": {
      "fs": "./patched/fs.ts"
    }
  }
}
```

Import maps can be very powerful, check out the official
[standards README](https://github.com/WICG/import-maps#the-import-map) for more
information.
