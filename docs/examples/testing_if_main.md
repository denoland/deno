## Testing if current file is the main program

To test if the current script has been executed as the main input to the program
check `import.meta.main`.

```ts
if (import.meta.main) {
  console.log("main");
}
```

Please also note that running:

```shell
deno eval "console.log(import.meta.main)"
```

Outputs `true` due to the behavior of the _eval_ command. Deno stores
your code in a temporary file then runs it. For more info check the
[source code](https://github.com/denoland/deno/blob/master/cli/main.rs#L341)
of the _eval_ command.
