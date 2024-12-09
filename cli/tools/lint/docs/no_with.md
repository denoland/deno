Disallows the usage of `with` statements.

The `with` statement is discouraged as it may be the source of confusing bugs
and compatibility issues. For more details, see [with - JavaScript | MDN].

[with - JavaScript | MDN]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/with

### Invalid:

```typescript
with (someVar) {
  console.log("foo");
}
```
