(function() {
  const INDEX_LEN = 0;
  const NUM_RECORDS = 128;
  const RECORD_SIZE = 4;
  const shared32 = new Int32Array(libdeno.shared);
  const global = this;

  if (!global["Deno"]) {
    global["Deno"] = {};
  }

  function idx(i, off) {
    return 1 + i * RECORD_SIZE + off;
  }

  Deno.sharedSimple = {
    push: (promiseId, opId, arg, result) => {
      if (shared32[INDEX_LEN] >= NUM_RECORDS) {
        return false;
      }
      const i = shared32[INDEX_LEN]++;
      shared32[idx(i, 0)] = promiseId;
      shared32[idx(i, 1)] = opId;
      shared32[idx(i, 2)] = arg;
      shared32[idx(i, 3)] = result;
      return true;
    },

    pop: () => {
      if (shared32[INDEX_LEN] == 0) {
        return null;
      }
      const i = --shared32[INDEX_LEN];
      return {
        promiseId: shared32[idx(i, 0)],
        opId: shared32[idx(i, 1)],
        arg: shared32[idx(i, 2)],
        result: shared32[idx(i, 3)]
      };
    },

    reset: () => {
      shared32[INDEX_LEN] = 0;
    },

    size: () => {
      return shared32[INDEX_LEN];
    }
  };
})();
