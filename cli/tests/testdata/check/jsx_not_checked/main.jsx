// should not error about jsx-runtime not being found in types here
/** @jsxImportSource npm:react@18.2.0 */

import "./other.ts";

export default (
  <>
    <h1>Hello world</h1>
    <p>This is a JSX page</p>
  </>
);

/**
 * @param {number} a
 * @param {number} b
 */
function add(a, b) {
  return a + b;
}

console.log(add("1", "2"));
