Requires `for` loop control variables to increment in the correct direction

Incrementing `for` loop control variables in the wrong direction leads to
infinite loops. This can occur through incorrect initialization, bad
continuation step logic or wrong direction incrementing of the loop control
variable.

### Invalid:

```typescript
// Infinite loop
for (let i = 0; i < 2; i--) {}
```

### Valid:

```typescript
for (let i = 0; i < 2; i++) {}
```
