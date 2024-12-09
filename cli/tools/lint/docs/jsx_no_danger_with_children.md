Using JSX children together with `dangerouslySetInnerHTML` is invalid as they
will be ignored.

### Invalid:

```tsx
<div dangerouslySetInnerHTML={{ __html: "<h1>hello</h1>" }}>
  <h1>this will never be rendered</h1>
</div>;
```

### Valid:

```tsx
<div dangerouslySetInnerHTML={{ __html: "<h1>hello</h1>" }} />;
```
