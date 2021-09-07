interface FooOptions {
  a: boolean;
  b: boolean;
  c: boolean;
}

export function foo(options: FooOptions) {
  console.log("foo", options);
}

interface BarOptions {
  a: boolean;
  b: boolean;
  c: boolean;
}

export function bar(options: BarOptions) {
  console.log("bar", options);
}

bar({
  a: true,
  b: true,
  c: true,
});

interface BazOptions {
  a: boolean;
  b: boolean;
  c: boolean;
}

export function baz(options: BazOptions) {
  console.log("baz", options);
}
