Disallows the use of top level await expressions.

Top level await cannot be used when distributing CommonJS/UMD via dnt.

### Invalid:

```typescript
await foo();
for await (item of items) {}
```

### Valid:

```typescript
async function foo() {
  await task();
}
async function foo() {
  for await (item of items) {}
}
```
