Fragments are only necessary at the top of a JSX "block" and only when there are
multiple children. Fragments are not needed in other scenarios.

### Invalid:

```tsx
<></>
<><div /></>
<><App /></>
<p>foo <>bar</></p>
```

### Valid:

```tsx
<>{foo}</>
<><div /><div /></>
<>foo <div /></>
<p>foo bar</p>
```
