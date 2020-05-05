## Testing if current file is the main program

To test if the current script has been executed as the main input to the program
check `import.meta.main`.

```ts
if (import.meta.main) {
  console.log("main");
}
```
