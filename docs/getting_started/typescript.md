## Using TypeScript

<!-- TODO(lucacasonato): text on 'just import .ts' -->

### Using external type definitions

Deno supports both JavaScript and TypeScript as first class languages at
runtime. This means it requires fully qualified module names, including the
extension (or a server providing the correct media type). In addition, Deno has
no "magical" module resolution.

The out of the box TypeScript compiler though relies on both extension-less
modules and the Node.js module resolution logic to apply types to JavaScript
modules.

In order to bridge this gap, Deno supports three ways of referencing type
definition files without having to resort to "magic" resolution.

#### Compiler hint

If you are importing a JavaScript module, and you know where the type definition
for that module is located, you can specify the type definition at import. This
takes the form of a compiler hint. Compiler hints inform Deno the location of
`.d.ts` files and the JavaScript code that is imported that they relate to. The
hint is `@deno-types` and when specified the value will be used in the compiler
instead of the JavaScript module. For example, if you had `foo.js`, but you know
that along side of it was `foo.d.ts` which was the types for the file, the code
would look like this:

```ts
// @deno-types="./foo.d.ts"
import * as foo from "./foo.js";
```

The value follows the same resolution logic as importing a module, meaning the
file needs to have an extension and is relative to the current module. Remote
specifiers are also allowed.

The hint affects the next `import` statement (or `export ... from` statement)
where the value of the `@deno-types` will be substituted at compile time instead
of the specified module. Like in the above example, the Deno compiler will load
`./foo.d.ts` instead of `./foo.js`. Deno will still load `./foo.js` when it runs
the program.

#### Triple-slash reference directive in JavaScript files

If you are hosting modules which you want to be consumed by Deno, and you want
to inform Deno about the location of the type definitions, you can utilise a
triple-slash directive in the actual code. For example, if you have a JavaScript
module and you would like to provide Deno with the location of the type
definitions which happen to be alongside that file, your JavaScript module named
`foo.js` might look like this:

```js
/// <reference types="./foo.d.ts" />
export const foo = "foo";
```

Deno will see this, and the compiler will use `foo.d.ts` when type checking the
file, though `foo.js` will be loaded at runtime. The resolution of the value of
the directive follows the same resolution logic as importing a module, meaning
the file needs to have an extension and is relative to the current file. Remote
specifiers are also allowed.

#### X-TypeScript-Types custom header

If you are hosting modules which you want to be consumed by Deno, and you want
to inform Deno the location of the type definitions, you can use a custom HTTP
header of `X-TypeScript-Types` to inform Deno of the location of that file.

The header works in the same way as the triple-slash reference mentioned above,
it just means that the content of the JavaScript file itself does not need to be
modified, and the location of the type definitions can be determined by the
server itself.

**Not all type definitions are supported.**

Deno will use the compiler hint to load the indicated `.d.ts` files, but some
`.d.ts` files contain unsupported features. Specifically, some `.d.ts` files
expect to be able to load or reference type definitions from other packages
using the module resolution logic. For example a type reference directive to
include `node`, expecting to resolve to some path like
`./node_modules/@types/node/index.d.ts`. Since this depends on non-relative
"magical" resolution, Deno cannot resolve this.

**Why not use the triple-slash type reference in TypeScript files?**

The TypeScript compiler supports triple-slash directives, including a type
reference directive. If Deno used this, it would interfere with the behavior of
the TypeScript compiler. Deno only looks for the directive in JavaScript (and
JSX) files.

### Custom TypeScript Compiler Options

In the Deno ecosystem, all strict flags are enabled in order to comply with
TypeScript's ideal of being `strict` by default. However, in order to provide a
way to support customization a configuration file such as `tsconfig.json` might
be provided to Deno on program execution.

You need to explicitly tell Deno where to look for this configuration by setting
the `-c` argument when executing your application.

```bash
deno run -c tsconfig.json mod.ts
```

Following are the currently allowed settings and their default values in Deno:

```json
{
  "compilerOptions": {
    "allowJs": false,
    "allowUmdGlobalAccess": false,
    "allowUnreachableCode": false,
    "allowUnusedLabels": false,
    "alwaysStrict": true,
    "assumeChangesOnlyAffectDirectDependencies": false,
    "checkJs": false,
    "disableSizeLimit": false,
    "generateCpuProfile": "profile.cpuprofile",
    "jsx": "react",
    "jsxFactory": "React.createElement",
    "lib": [],
    "noFallthroughCasesInSwitch": false,
    "noImplicitAny": true,
    "noImplicitReturns": true,
    "noImplicitThis": true,
    "noImplicitUseStrict": false,
    "noStrictGenericChecks": false,
    "noUnusedLocals": false,
    "noUnusedParameters": false,
    "preserveConstEnums": false,
    "removeComments": false,
    "resolveJsonModule": true,
    "strict": true,
    "strictBindCallApply": true,
    "strictFunctionTypes": true,
    "strictNullChecks": true,
    "strictPropertyInitialization": true,
    "suppressExcessPropertyErrors": false,
    "suppressImplicitAnyIndexErrors": false,
    "useDefineForClassFields": false
  }
}
```

For documentation on allowed values and use cases please visit the
[typescript docs](https://www.typescriptlang.org/docs/handbook/compiler-options.html).

**Note**: Any options not listed above are either not supported by Deno or are
listed as deprecated/experimental in the TypeScript documentation.
