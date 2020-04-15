System.register(
  "$deno$/web/form_data.ts",
  [
    "$deno$/web/blob.ts",
    "$deno$/web/dom_file.ts",
    "$deno$/web/dom_iterable.ts",
    "$deno$/web/util.ts",
  ],
  function (exports_91, context_91) {
    "use strict";
    let _a,
      blob,
      domFile,
      dom_iterable_ts_1,
      util_ts_15,
      dataSymbol,
      FormDataBase,
      FormDataImpl;
    const __moduleName = context_91 && context_91.id;
    return {
      setters: [
        function (blob_2) {
          blob = blob_2;
        },
        function (domFile_1) {
          domFile = domFile_1;
        },
        function (dom_iterable_ts_1_1) {
          dom_iterable_ts_1 = dom_iterable_ts_1_1;
        },
        function (util_ts_15_1) {
          util_ts_15 = util_ts_15_1;
        },
      ],
      execute: function () {
        dataSymbol = Symbol("data");
        FormDataBase = class FormDataBase {
          constructor() {
            this[_a] = [];
          }
          append(name, value, filename) {
            util_ts_15.requiredArguments(
              "FormData.append",
              arguments.length,
              2
            );
            name = String(name);
            if (value instanceof domFile.DomFileImpl) {
              this[dataSymbol].push([name, value]);
            } else if (value instanceof blob.DenoBlob) {
              const dfile = new domFile.DomFileImpl([value], filename || name, {
                type: value.type,
              });
              this[dataSymbol].push([name, dfile]);
            } else {
              this[dataSymbol].push([name, String(value)]);
            }
          }
          delete(name) {
            util_ts_15.requiredArguments(
              "FormData.delete",
              arguments.length,
              1
            );
            name = String(name);
            let i = 0;
            while (i < this[dataSymbol].length) {
              if (this[dataSymbol][i][0] === name) {
                this[dataSymbol].splice(i, 1);
              } else {
                i++;
              }
            }
          }
          getAll(name) {
            util_ts_15.requiredArguments(
              "FormData.getAll",
              arguments.length,
              1
            );
            name = String(name);
            const values = [];
            for (const entry of this[dataSymbol]) {
              if (entry[0] === name) {
                values.push(entry[1]);
              }
            }
            return values;
          }
          get(name) {
            util_ts_15.requiredArguments("FormData.get", arguments.length, 1);
            name = String(name);
            for (const entry of this[dataSymbol]) {
              if (entry[0] === name) {
                return entry[1];
              }
            }
            return null;
          }
          has(name) {
            util_ts_15.requiredArguments("FormData.has", arguments.length, 1);
            name = String(name);
            return this[dataSymbol].some((entry) => entry[0] === name);
          }
          set(name, value, filename) {
            util_ts_15.requiredArguments("FormData.set", arguments.length, 2);
            name = String(name);
            // If there are any entries in the context object’s entry list whose name
            // is name, replace the first such entry with entry and remove the others
            let found = false;
            let i = 0;
            while (i < this[dataSymbol].length) {
              if (this[dataSymbol][i][0] === name) {
                if (!found) {
                  if (value instanceof domFile.DomFileImpl) {
                    this[dataSymbol][i][1] = value;
                  } else if (value instanceof blob.DenoBlob) {
                    const dfile = new domFile.DomFileImpl(
                      [value],
                      filename || name,
                      {
                        type: value.type,
                      }
                    );
                    this[dataSymbol][i][1] = dfile;
                  } else {
                    this[dataSymbol][i][1] = String(value);
                  }
                  found = true;
                } else {
                  this[dataSymbol].splice(i, 1);
                  continue;
                }
              }
              i++;
            }
            // Otherwise, append entry to the context object’s entry list.
            if (!found) {
              if (value instanceof domFile.DomFileImpl) {
                this[dataSymbol].push([name, value]);
              } else if (value instanceof blob.DenoBlob) {
                const dfile = new domFile.DomFileImpl(
                  [value],
                  filename || name,
                  {
                    type: value.type,
                  }
                );
                this[dataSymbol].push([name, dfile]);
              } else {
                this[dataSymbol].push([name, String(value)]);
              }
            }
          }
          get [((_a = dataSymbol), Symbol.toStringTag)]() {
            return "FormData";
          }
        };
        FormDataImpl = class FormDataImpl extends dom_iterable_ts_1.DomIterableMixin(
          FormDataBase,
          dataSymbol
        ) {};
        exports_91("FormDataImpl", FormDataImpl);
      },
    };
  }
);
