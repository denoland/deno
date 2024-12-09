Requires all functions called with any amount of `boolean` literals as
parameters to use a self-documenting constant instead.

Is common to define functions that can take `booleans` as arguments. However,
passing `boolean` literals as parameters can lead to lack of context regarding
the role of the argument inside the function in question.

A simple fix for the points mentioned above is the use of self documenting
constants that will end up working as "named booleans", that allow for a better
understanding on what the parameters mean in the context of the function call.

### Invalid

```typescript
function redraw(allViews: boolean, inline: boolean) {
  // redraw logic.
}
redraw(true, true);

function executeCommand(recursive: boolean, executionMode: EXECUTION_MODES) {
  // executeCommand logic.
}
executeCommand(true, EXECUTION_MODES.ONE);

function enableLogs(enable: boolean) {
  // enabledLogs logic.
}
enableLogs(true);
```

### Valid

```typescript
function redraw(allViews: boolean, inline: boolean) {
  // redraw logic.
}
const ALL_VIEWS = true, INLINE = true;
redraw(ALL_VIEWS, INLINE);

function executeCommand(recursive: boolean, executionMode: EXECUTION_MODES) {
  // executeCommand logic.
}
const RECURSIVE = true;
executeCommand(RECURSIVE, EXECUTION_MODES.ONE);

function enableLogs(enable: boolean) {
  // enabledLogs logic.
}
const ENABLE = true;
enableLogs(ENABLE);
```
