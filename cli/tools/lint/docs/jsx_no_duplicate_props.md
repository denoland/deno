Disallow duplicated JSX props. Later props will always overwrite earlier props
often leading to unexpected results.

### Invalid:

```tsx
<div id="1" id="2" />;
<App a a />;
<App a {...b} a />;
```

### Valid:

```tsx
<div id="1" />
<App a />
<App a {...b} />
<App {...b} b />
```
