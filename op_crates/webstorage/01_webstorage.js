((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;

  class Storage {
    constructor(session = false) {
      this.#session = session;

      return new Proxy(this, {
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

    #rid;
    #session;

    #getRid() {
      if (!this.#session) window.location;

      if (!this.#rid) {
        const data = core.jsonOpSync("op_localstorage_open", {
          session: this.#session,
        });
        this.#rid = data.rid;
      }
      return this.#rid;
    }

    get length() {
      return core.jsonOpSync("op_localstorage_length", {
        rid: this.#getRid(),
      });
    }

    key(index) {
      const prefix = "Failed to execute 'key' on 'Storage'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      index = webidl.converters["unsigned long"](index, {
        prefix,
        context: "Argument 1",
      });

      return core.jsonOpSync("op_localstorage_key", {
        rid: this.#getRid(),
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

      core.jsonOpSync("op_localstorage_set", {
        rid: this.#getRid(),
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

      return core.jsonOpSync("op_localstorage_get", {
        rid: this.#getRid(),
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

      core.jsonOpSync("op_localstorage_remove", {
        rid: this.#getRid(),
        keyName: key,
      });
    }

    clear() {
      core.jsonOpSync("op_localstorage_clear", {
        rid: this.#getRid(),
      });
    }
  }

  window.__bootstrap.webStorage = {
    localStorage: new Storage(false),
    sessionStorage: new Storage(true),
    Storage,
  };
})(this);
