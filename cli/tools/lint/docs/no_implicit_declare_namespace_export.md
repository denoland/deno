Disallows the use of implicit exports in ["ambient" namespaces].

TypeScript implicitly export all members of an ["ambient" namespaces], except
whether a named export is present.

["ambient" namespaces]: https://www.typescriptlang.org/docs/handbook/namespaces.html#ambient-namespaces

### Invalid:

```ts
// foo.ts or foo.d.ts
declare namespace ns {
  interface ImplicitlyExported {}
  export type Exported = true;
}
```

### Valid:

```ts
// foo.ts or foo.d.ts
declare namespace ns {
  interface NonExported {}
  export {};
}

declare namespace ns {
  interface Exported {}
  export { Exported };
}

declare namespace ns {
  export interface Exported {}
}
```
