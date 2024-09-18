// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/** @jsx h */
import results from "./deno.json" assert { type: "json" };
import nodeResults from "./node.json" assert { type: "json" };
import { h, ssr } from "https://crux.land/nanossr@0.0.4";
import { router } from "https://crux.land/router@0.0.11";

function once(fn) {
  let called = false;
  let result;
  return function () {
    if (!called) {
      called = true;
      result = fn();
      return result;
    }
    return result;
  };
}

const body = once(() =>
  Object.entries(results).map(([name, data]) => (
    <tr>
      <td class="border px-4 py-2">{name}</td>
      <td class="border px-4 py-2">
        {data.reduce((a, b) => a + b, 0) / data.length} ops/sec
      </td>
      <td class="border px-4 py-2">
        {nodeResults[name].reduce((a, b) => a + b, 0) /
          nodeResults[name].length} ops/sec
      </td>
    </tr>
  ))
);

function App() {
  return (
    <table class="table-auto">
      <thead>
        <tr>
          <th class="px-4 py-2">Benchmark</th>
          <th class="px-4 py-2">Deno</th>
          <th class="px-4 py-2">Node</th>
        </tr>
      </thead>
      <tbody>
        {body()}
      </tbody>
    </table>
  );
}

const { serve } = Deno;
serve(router({
  "/": () => ssr(() => <App />),
}));
