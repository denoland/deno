Disallow throwing literals as exceptions

It is considered good practice to only `throw` the `Error` object itself or an
object using the `Error` object as base objects for user-defined exceptions. The
fundamental benefit of `Error` objects is that they automatically keep track of
where they were built and originated.

### Invalid:

```typescript
throw "error";
throw 0;
throw undefined;
throw null;
```

### Valid:

```typescript
throw new Error("error");
```
