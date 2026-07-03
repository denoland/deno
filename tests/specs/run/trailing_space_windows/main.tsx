const React = {
  createElement: (tag: string, _props: unknown, ...children: string[]) =>
    `<${tag}>${children.join("")}</${tag}>`,
};
const el = <h1>Hello, TSX!</h1>;
console.log(el);
