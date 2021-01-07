((window) => {
  const core = window.Deno.core;

  function webStorage(session = false) {
    let rid;

    function getRid() {
      if (!rid) {
        const data = core.jsonOpSync("op_localstorage_open", {
          session,
          location: "foobar",
        });
        rid = data.rid;
      }
      return rid;
    }

    const storage = {
      get length() {
        return core.jsonOpSync("op_localstorage_length", {
          rid: getRid(),
        });
      },
      key(index) {
        return core.jsonOpSync("op_localstorage_key", {
          rid: getRid(),
          index: Number(index),
        });
      },
      setItem(keyName, keyValue) {
        core.jsonOpSync("op_localstorage_set", {
          rid: getRid(),
          keyName: String(keyName),
          keyValue: String(keyValue),
        });
      },
      getItem(keyName) {
        return core.jsonOpSync("op_localstorage_get", {
          rid: getRid(),
          keyName: String(keyName),
        });
      },
      removeItem(keyName) {
        core.jsonOpSync("op_localstorage_remove", {
          rid: getRid(),
          keyName: String(keyName),
        });
      },
      clear() {
        core.jsonOpSync("op_localstorage_clear", {
          rid: getRid(),
        });
      },
    };

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

  const localStorage = webStorage();
  const sessionStorage = webStorage(true);

  window.__bootstrap.webStorage = {
    localStorage,
    sessionStorage,
  };
})(this);
