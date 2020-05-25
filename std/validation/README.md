# Validation

Utilities to help with validation

## assert

Make an assertion, throwing an AssertionError if the assertion is not valid.

```
const x=1;
const y=1;
const z=3;
assert(x===y); // passes
assert(x===z); // throws AssertionError
assert(x===z, "x and z should be equal"); // throws AssertionError with message
```
