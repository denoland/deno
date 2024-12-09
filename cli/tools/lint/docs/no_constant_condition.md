Disallows the use of a constant expression in conditional test

Using a constant expression in a conditional test is often either a mistake or a
temporary situation introduced during development and is not ready for
production.

### Invalid:

```typescript
if (true) {}
if (2) {}
do {} while (x = 2); // infinite loop
```

### Valid:

```typescript
if (x) {}
if (x === 0) {}
do {} while (x === 2);
```
