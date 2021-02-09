## Runtime compiler APIs

> ⚠️ The runtime compiler API is unstable (and requires the `--unstable` flag to
> be used to enable it).

The runtime compiler API allows access to the internals of Deno to be able to
type check, transpile and bundle JavaScript and TypeScript. As of Deno 1.7,
several disparate APIs we consolidated into a single API, `Deno.emit()`.

### Deno.emit()

The API is defined in the `Deno` namespace as:

```ts
function emit(
  rootSpecifier: string | URL,
  options?: EmitOptions,
): Promise<EmitResult>;
```

The emit options are defined in the `Deno` namespace as:

```ts
interface EmitOptions {
  /** Indicate that the source code should be emitted to a single file
   * JavaScript bundle that is an ES module (`"esm"`). */
  bundle?: "esm";
  /** If `true` then the sources will be typed checked, returning any
   * diagnostic errors in the result.  If `false` type checking will be
   * skipped.  Defaults to `true`.
   * 
   * *Note* by default, only TypeScript will be type checked, just like on
   * the command line.  Use the `compilerOptions` options of `checkJs` to
   * enable type checking of JavaScript. */
  check?: boolean;
  /** A set of options that are aligned to TypeScript compiler options that
   * are supported by Deno. */
  compilerOptions?: CompilerOptions;
  /** An [import-map](https://deno.land/manual/linking_to_external_code/import_maps#import-maps)
   * which will be applied to the imports. */
  importMap?: ImportMap;
  /** An absolute path to an [import-map](https://deno.land/manual/linking_to_external_code/import_maps#import-maps).
   * Required to be specified if an `importMap` is specified to be able to
   * determine resolution of relative paths. If a `importMap` is not
   * specified, then it will assumed the file path points to an import map on
   * disk and will be attempted to be loaded based on current runtime
   * permissions.
   */
  importMapPath?: string;
  /** A record of sources to use when doing the emit.  If provided, Deno will
   * use these sources instead of trying to resolve the modules externally. */
  sources?: Record<string, string>;
}
```

The emit result is defined in the `Deno` namespace as:

```ts
interface EmitResult {
  /** Diagnostic messages returned from the type checker (`tsc`). */
  diagnostics: Diagnostic[];
  /** Any emitted files.  If bundled, then the JavaScript will have the
   * key of `deno:///bundle.js` with an optional map (based on
   * `compilerOptions`) in `deno:///bundle.js.map`. */
  files: Record<string, string>;
  /** An optional array of any compiler options that were ignored by Deno. */
  ignoredOptions?: string[];
  /** An array of internal statistics related to the emit, for diagnostic
   * purposes. */
  stats: Array<[string, number]>;
}
```

The API is designed to support several use cases, which are described in the
sections below.

### Using external sources

Using external sources, both local and remote, `Deno.emit()` can behave like
`deno cache` does on the command line, resolving those external dependencies,
type checking those dependencies, and providing an emitted output.

By default, `Deno.emit()` will utilise external resources. The _rootSpecifier_
supplied as the first argument will determine what module will be used as the
root. The root module is similar to what you would provide on the command line.

For example if you did:

```
> deno run mod.ts
```

You could do something similar with `Deno.emit()`:

```ts
try {
  const { files } = await Deno.emit("mod.ts");
  for (const [fileName, text] of Object.entries(files)) {
    console.log(`emitted ${fileName} with a length of ${text.length}`);
  }
} catch (e) {
  // something went wrong, inspect `e` to determine
}
```

`Deno.emit()` will use the same on disk cache for remote modules that the
standard CLI does, and it inherits the permissions and cache options of the
process that executes it.

If the _rootSpecifier_ is a relative path, then the current working directory of
the Deno process will be used to resolve the specifier. (Not relative to the
current module!)

The _rootSpecifier_ can be a string file path, a string URL, or a URL.
`Deno.emit()` supports the same protocols for URLs that Deno supports, which are
currently `file`, `http`, `https`, and `data`.

### Providing sources

Instead of resolving modules externally, you can provide `Deno.emit()` with the
sources directly. This is especially useful for a server to be able to provide
_on demand_ compiling of code supplied by a user, where the Deno process has
collected all the code it wants to emit.

The sources are passed in the _sources_ property of the `Deno.emit()` _options_
argument:

```ts
const { files } = await Deno.emit("/mod.ts", {
  sources: {
    "/mod.ts": `import * as a from "./a.ts";\nconsole.log(a);\n`,
    "/a.ts": `export const a: Record<string, string> = {};\n`,
  },
});
```

When sources are provided, Deno will no longer look externally and will try to
resolve all modules from within the map of sources provided, though the module
resolution follow the same rules as if the modules were external. For example
all module specifiers need their full filename. Also, because there are no media
types, if you are providing remote URLs in the sources, the path should end with
the appropriate extension, so that Deno can determine how to handle the file.

### Type checking and emitting

By default, `Deno.emit()` will type check any TypeScript (and TSX) it
encounters, just like on the command line. It will also attempt to transpile
JSX, but will leave JavaScript "alone". This behavior can be changed by changing
the compiler options. For example if you wanted Deno to type check your
JavaScript as well, you could set the _checkJs_ option to `true` in the compiler
options:

```ts
const { files, diagnostics } = await Deno.emit("./mod.js", {
  compilerOptions: {
    checkJs: true,
  },
});
```

The `Deno.emit()` result provides any diagnostic messages about the code
supplied. On the command line, any diagnostic messages get logged to stderr and
the Deno process terminates, but with `Deno.emit()` they are returned to the
caller.

Typically you will want to check if there are any diagnostics and handle them
appropriately. You can introspect the diagnostics individually, but there is a
handy formatting function available to make it easier to potentially log the
diagnostics to the console for the user called `Deno.formatDiagnostics()`:

```ts
const { files, diagnostics } = await Deno.emit("./mod.ts");
if (diagnostics.length) {
  // there is something that impacted the emit
  console.warn(Deno.formatDiagnostics(diagnostics));
}
```

### Bundling

`Deno.emit()` is also capable of providing output similar to `deno bundle` on
the command line. This is enabled by setting the _bundle_ option to `"esm"`.
(Currently Deno only supports bundling as a single file ES module, but there are
plans to add support for an IIFE bundle format as well):

```ts
const { files, diagnostics } = await Deno.emit("./mod.ts", {
  bundle: "esm",
});
```

The _files_ of the result will contain a single key named `deno:///bundle.js` of
which the value with be the resulting bundle.

> ⚠️ Just like with `deno bundle`, the bundle will not include things like
> dynamic imports or worker scripts, and those would be expected to be resolved
> and available when the code is run.

### Import maps

`Deno.emit()` supports import maps as well, just like on the command line. This
is a really powerful feature that can be used even more effectively to emit and
bundle code.

Because of the way import maps work, when using with `Deno.emit()` you also have
to supply an absolute URL for the import map. This allows Deno to resolve any
relative URLs specified in the import map. This needs to be supplied even if the
import map doesn't contain any relative URLs. The URL does not need to really
exist, it is just feed to the API.

An example might be that I want to use a bare specifier to load a special
version of _lodash_ I am using with my project. I could do the following:

```ts
const { files } = await Deno.emit("mod.ts", {
  bundle: "esm",
  importMap: {
    imports: {
      "lodash": "https://deno.land/x/lodash",
    },
  },
  importMapPath: "file:///import-map.json",
});
```

> ⚠️ If you are not bundling your code, the emitted code specifiers do not get
> rewritten, that means that whatever process will consume the code, Deno or a
> browser for example, would need to support import maps and have that map
> available at runtime.

### Skip type checking/transpiling only

`Deno.emit()` supports skipping type checking similar to the `--no-check` flag
on the command line. This is accomplished by setting the _check_ property to
`false`:

```ts
const { files } = await Deno.emit("./mod.ts", {
  check: false,
});
```

Setting _check_ to `false` will instruct Deno to not utilise the TypeScript
compiler to type check the code and emit it, instead only transpiling the code
from within Deno. This can be significantly quicker than doing the full type
checking.

### Compiler options

`Deno.emit()` supports quite a few compiler options that can impact how code is
type checked and emitted. They are similar to the options supported by a
`tsconfig.json` in the `compilerOptions` section, but there are several options
that are not supported. This is because they are either meaningless in Deno or
would cause Deno to not be able to work properly. The defaults for `Deno.emit()`
are the same defaults that are on the command line. The options are
[documented here](https://doc.deno.land/builtin/unstable#Deno.CompilerOptions)
along with their default values and are built into the Deno types.

If you are type checking your code, the compiler options will be type checked
for you, but if for some reason you are either dynamically providing the
compiler options or are not type checking, then the result of `Deno.emit()` will
provide you with an array of _ignoredOptions_ if there are any.

> ⚠️ we have only tried to disable/remove options that we know won't work, that
> does not mean we extensively test all options in all configurations under
> `Deno.emit()`. You may find that some behaviors do not match what you can get
> from `tsc` or are otherwise incompatible. If you do find something that
> doesn't work, please do feel free to raise an issue.
