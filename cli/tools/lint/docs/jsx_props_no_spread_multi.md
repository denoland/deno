Spreading the same expression twice is typically a mistake and causes
unnecessary computations.

### Invalid:

```tsx
<div {...foo} {...foo} />
<div {...foo} a {...foo} />
<Foo {...foo.bar} {...foo.bar} />
```

### Valid:

```tsx
<div {...foo} />
<div {...foo.bar} a />
<Foo {...foo.bar} />
```
