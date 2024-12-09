Ensure that void elements in HTML don't have any children as that is not valid
HTML. See
[`Void element` article on MDN](https://developer.mozilla.org/en-US/docs/Glossary/Void_element)
for more information.

### Invalid:

```tsx
<br>foo</br>
<img src="a.jpg">foo</img>
```

### Valid:

```tsx
<br />
<img src="a.jpg" />
```
