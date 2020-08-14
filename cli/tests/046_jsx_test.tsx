declare namespace JSX {
  interface IntrinsicElements {
    [elemName: string]: any;
  }
}
const React = {
  createElement(factory: any, props: any, ...children: any[]) {
    return { factory, props, children };
  },
};
const View = () => (
  <div class="deno">land</div>
);
console.log(<View />);
