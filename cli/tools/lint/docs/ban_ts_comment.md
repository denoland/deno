Disallows the use of Typescript directives without a comment.

Typescript directives reduce the effectiveness of the compiler, something which
should only be done in exceptional circumstances. The reason why should be
documented in a comment alongside the directive.

### Invalid:

```typescript
// @ts-expect-error
let a: number = "I am a string";
```

```typescript
// @ts-ignore
let a: number = "I am a string";
```

```typescript
// @ts-nocheck
let a: number = "I am a string";
```

### Valid:

```typescript
// @ts-expect-error: Temporary workaround (see ticket #422)
let a: number = "I am a string";
```

```typescript
// @ts-ignore: Temporary workaround (see ticket #422)
let a: number = "I am a string";
```

```typescript
// @ts-nocheck: Temporary workaround (see ticket #422)
let a: number = "I am a string";
```
