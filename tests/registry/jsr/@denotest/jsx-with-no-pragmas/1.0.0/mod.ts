import renderJsx from "./foo.jsx";
import renderTsx from "./foo.tsx";

export function render() {
  console.log(renderJsx());
  console.log(renderTsx());
}
