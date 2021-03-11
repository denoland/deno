((window) => {
  const core = window.Deno.core;

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
      return core.jsonOpSync("op_localstorage_key", {
        rid: this.#getRid(),
        index: Number(index),
      });
    }

    setItem(keyName, keyValue) {
      core.jsonOpSync("op_localstorage_set", {
        rid: this.#getRid(),
        keyName: String(keyName),
        keyValue: String(keyValue),
      });
    }

    getItem(keyName) {
      return core.jsonOpSync("op_localstorage_get", {
        rid: this.#getRid(),
        keyName: String(keyName),
      });
    }

    removeItem(keyName) {
      core.jsonOpSync("op_localstorage_remove", {
        rid: this.#getRid(),
        keyName: String(keyName),
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
