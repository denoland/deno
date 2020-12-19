const React = {
  createElement(factory, props, ...children) {
    return { factory, props, children };
  },
};
const View = () => (
  <div class="deno">land</div>
);
console.log(<View />);
