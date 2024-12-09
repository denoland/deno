Warns unused ignore directives

We sometimes have to suppress and ignore lint errors for some reasons and we can
do so using [ignore directives](https://lint.deno.land/ignoring-rules).

In some cases, however, like after refactoring, we may end up having ignore
directives that are no longer necessary. Such superfluous ignore directives are
likely to confuse future code readers, and to make matters worse, might hide
future lint errors unintentionally. To prevent such situations, this rule
detects unused, superfluous ignore directives.

### Invalid:

```typescript
// Actually this line is valid since `export` means "used",
// so this directive is superfluous
// deno-lint-ignore no-unused-vars
export const foo = 42;
```

### Valid:

```typescript
export const foo = 42;
```
