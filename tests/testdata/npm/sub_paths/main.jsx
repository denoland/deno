import React from "npm:react@18.2.0";
import { renderToString } from "npm:react-dom@18.2.0/server";

function App({ name }) {
  return <div>Hello {name}!</div>;
}

console.log(renderToString(<App name="World" />));
