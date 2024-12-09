Ensure the `key` attribute is present when passing iterables into JSX. It allows
frameworks to optimize checking the order of elements.

### Invalid:

```tsx
const foo = [<div>foo</div>];
const foo = [<>foo</>];
[1, 2, 3].map(() => <div />);
Array.from([1, 2, 3], () => <div />);
```

### Valid:

```tsx
const foo = [<div key="a">foo</div>];
const foo = [<Fragment key="b">foo</Fragment>];
[1, 2, 3].map((x) => <div key={x} />);
Array.from([1, 2, 3], (x) => <div key={x} />);
```
