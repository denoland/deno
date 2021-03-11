((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;

  const _rid = Symbol("[[rid]]");

  class Storage {
    [_rid];

    constructor() {
      webidl.illegalConstructor();
    }

    get length() {
      return core.jsonOpSync("op_webstorage_length", {
        rid: this[_rid],
      });
    }

    key(index) {
      const prefix = "Failed to execute 'key' on 'Storage'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      index = webidl.converters["unsigned long"](index, {
        prefix,
        context: "Argument 1",
      });

      return core.jsonOpSync("op_webstorage_key", {
        rid: this[_rid],
        index,
      });
    }

    setItem(key, value) {
      const prefix = "Failed to execute 'setItem' on 'Storage'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      key = webidl.converters.DOMString(key, {
        prefix,
        context: "Argument 1",
      });
      value = webidl.converters.DOMString(value, {
        prefix,
        context: "Argument 2",
      });

      core.jsonOpSync("op_webstorage_set", {
        rid: this[_rid],
        keyName: key,
        keyValue: value,
      });
    }

    getItem(key) {
      const prefix = "Failed to execute 'getItem' on 'Storage'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      key = webidl.converters.DOMString(key, {
        prefix,
        context: "Argument 1",
      });

      return core.jsonOpSync("op_webstorage_get", {
        rid: this[_rid],
        keyName: key,
      });
    }

    removeItem(key) {
      const prefix = "Failed to execute 'removeItem' on 'Storage'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      key = webidl.converters.DOMString(key, {
        prefix,
        context: "Argument 1",
      });

      core.jsonOpSync("op_webstorage_remove", {
        rid: this[_rid],
        keyName: key,
      });
    }

    clear() {
      core.jsonOpSync("op_webstorage_clear", {
        rid: this[_rid],
      });
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          length: this.length,
        })
      }`;
    }
  }

  function createStorage(persistent) {
    if (persistent) window.location;

    const data = core.jsonOpSync("op_webstorage_open", {
      persistent,
    });

    const storage = webidl.createBranded(Storage);
    storage[_rid] = data.rid;

    return new Proxy(storage, {
      deleteProperty(target, prop) {
        target.removeItem(prop);
      },

      get(target, p) {
        if (p in target) {
          return Reflect.get(...arguments);
        } else {
          return target.getItem(p);
        }
      },

      set(target, p, value) {
        if (p in target) {
          return false;
        } else {
          target.setItem(p, value);

          return true;
        }
      },
    });
  }

  let localStorage;
  let sessionStorage;

  window.__bootstrap.webStorage = {
    localStorage() {
      if (!localStorage) {
        localStorage = createStorage(true);
      }
      return localStorage;
    },
    sessionStorage() {
      if (!sessionStorage) {
        sessionStorage = createStorage(false);
      }
      return sessionStorage;
    },
    Storage,
  };
})(this);
