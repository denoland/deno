Disallows the implicit fallthrough of case statements

Case statements without a `break` will execute their body and then fallthrough
to the next case or default block and execute this block as well. While this is
sometimes intentional, many times the developer has forgotten to add a break
statement, intending only for a single case statement to be executed. This rule
enforces that you either end each case statement with a break statement or an
explicit comment that fallthrough was intentional. The fallthrough comment must
contain one of `fallthrough`, `falls through` or `fall through`.

### Invalid:

```typescript
switch (myVar) {
  case 1:
    console.log("1");

  case 2:
    console.log("2");
}
// If myVar = 1, outputs both `1` and `2`.  Was this intentional?
```

### Valid:

```typescript
switch (myVar) {
  case 1:
    console.log("1");
    break;

  case 2:
    console.log("2");
    break;
}
// If myVar = 1, outputs only `1`

switch (myVar) {
  case 1:
    console.log("1");
    /* falls through */
  case 2:
    console.log("2");
}
// If myVar = 1, intentionally outputs both `1` and `2`
```
