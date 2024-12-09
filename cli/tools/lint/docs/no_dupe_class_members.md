Disallows using a class member function name more than once

Declaring a function of the same name twice in a class will cause the previous
declaration(s) to be overwritten, causing unexpected behaviors.

### Invalid:

```typescript
class Foo {
  bar() {}
  bar() {}
}
```

### Valid:

```typescript
class Foo {
  bar() {}
  fizz() {}
}
```
