// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const blob = window.__bootstrap.blob;
  const domFile = window.__bootstrap.domFile;
  const { DomIterableMixin } = window.__bootstrap.domIterable;
  const { requiredArguments } = window.__bootstrap.webUtil;

  const dataSymbol = Symbol("data");

  function parseFormDataValue(value, filename) {
    if (value instanceof domFile.DomFile) {
      return new domFile.DomFile([value], filename || value.name, {
        type: value.type,
        lastModified: value.lastModified,
      });
    } else if (value instanceof blob.Blob) {
      return new domFile.DomFile([value], filename || "blob", {
        type: value.type,
      });
    } else {
      return String(value);
    }
  }

  class FormDataBase {
    [dataSymbol] = [];

    append(name, value, filename) {
      requiredArguments("FormData.append", arguments.length, 2);
      name = String(name);
      this[dataSymbol].push([name, parseFormDataValue(value, filename)]);
    }

    delete(name) {
      requiredArguments("FormData.delete", arguments.length, 1);
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
      requiredArguments("FormData.getAll", arguments.length, 1);
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
      requiredArguments("FormData.get", arguments.length, 1);
      name = String(name);
      for (const entry of this[dataSymbol]) {
        if (entry[0] === name) {
          return entry[1];
        }
      }

      return null;
    }

    has(name) {
      requiredArguments("FormData.has", arguments.length, 1);
      name = String(name);
      return this[dataSymbol].some((entry) => entry[0] === name);
    }

    set(name, value, filename) {
      requiredArguments("FormData.set", arguments.length, 2);
      name = String(name);

      // If there are any entries in the context object’s entry list whose name
      // is name, replace the first such entry with entry and remove the others
      let found = false;
      let i = 0;
      while (i < this[dataSymbol].length) {
        if (this[dataSymbol][i][0] === name) {
          if (!found) {
            this[dataSymbol][i][1] = parseFormDataValue(value, filename);
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
        this[dataSymbol].push([name, parseFormDataValue(value, filename)]);
      }
    }

    get [Symbol.toStringTag]() {
      return "FormData";
    }
  }

  class FormData extends DomIterableMixin(FormDataBase, dataSymbol) {}

  window.__bootstrap.formData = {
    FormData,
  };
})(this);
