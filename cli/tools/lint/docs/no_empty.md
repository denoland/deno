Disallows the use of empty block statements.

Empty block statements are legal but often represent that something was missed
and can make code less readable. This rule ignores block statements that only
contain comments. This rule also ignores empty constructors and function bodies
(including arrow functions).

### Invalid:

```typescript
if (foo) {}

while (foo) {}

switch (foo) {}

try {
  doSomething();
} catch (e) {
} finally {
}
```

### Valid:

```typescript
if (foo) {
  // empty
}

while (foo) {
  /* empty */
}

try {
  doSomething();
} catch (e) {
  // continue regardless of error
}

try {
  doSomething();
} finally {
  /* continue regardless of error */
}
```
