const a = <div style={{}} />;
const b = <div style="foo" />;
const c: Foo<number> = {
  node: 2,
};

interface Foo<T> {
  node: T;
}
