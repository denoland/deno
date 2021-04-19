## Configuring TypeScript in Deno

TypeScript comes with a load of different options that can be configured, but
Deno strives to make it easy to use TypeScript with Deno. Lots of different
options frustrates that goal. To make things easier, Deno configures TypeScript
to "just work" and shouldn't require additional configuration.

That being said, Deno does support using a TypeScript configuration file, though
like the rest of Deno, the detection and use of use of a configuration file is
not automatic. To use a TypeScript configuration file with Deno, you have to
provide a path on the command line. For example:

```
> deno run --config ./tsconfig.json main.ts
```

> ⚠️ Do consider though that if you are creating libraries that require a
> configuration file, all of the consumers of your modules will require that
> configuration file too if you distribute your modules as TypeScript. In
> addition, there could be settings you do in the configuration file that make
> other TypeScript modules incompatible. Honestly it is best to use the Deno
> defaults and to think long and hard about using a configuration file.

### How Deno uses a configuration file

Deno does not process a TypeScript configuration file like `tsc` does, as there
are lots of parts of a TypeScript configuration file that are meaningless in a
Deno context or would cause Deno to not function properly if they were applied.

Deno only looks at the `compilerOptions` section of a configuration file, and
even then it only considers certain compiler options, with the rest being
ignored.

Here is a table of compiler options that can be changed, their default in Deno
and any other notes about that option:

| Option                           | Default                 | Notes                                                                                                                                                                                                                                                                            |
| -------------------------------- | ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `allowJs`                        | `true`                  | This almost never needs to be changed                                                                                                                                                                                                                                            |
| `allowUnreachableCode`           | `false`                 |                                                                                                                                                                                                                                                                                  |
| `allowUnusedLabels`              | `false`                 |                                                                                                                                                                                                                                                                                  |
| `checkJs`                        | `false`                 | If `true` causes TypeScript to type check JavaScript                                                                                                                                                                                                                             |
| `experimentalDecorators`         | `true`                  | We enable these by default as they are already opt-in in the code and when we skip type checking, the Rust based emitter has them on by default. We strongly discourage the use of legacy decorators, as they are incompatible with the future decorators standard in JavaScript |
| `jsx`                            | `"react"`               |                                                                                                                                                                                                                                                                                  |
| `jsxFactory`                     | `"React.createElement"` |                                                                                                                                                                                                                                                                                  |
| `jsxFragmentFactory`             | `"React.Fragment"`      |                                                                                                                                                                                                                                                                                  |
| `keyofStringsOnly`               | `false`                 |                                                                                                                                                                                                                                                                                  |
| `lib`                            | `[ "deno.window" ]`     | The default for this varies based on other settings in Deno. If it is supplied, it overrides the default. See below for more information.                                                                                                                                        |
| `noFallthroughCasesInSwitch`     | `false`                 |                                                                                                                                                                                                                                                                                  |
| `noImplicitAny`                  | `true`                  |                                                                                                                                                                                                                                                                                  |
| `noImplicitReturns`              | `false`                 |                                                                                                                                                                                                                                                                                  |
| `noImplicitThis`                 | `true`                  |                                                                                                                                                                                                                                                                                  |
| `noImplicitUseStrict`            | `true`                  |                                                                                                                                                                                                                                                                                  |
| `noStrictGenericChecks`          | `false`                 |                                                                                                                                                                                                                                                                                  |
| `noUnusedLocals`                 | `false`                 |                                                                                                                                                                                                                                                                                  |
| `noUnusedParameters`             | `false`                 |                                                                                                                                                                                                                                                                                  |
| `reactNamespace`                 | `React`                 |                                                                                                                                                                                                                                                                                  |
| `strict`                         | `true`                  |                                                                                                                                                                                                                                                                                  |
| `strictBindCallApply`            | `true`                  |                                                                                                                                                                                                                                                                                  |
| `strictFunctionTypes`            | `true`                  |                                                                                                                                                                                                                                                                                  |
| `strictPropertyInitialization`   | `true`                  |                                                                                                                                                                                                                                                                                  |
| `strictNullChecks`               | `true`                  |                                                                                                                                                                                                                                                                                  |
| `suppressExcessPropertyErrors`   | `false`                 |                                                                                                                                                                                                                                                                                  |
| `suppressImplicitAnyIndexErrors` | `false`                 |                                                                                                                                                                                                                                                                                  |

For a full list of compiler options and how they affect TypeScript, please refer
to the
[TypeScript Handbook](https://www.typescriptlang.org/docs/handbook/compiler-options.html)

### What an implied tsconfig.json looks like

It is impossible to get `tsc` to behave like Deno. It is also difficult to get
the TypeScript language service to behave like Deno. This is why we have built a
language service directly into Deno. That being said, it can be useful to
understand what is implied.

If you were to write a `tsconfig.json` for Deno, it would look something like
this:

```json
{
  "compilerOptions": {
    "allowJs": true,
    "esModuleInterop": true,
    "experimentalDecorators": true,
    "inlineSourceMap": true,
    "isolatedModules": true,
    "jsx": "react",
    "lib": ["deno.window"],
    "module": "esnext",
    "strict": true,
    "target": "esnext",
    "useDefineForClassFields": true
  }
}
```

You can't copy paste this into a `tsconfig.json` and get it to work,
specifically because of the built in type libraries that are custom to Deno
which are provided to the TypeScript compiler. This can somewhat be mocked by
running `deno types` on the command line and piping the output to a file and
including that in the files as part of the program, removing the `"lib"` option,
and setting the `"noLib"` option to `true`.

If you use the `--unstable` flag, Deno will change the `"lib"` option to
`[ "deno.window", "deno.unstable" ]`. If you are trying to load a worker, that
is type checked with `"deno.worker"` instead of `"deno.window"`.

### Using the "lib" property

[TBC]
