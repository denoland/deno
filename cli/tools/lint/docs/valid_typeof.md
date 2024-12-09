Restricts the use of the `typeof` operator to a specific set of string literals.

When used with a value the `typeof` operator returns one of the following
strings:

- `"undefined"`
- `"object"`
- `"boolean"`
- `"number"`
- `"string"`
- `"function"`
- `"symbol"`
- `"bigint"`

This rule disallows comparison with anything other than one of these string
literals when using the `typeof` operator, as this likely represents a typing
mistake in the string. The rule also disallows comparing the result of a
`typeof` operation with any non-string literal value, such as `undefined`, which
can represent an inadvertent use of a keyword instead of a string. This includes
comparing against string variables even if they contain one of the above values
as this cannot be guaranteed. An exception to this is comparing the results of
two `typeof` operations as these are both guaranteed to return on of the above
strings.

### Invalid:

```typescript
// typo
typeof foo === "strnig";
typeof foo == "undefimed";
typeof bar != "nunber";
typeof bar !== "fucntion";

// compare with non-string literals
typeof foo === undefined;
typeof bar == Object;
typeof baz === anotherVariable;
typeof foo == 5;
```

### Valid:

```typescript
typeof foo === "undefined";
typeof bar == "object";
typeof baz === "string";
typeof bar === typeof qux;
```
