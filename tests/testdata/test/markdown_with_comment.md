# Documentation

The following examples are inside HTML comments and will not trigger the
type-checker:

<!-- ```ts ignore
const value: Invalid = "ignored";
``` -->

<!--
```ts
const a: string = 42;
```
-->

<!--

This is a comment.

```ts
const a: string = 42;
```

Something something more comments.

```typescript
const a: boolean = "true";
```

-->

The following example will trigger the type-checker to fail:

```ts
const a: string = 42;
```
