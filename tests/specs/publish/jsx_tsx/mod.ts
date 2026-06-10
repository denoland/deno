import fooTsx from "./foo.tsx";
import fooJsx from "./foo.jsx";

export function renderTsxJsx() {
  console.log(fooTsx());
  console.log(fooJsx());
}
