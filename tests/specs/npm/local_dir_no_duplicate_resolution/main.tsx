// this previously was ending up with two preacts and would crash
import { useMemo } from "preact/hooks";
import renderToString from "preact-render-to-string";

function Test() {
  useMemo(() => "test", []);
  return <div>Test</div>;
}

const html = renderToString(<Test />);
