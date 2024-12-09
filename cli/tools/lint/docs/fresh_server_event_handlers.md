Disallows event handlers in fresh server components

Components inside the `routes/` folder in a fresh app are exclusively rendered
on the server. They are not rendered in the client and setting an event handler
will have no effect.

Note that this rule only applies to server components inside the `routes/`
folder, not to fresh islands or any other components.

### Invalid:

```jsx
<button onClick={() => {}} />
<button onclick={() => {}} />
<my-custom-element foo={() => {}} />
```

### Valid:

```jsx
<button />
<my-custom-element />
```
