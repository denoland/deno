# Module metadata

## Concepts

- [import.meta](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/import.meta)
  can provide information on the context of the module.
- The boolean
  [import.meta.main](https://doc.deno.land/builtin/stable#ImportMeta) will let
  you know if the current module is the program entry point.
- The string [import.meta.url](https://doc.deno.land/builtin/stable#ImportMeta)
  will give you the URL of the current module.
- The string
  [Deno.mainModule](https://doc.deno.land/builtin/stable#Deno.mainModule) will
  give you the URL of the main module entry point, i.e. the module invoked by
  the deno runtime.

## Example

The example below uses two modules to show the difference between
`import.meta.url`, `import.meta.main` and `Deno.mainModule`. In this example,
`module_a.ts` is the main module entry point:

```ts
/**
 * module_b.ts
 */
export function outputB() {
  console.log("Module B's import.meta.url", import.meta.url);
  console.log("Module B's mainModule url", Deno.mainModule);
  console.log(
    "Is module B the main module via import.meta.main?",
    import.meta.main,
  );
}
```

```ts
/**
 * module_a.ts
 */
import { outputB } from "./module_b.ts";

function outputA() {
  console.log("Module A's import.meta.url", import.meta.url);
  console.log("Module A's mainModule url", Deno.mainModule);
  console.log(
    "Is module A the main module via import.meta.main?",
    import.meta.main,
  );
}

outputA();
console.log("");
outputB();
```

If `module_a.ts` is located in `/home/alice/deno` then the output of
`deno run --allow-read module_a.ts` is:

```
Module A's import.meta.url file:///home/alice/deno/module_a.ts
Module A's mainModule url file:///home/alice/deno/module_a.ts
Is module A the main module via import.meta.main? true

Module B's import.meta.url file:///home/alice/deno/module_b.ts
Module B's mainModule url file:///home/alice/deno/module_a.ts
Is module B the main module via import.meta.main? false
```
