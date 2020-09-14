((window) => {
  const { sendSync } = window.__bootstrap.dispatchJson;

  function webStorage(temporary = false) {
    let rid;

    function getRid() {
      if (!rid) {
        rid = sendSync("op_localstorage_open", {
          temporary
        });
      }
      return rid;
    }

    return {
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
      }
    };
  }

  const localStorage = webStorage();
  const sessionStorage = webStorage(true);

  window.__bootstrap.webStorage = {
    localStorage,
    sessionStorage,
  };
})(this);
