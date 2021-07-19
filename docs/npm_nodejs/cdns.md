## Packages from CDNs

Because Deno supports remote HTTP modules, and content delivery networks (CDNs)
can be powerful tools to transform code, the combination allows an easy way to
access code in the npm registry via Deno, usually in a way that works with Deno
without any further actions, and often enriched with TypeScript types. In this
section we will explore that in detail.

### What about `deno.land/x/`?

The [`deno.land/x/`](https://deno.land/x/) is a public registry for code,
hopefully code written specifically for Deno. It is a public registry though and
all it does is "redirect" Deno to the location where the code exists. It doesn't
transform the code in any way. There is a lot of great code on the registry, but
at the same time, there is some code that just isn't well maintained (or doesn't
work at all). If you are familiar with the npm registry, you know that as well,
there are varying degrees of quality.

Because it simply serves up the original published source code, it doesn't
really help when trying to use code that didn't specifically consider Deno when
authored.

### Deno "friendly" CDNs

Deno friendly content delivery networks (CDNs) not only host packages from npm,
they provide them in a way that maximizes their integration to Deno. They
directly address some of the challenges in consuming code written for Node.js:

- The provide packages and modules in the ES Module format, irrespective of how
  they are published on npm.
- They resolve all the dependencies as the modules are served, meaning that all
  the Node.js specific module resolution logic is handled by the CDN.
- Often, they inform Deno of type definitions for a package, meaning that Deno
  can use them to type check your code and provide a better development
  experience.
- The CDNs also "polyfill" the built-in Node.js modules, making a lot of code
  that leverages the built-in Node.js modules _just work_.
- The CDNs deal with all the semver matching for packages that a package manager
  like `npm` would be required for a Node.js application, meaning you as a
  developer can express your 3rd party dependency versioning as part of the URL
  you use to import the package.

#### esm.sh

[esm.sh](https://esm.sh/) is a CDN that was specifically designed for Deno,
though addressing the concerns for Deno also makes it a general purpose CDN for
accessing npm packages as ES Module bundles. esm.sh uses
[esbuild](https://esbuild.github.io/) to take an arbitrary npm package and
ensure that it is consumable as an ES Module. In many cases you can just import
the npm package into your Deno application:

```ts
import React from "https://esm.sh/react";

export default class A extends React.Component {
  render() {
    return (
      <div></div>
    );
  }
}
```

esm.sh supports the use of both specific versions of packages, as well as
[semver](https://semver.npmjs.com/) versions of packages, so you can express
your dependency in a similar way you would in a `package.json` file when you
import it. For example, to get a specific version of a package:

```ts
import React from "https://esm.sh/react@17.0.2";
```

Or to get the latest patch release of a minor release:

```ts
import React from "https://esm.sh/react@~16.13.0";
```

esm.sh uses the `std/node` polyfills to replace the built-in modules in Node.js,
meaning that code that uses those built-in modules will have the same
limitations and caveats as those modules in `std/node`.

esm.sh also automatically sets a header which Deno recognizes that allows Deno
to be able to retrieve type definitions for the package/module. See
[Using `X-TypeScript-Types` header](../typescript/types.md#using-x-typescript-types-header)
in this manual for more details on how this works.

The CDN is also a good choice for people who develop in mainland China, as the
hosting of the CDN is specifically designed to work with "the great firewall of
China", as well as esm.sh provides information on self hosting the CDN as well.

Check out the [esm.sh homepage](https://esm.sh/) for more detailed information
on how the CDN can be used and what features it has.

#### Skypack

[Skypack.dev](https://www.skypack.dev/) is designed to make development overall
easier by not requiring packages to be installed locally, even for Node.js
development, and to make it easy to create web and Deno applications that
leverage code from the npm registry.

Skypack has a great way of discovering packages in the npm registry, by
providing a lot of contextual information about the package, as well as a
"scoring" system to try to help determine if the package follows best-practices.

Skypack detects Deno's user agent when requests for modules are received and
ensures the code served up is tailored to meet the needs of Deno. The easiest
way to load a package is to use the
[lookup URL](https://docs.skypack.dev/skypack-cdn/api-reference/lookup-urls) for
the package:

```ts
import React from "https://cdn.skypack.dev/react";

export default class A extends React.Component {
  render() {
    return (
      <div></div>
    );
  }
}
```

Lookup URLs can also contain the [semver](https://semver.npmjs.com/) version in
the URL:

```ts
import React from "https://cdn.skypack.dev/react@~16.13.0";
```

By default, Skypack does not set the types header on packages. In order to have
the types header set, which is automatically recognized by Deno, you have to
append `?dts` to the URL for that package:

```ts
import { pathToRegexp } from "https://cdn.skypack.dev/path-to-regexp?dts";

const re = pathToRegexp("/path/:id");
```

See
[Using `X-TypeScript-Types` header](../typescript/types.md#using-x-typescript-types-header)
in this manual for more details on how this works.

Skypack docs have a
[specific page on usage with Deno](https://docs.skypack.dev/skypack-cdn/code/deno)
for more information.

### Other CDNs

There are a couple of other CDNs worth mentioning.

#### UNPKG

[UNPKG](https://unpkg.com/) is the most well known CDN for npm packages. For
packages that include an ES Module distribution for things like the browsers,
many of them can be used directly off of UNPKG. That being said, everything
available on UNPKG is available on more Deno friendly CDNs.

#### JSPM

The [jspm.io](https://jspm.io) CDN is specifically designed to provide npm and
other registry packages as ES Modules in a way that works well with import maps.
While it doesn't currently cater to Deno, the fact that Deno can utilize import
maps, allows you to use the [JSPM.io generator](https://generator.jspm.io/) to
generate an import-map of all the packages you want to use and have them served
up from the CDN.

### Considerations

While CDNs can make it easy to allow Deno to consume packages and modules from
the npm registry, there can still be some things to consider:

- Deno does not (and will not) support Node.js plugins. If the package requires
  a native plugin, it won't work under Deno.
- Dependency management can always be a bit of a challenge and a CDN can make it
  a bit more obfuscated what dependencies are there. You can always use
  `deno info` with the module or URL to get a full breakdown of how Deno
  resolves all the code.
- While the Deno friendly CDNs try their best to serve up types with the code
  for consumption with Deno, lots of types for packages conflict with other
  packages and/or don't consider Deno, which means you can often get strange
  diagnostic message when type checking code imported from these CDNs, though
  skipping type checking will result in the code working perfectly fine. This is
  a fairly complex topic and is covered in the
  [Types and type declarations](../typescript/types.md) section of the manual.
