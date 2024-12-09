Enforce a consistent JSX boolean value style. Passing `true` as the boolean
value can be omitted with the shorthand syntax.

### Invalid:

```tsx
const foo = <Foo isFoo={true} />;
const foo = <Foo isFoo={false} />;
```

### Valid:

```tsx
const foo = <Foo isFoo />;
const foo = <Foo isFoo={false} />;
```
