Pass children as JSX children instead of as an attribute.

### Invalid:

```tsx
<div children="foo" />
<div children={[<Foo />, <Bar />]} />
```

### Valid:

```tsx
<div>foo</div>
<div><Foo /><Bar /></div>
```
