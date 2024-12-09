Checks that a `<button>` JSX element has a valid `type` attribute. The default
value is `"submit"` which is often not the desired behavior.

### Invalid:

```tsx
<button />
<button type="foo" />
<button type={condition ? "foo" : "bar"} />
<button type={foo} />
<button type={2} />
```

### Valid:

```tsx
<button type="submit" />
<button type="button" />
<button type="reset" />
<button type={condition ? "button" : "submit"} />
```
