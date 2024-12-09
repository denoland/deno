Disallows the use of the `debugger` statement

`debugger` is a statement which is meant for stopping the javascript execution
environment and start the debugger at the statement. Modern debuggers and
tooling no longer need this statement and leaving it in can cause the execution
of your code to stop in production.

### Invalid:

```typescript
function isLongString(x: string) {
  debugger;
  return x.length > 100;
}
```

### Valid:

```typescript
function isLongString(x: string) {
  return x.length > 100; // set breakpoint here instead
}
```
