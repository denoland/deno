Recommends the use of `namespace` keyword over `module` keyword when declaring
TypeScript module.

TypeScript supports the `module` keyword for organizing code, but this wording
can lead to a confusion with the ECMAScript's module. Since TypeScript v1.5, it
has provided us with the alternative keyword `namespace`, encouraging us to
always use `namespace` instead whenever we write TypeScript these days. See
[TypeScript v1.5 release note](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-1-5.html#namespace-keyword)
for more details.

### Invalid:

```typescript
module modA {}

declare module modB {}
```

### Valid:

```typescript
namespace modA {}

// "ambient modules" are allowed
// https://www.typescriptlang.org/docs/handbook/modules.html#ambient-modules
declare module "modB";
declare module "modC" {}
```
