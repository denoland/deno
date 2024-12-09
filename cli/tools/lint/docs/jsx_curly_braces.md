Ensure consistent use of curly braces around JSX expressions.

### Invalid:

```tsx
const foo = <Foo foo=<div /> />;
const foo = <Foo str={"foo"} />;
const foo = <div>{"foo"}</div>;
```

### Valid:

```tsx
const foo = <Foo foo={<div />} />;
const foo = <Foo str="foo" />;
const foo = <div>foo</div>;
```
