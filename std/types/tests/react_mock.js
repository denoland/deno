// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const React = {
  createElement(type, props, ...children) {
    return JSON.stringify({ type, props, children });
  },
};

export default React;
