// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This is a specialised implementation of a System module loader.

// eslint-disable-next-line @typescript-eslint/no-unused-vars
let System;
let __inst;

(() => {
  const mMap = new Map();
  System = {
    register(id, deps, f) {
      mMap.set(id, {
        id,
        deps,
        f,
        exp: {}
      });
    }
  };

  const gC = (data, main) => {
    const { id } = data;
    return {
      id,
      import: async id => mMap.get(id)?.exp,
      meta: { url: id, main }
    };
  };

  const gE = data => {
    const { exp } = data;
    return (id, value) => {
      const values = typeof id === "string" ? { [id]: value } : id;
      for (const [id, value] of Object.entries(values)) {
        Object.defineProperty(exp, id, {
          value,
          writable: true,
          enumerable: true
        });
      }
    };
  };

  const iQ = [];

  const enq = ids => {
    for (const id of ids) {
      if (!iQ.includes(id)) {
        const { deps } = mMap.get(id);
        iQ.push(id);
        enq(deps);
      }
    }
  };

  const dr = async main => {
    const rQ = [];
    let id;
    while ((id = iQ.pop())) {
      const m = mMap.get(id);
      const { f } = m;
      if (!f) {
        return;
      }
      rQ.push([m.deps, f(gE(m), gC(m, id === main))]);
      m.f = undefined;
    }
    let r;
    while ((r = rQ.shift())) {
      const [deps, { execute, setters }] = r;
      for (let i = 0; i < deps.length; i++) setters[i](mMap.get(deps[i])?.exp);
      const e = execute();
      if (e) await e;
    }
  };

  __inst = async id => {
    System = undefined;
    __inst = undefined;
    enq([id]);
    await dr(id);
    return mMap.get(id)?.exp;
  };
})();
