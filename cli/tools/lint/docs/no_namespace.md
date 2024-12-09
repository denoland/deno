Disallows the use of `namespace` and `module` keywords in TypeScript code.

`namespace` and `module` are both thought of as outdated keywords to organize
the code. Instead, it is generally preferable to use ES2015 module syntax (e.g.
`import`/`export`).

However, this rule still allows the use of these keywords in the following two
cases:

- they are used for defining ["ambient" namespaces] along with `declare`
  keywords
- they are written in TypeScript's type definition files: `.d.ts`

["ambient" namespaces]: https://www.typescriptlang.org/docs/handbook/namespaces.html#ambient-namespaces

### Invalid:

```typescript
// foo.ts
module mod {}
namespace ns {}
```

```dts
// bar.d.ts
// all usage of `module` and `namespace` keywords are allowed in `.d.ts`
```

### Valid:

```typescript
// foo.ts
declare global {}
declare module mod1 {}
declare module "mod2" {}
declare namespace ns {}
```

```dts
// bar.d.ts
module mod1 {}
namespace ns1 {}
declare global {}
declare module mod2 {}
declare module "mod3" {}
declare namespace ns2 {}
```
