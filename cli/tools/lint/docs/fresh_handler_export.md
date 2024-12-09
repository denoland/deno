Checks correct naming for named fresh middleware export

Files inside the `routes/` folder can export middlewares that run before any
rendering happens. They are expected to be available as a named export called
`handler`. This rule checks for when the export was incorrectly named `handlers`
instead of `handler`.

### Invalid:

```js
export const handlers = {
  GET() {},
  POST() {},
};
export function handlers() {}
export async function handlers() {}
```

### Valid:

```jsx
export const handler = {
  GET() {},
  POST() {},
};
export function handler() {}
export async function handler() {}
```
