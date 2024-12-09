Requires overload signatures to be adjacent to each other.

Overloaded signatures which are not next to each other can lead to code which is
hard to read and maintain.

### Invalid:

(`bar` is declared in-between `foo` overloads)

```typescript
type FooType = {
  foo(s: string): void;
  foo(n: number): void;
  bar(): void;
  foo(sn: string | number): void;
};
```

```typescript
interface FooInterface {
  foo(s: string): void;
  foo(n: number): void;
  bar(): void;
  foo(sn: string | number): void;
}
```

```typescript
class FooClass {
  foo(s: string): void;
  foo(n: number): void;
  bar(): void {}
  foo(sn: string | number): void {}
}
```

```typescript
export function foo(s: string): void;
export function foo(n: number): void;
export function bar(): void {}
export function foo(sn: string | number): void {}
```

### Valid:

(`bar` is declared after `foo`)

```typescript
type FooType = {
  foo(s: string): void;
  foo(n: number): void;
  foo(sn: string | number): void;
  bar(): void;
};
```

```typescript
interface FooInterface {
  foo(s: string): void;
  foo(n: number): void;
  foo(sn: string | number): void;
  bar(): void;
}
```

```typescript
class FooClass {
  foo(s: string): void;
  foo(n: number): void;
  foo(sn: string | number): void {}
  bar(): void {}
}
```

```typescript
export function foo(s: string): void;
export function foo(n: number): void;
export function foo(sn: string | number): void {}
export function bar(): void {}
```
