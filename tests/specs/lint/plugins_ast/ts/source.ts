let a: [number, foo: number] = [1, 2];

type A1 = boolean;
type A2 = any;
type A3 = unknown;
type A4 = object;
type A5 = bigint;
type A6 = symbol;
type A7 = void;
type A8 = undefined;
type A9 = null;
type A10 = never;
type A11 = intrinsic;

class Foo {
  foo(): this {
    return this;
  }
}

interface Foo1<T> {
  foo: T;
  bar?(a: number): void;
  foo2: (...arg: any[]) => void;
}
interface Foo2 {}
interface Foo3 extends Foo1<number>, Foo2 {}
type B<T> = Foo1<T>;

interface Animal {
  live(): void;
}
interface Dog extends Animal {
  woof(): void;
}

type Example1 = Dog extends Animal ? number : string;
type Both = Animal | Dog;
type NameOrId<T extends number | string> = T extends number ? IdLabel
  : NameLabel;
type MessageOf<T> = T extends { message: unknown } ? T["message"] : never;
type Flatten<T> = T extends any[] ? T[number] : T;
type Flatten<Type> = Type extends Array<infer Item> ? Item : Type;
type GetReturnType<Type> = Type extends (...args: never[]) => infer Return
  ? Return
  : never;

type Num = GetReturnType<() => number>;

interface Lit<T extends string> {
  foo: "foo";
  b: true;
  b2: false;
  n: 1;
  r: `foo${T}`;
}

type U = A | B & C;

type CreateMutable<Type> = {
  -readonly [Property in keyof Type]: Type[Property];
};
