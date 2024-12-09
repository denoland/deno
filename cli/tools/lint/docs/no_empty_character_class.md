Disallows using the empty character class in a regular expression

Regular expression character classes are a series of characters in brackets,
e.g. `[abc]`. if nothing is supplied in the brackets it will not match anything
which is likely a typo or mistake.

### Invalid:

```typescript
/^abc[]/.test("abcdefg"); // false, as `d` does not match an empty character class
"abcdefg".match(/^abc[]/); // null
```

### Valid:

```typescript
// Without a character class
/^abc/.test("abcdefg"); // true
"abcdefg".match(/^abc/); // ["abc"]

// With a valid character class
/^abc[a-z]/.test("abcdefg"); // true
"abcdefg".match(/^abc[a-z]/); // ["abcd"]
```
