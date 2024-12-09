Prevent the use of `dangerouslySetInnerHTML` which can lead to XSS
vulnerabilities if used incorrectly.

### Invalid:

```tsx
const hello = <div dangerouslySetInnerHTML={{ __html: "Hello World!" }} />;
```

### Valid:

```tsx
const hello = <div>Hello World!</div>;
```
