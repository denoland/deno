Disallows the use of the `console` global.

Oftentimes, developers accidentally commit `console.log`/`console.error`
statements, left in particularly after debugging. Moreover, using these in code
may leak sensitive information to the output or clutter the console with
unnecessary information. This rule helps maintain clean and secure code by
disallowing the use of `console`.

This rule is especially useful in libraries where you almost never want to
output to the console.

### Invalid

```typescript
console.log("Debug message");
console.error("Debug message");
console.debug(obj);

if (debug) console.log("Debugging");

function log() {
  console.log("Log");
}
```

### Valid

It is recommended to explicitly enable the console via a `deno-lint-ignore`
comment for any calls where you actually want to use it.

```typescript
function logWarning(message: string) {
  // deno-lint-ignore no-console
  console.warn(message);
}
```
