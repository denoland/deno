((window) => {
  const { sendSync, sendAsync } = window.__bootstrap.dispatchJson;

  async function eventLoop(eventRid, rid) {
    const {key, newValue, oldValue} = await sendAsync("op_localstorage_events_poll", {
      eventRid,
      rid,
    });
    if (key !== undefined) {
      const event = new StorageEvent("storage", {
        key,
        newValue,
        oldValue,
        storageArea: localStorage,
      });
      window.dispatchEvent("storage", event);
      window.onstorage?.(event);
    }
    eventLoop(eventRid, rid);
  }

  function webStorage(session = false) {
    let rid;

    function getRid() {
      if (!rid) {
        const data = sendSync("op_localstorage_open", {
          session,
          location: "foobar",
        });
        rid = data.rid;

        if (!session) {
          eventLoop({
            eventRid: data.eventRid,
            rid: data.rid,
          });
        }
      }
      return rid;
    }

    const storage = {
      get length() {
        return sendSync("op_localstorage_length", {
          rid: getRid(),
        });
      },
      key(index) {
        return sendSync("op_localstorage_key", {
          rid: getRid(),
          index,
        });
      },
      setItem(keyName, keyValue) {
        sendSync("op_localstorage_set", {
          rid: getRid(),
          keyName,
          keyValue,
        });
      },
      getItem(keyName) {
        return sendSync("op_localstorage_get", {
          rid: getRid(),
          keyName,
        });
      },
      removeItem(keyName) {
        sendSync("op_localstorage_remove", {
          rid: getRid(),
          keyName,
        });
      },
      clear() {
        sendSync("op_localstorage_clear", {
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
