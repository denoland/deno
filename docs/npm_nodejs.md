# Using npm/Node.js code

While Deno is pretty powerful itself, there is a large eco-system of code in the
[npm](https://npmjs.com/) registry, and many people will want to re-leverage
tools, code and libraries that are built for [Node.js](https://nodejs.org/). In
this chapter we will explore how to use it.

The good news, is that in many cases, it _just works_.

There are some foundational things to understand about differences between
Node.js and Deno that can help in understanding what challenges might be faced:

- Current Node.js supports both CommonJS and ES Modules, while Deno only
  supports ES Modules. The addition of stabilized ES Modules in Node.js is
  relatively recent and most code written for Node.js is in the CommonJS format.
- Node.js has quite a few built-in modules that can be imported and they are a
  fairly expansive set of APIs. On the other hand, Deno focuses on implementing
  web standards and where functionality goes beyond the browser, we locate APIs
  in a single global `Deno` variable/namespace. Lots of code written for Node.js
  expects/depends upon these built-in APIs to be available.
- Node.js has a non-standards based module resolution algorithm, where you can
  import bare-specifiers (e.g. `react` or `lodash`) and Node.js will look in
  your local and global `node_modules` for a path, introspect the `package.json`
  and try to see if there is a module named the right way. Deno resolves modules
  the same way a browser does. For local files, Deno expects a full module name,
  including the extension. When dealing with remote imports, Deno expects the
  web server to do any "resolving" and provide back the media type of the code
  (see the
  [Determining the type of file](../typescript/overview.md#determining-the-type-of-file)
  for more information).
- Node.js effectively doesn't work without a `package.json` file. Deno doesn't
  require an external meta-data file to function or resolve modules.
- Node.js doesn't support remote HTTP imports. It expects all 3rd party code to
  be installed locally on your machine using a package manager like `npm` into
  the local or global `node_modules` folder. Deno supports remote HTTP imports
  (as well as `data` and `blob` URLs) and will go ahead and fetch the remote
  code and cache it locally, similar to the way a browser works.

In order to help mitigate these differences, we will further explore in this
chapter:

- Using the [`std/node`](./npm_nodejs/std_node.md) modules of the Deno standard
  library to "polyfill" the built-in modules of Node.js
- Using [CDNs](./npm_nodejs/cdns.md) to access the vast majority of npm packages
  in ways that work under Deno.
- How [import maps](./npm_nodejs/import_maps.md) can be used to provide "bare
  specifier" imports like Node.js under Deno, without needing to use a package
  manager to install packages locally.
- And finally, a general section of
  [frequently asked questions](./npm_nodejs/faqs.md)

That being said, there are some differences that cannot be overcome:

- Node.js has a plugin system that is incompatible with Deno, and Deno will
  never support Node.js plugins. If the Node.js code you want to use requires a
  "native" Node.js plugin, it won't work under Deno.
- Node.js has some built in modules (e.g. like `vm`) that are effectively
  incompatible with the scope of Deno and therefore there aren't easy ways to
  provide a _polyfill_ of the functionality in Deno.
