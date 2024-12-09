Enforces default parameter(s) to be last in the function signature.

Parameters with default values are optional by nature but cannot be left out of
the function call without mapping the function inputs to different parameters
which is confusing and error prone. Specifying them last allows them to be left
out without changing the semantics of the other parameters.

### Invalid:

```typescript
function f(a = 2, b) {}
function f(a = 5, b, c = 5) {}
```

### Valid:

```typescript
function f() {}
function f(a) {}
function f(a = 5) {}
function f(a, b = 5) {}
function f(a, b = 5, c = 5) {}
function f(a, b = 5, ...c) {}
function f(a = 2, b = 3) {}
```
