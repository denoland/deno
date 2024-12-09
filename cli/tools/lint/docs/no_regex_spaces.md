Disallows multiple spaces in regular expression literals.

Multiple spaces in regular expression literals are generally hard to read when
the regex gets complicated. Instead, it's better to use only one space character
and specify how many times spaces should appear with the `{n}` syntax, for
example:

```typescript
// Multiple spaces in the regex literal are harder to understand how many
// spaces are expected to be matched
const re = /foo   bar/;

// Instead use `{n}` syntax for readability
const re = /foo {3}var/;
```

### Invalid:

```typescript
const re1 = /  /;
const re2 = /foo  bar/;
const re3 = / a b  c d /;
const re4 = /foo  {3}bar/;

const re5 = new RegExp("  ");
const re6 = new RegExp("foo  bar");
const re7 = new RegExp(" a b  c d ");
const re8 = new RegExp("foo  {3}bar");
```

### Valid:

```typescript
const re1 = /foo/;
const re2 = / /;
const re3 = / {3}/;
const re4 = / +/;
const re5 = / ?/;
const re6 = / */;

const re7 = new RegExp("foo");
const re8 = new RegExp(" ");
const re9 = new RegExp(" {3}");
const re10 = new RegExp(" +");
const re11 = new RegExp(" ?");
const re12 = new RegExp(" *");
```
