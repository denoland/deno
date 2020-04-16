// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// @deno-types="../../../react/v16.13.1/react.d.ts"
import React from "https://cdn.pika.dev/@pika/react@v16.13.1";
// @deno-types="../server.d.ts"
import ReactDomServer from "https://dev.jspm.io/react-dom@16.13.1/server.js";
import { assertEquals } from "../../../../testing/asserts.ts";

class ClassComponent extends React.Component {
  render() {
    return (
      <h1>Testing class component</h1>
    );
  }
}

const FunctionalComponent = () => (
  <h1>Testing functional component</h1>
);

function NestedComponent (){
  return (
    <div>
      <span>Testing nested components</span>,
      <ClassComponent/>
      <FunctionalComponent/>
    </div>
  );
}

Deno.test({
  name: "ReactDomServer is typed to render",
  fn() {
    assertEquals(
      ReactDomServer.renderToString(<ClassComponent />),
      '<h1 data-reactroot="">Testing class component</h1>',
    );
    assertEquals(
      ReactDomServer.renderToString(<FunctionalComponent />),
      '<h1 data-reactroot="">Testing functional component</h1>',
    );
    assertEquals(
      ReactDomServer.renderToString(<NestedComponent />),
      '<div data-reactroot=""><span>Testing nested components</span>,<h1>Testing class component</h1><h1>Testing functional component</h1></div>',
    );
    assertEquals(
      ReactDomServer.renderToStaticMarkup(<ClassComponent />),
      '<h1>Testing class component</h1>',
    );
    assertEquals(
      ReactDomServer.renderToStaticMarkup(<FunctionalComponent />),
      '<h1>Testing functional component</h1>',
    );
    assertEquals(
      ReactDomServer.renderToStaticMarkup(<NestedComponent />),
      '<div><span>Testing nested components</span>,<h1>Testing class component</h1><h1>Testing functional component</h1></div>',
    );
  },
});
