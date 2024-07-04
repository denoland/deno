// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file

/*
 * @module mod
 * @description
 * This module provides a `display()` function for the Jupyter Deno Kernel, similar to IPython's display.
 * It can be used to asynchronously display objects in Jupyter frontends. There are also tagged template functions
 * for quickly creating HTML, Markdown, and SVG views.
 *
 * @example
 * Displaying objects asynchronously in Jupyter frontends.
 * ```typescript
 * import { display, html, md } from "https://deno.land/x/deno_jupyter/mod.ts";
 *
 * await display(html`<h1>Hello, world!</h1>`);
 * await display(md`# Notebooks in TypeScript via Deno ![Deno logo](https://github.com/denoland.png?size=32)
 *
 * * TypeScript ${Deno.version.typescript}
 * * V8 ${Deno.version.v8}
 * * Deno ${Deno.version.deno}
 *
 * Interactive compute with Jupyter _built into Deno_!
 * `);
 * ```
 *
 * @example
 * Emitting raw MIME bundles.
 * ```typescript
 * import { display } from "https://deno.land/x/deno_jupyter/mod.ts";
 *
 * await display({
 *  "text/plain": "Hello, world!",
 *  "text/html": "<h1>Hello, world!</h1>",
 *  "text/markdown": "# Hello, world!",
 * }, { raw: true });
 * ```
 */
import { core, internals } from "ext:core/mod.js";

const $display = Symbol.for("Jupyter.display");

/** Escape copied from https://jsr.io/@std/html/0.221.0/entities.ts */
const rawToEntityEntries = [
  ["&", "&amp;"],
  ["<", "&lt;"],
  [">", "&gt;"],
  ['"', "&quot;"],
  ["'", "&#39;"],
];

const rawToEntity = new Map(rawToEntityEntries);

const rawRe = new RegExp(`[${[...rawToEntity.keys()].join("")}]`, "g");

function escapeHTML(str) {
  return str.replaceAll(
    rawRe,
    (m) => rawToEntity.has(m) ? rawToEntity.get(m) : m,
  );
}

/** Duck typing our way to common visualization and tabular libraries */
/** Vegalite */
function isVegaLike(obj) {
  return obj !== null && typeof obj === "object" && "toSpec" in obj;
}
function extractVega(obj) {
  const spec = obj.toSpec();
  if (!("$schema" in spec)) {
    return null;
  }
  if (typeof spec !== "object") {
    return null;
  }
  let mediaType = "application/vnd.vega.v5+json";
  if (spec.$schema === "https://vega.github.io/schema/vega-lite/v4.json") {
    mediaType = "application/vnd.vegalite.v4+json";
  } else if (
    spec.$schema === "https://vega.github.io/schema/vega-lite/v5.json"
  ) {
    mediaType = "application/vnd.vegalite.v5+json";
  }
  return {
    [mediaType]: spec,
  };
}
/** Polars */
function isDataFrameLike(obj) {
  const isObject = obj !== null && typeof obj === "object";
  if (!isObject) {
    return false;
  }
  const df = obj;
  return (
    df.schema !== void 0 &&
    typeof df.schema === "object" &&
    df.head !== void 0 &&
    typeof df.head === "function" &&
    df.toRecords !== void 0 &&
    typeof df.toRecords === "function"
  );
}
/**
 * Map Polars DataType to JSON Schema data types.
 * @param dataType - The Polars DataType.
 * @returns The corresponding JSON Schema data type.
 */
function mapPolarsTypeToJSONSchema(colType) {
  const typeMapping = {
    Null: "null",
    Bool: "boolean",
    Int8: "integer",
    Int16: "integer",
    Int32: "integer",
    Int64: "integer",
    UInt8: "integer",
    UInt16: "integer",
    UInt32: "integer",
    UInt64: "integer",
    Float32: "number",
    Float64: "number",
    Date: "string",
    Datetime: "string",
    Utf8: "string",
    Categorical: "string",
    List: "array",
    Struct: "object",
  };
  // These colTypes are weird. When you console.dir or console.log them
  // they show a `DataType` field, however you can't access it directly until you
  // convert it to JSON
  const dataType = colType.toJSON()["DataType"];
  return typeMapping[dataType] || "string";
}

function extractDataFrame(df) {
  const fields = [];
  const schema = {
    fields,
  };
  let data = [];
  // Convert DataFrame schema to Tabular DataResource schema
  for (const [colName, colType] of Object.entries(df.schema)) {
    const dataType = mapPolarsTypeToJSONSchema(colType);
    schema.fields.push({
      name: colName,
      type: dataType,
    });
  }
  // Convert DataFrame data to row-oriented JSON
  //
  // TODO(rgbkrk): Determine how to get the polars format max rows
  //       Since pl.setTblRows just sets env var POLARS_FMT_MAX_ROWS,
  //       we probably just have to pick a number for now.
  //

  data = df.head(50).toRecords();
  let htmlTable = "<table>";
  htmlTable += "<thead><tr>";
  schema.fields.forEach((field) => {
    htmlTable += `<th>${escapeHTML(String(field.name))}</th>`;
  });
  htmlTable += "</tr></thead>";
  htmlTable += "<tbody>";
  df.head(10)
    .toRecords()
    .forEach((row) => {
      htmlTable += "<tr>";
      schema.fields.forEach((field) => {
        htmlTable += `<td>${escapeHTML(String(row[field.name]))}</td>`;
      });
      htmlTable += "</tr>";
    });
  htmlTable += "</tbody></table>";
  return {
    "application/vnd.dataresource+json": { data, schema },
    "text/html": htmlTable,
  };
}

/** Canvas */
function isCanvasLike(obj) {
  return obj !== null && typeof obj === "object" && "toDataURL" in obj;
}

/** Possible HTML and SVG Elements */
function isSVGElementLike(obj) {
  return (
    obj !== null &&
    typeof obj === "object" &&
    "outerHTML" in obj &&
    typeof obj.outerHTML === "string" &&
    obj.outerHTML.startsWith("<svg")
  );
}

function isHTMLElementLike(obj) {
  return (
    obj !== null &&
    typeof obj === "object" &&
    "outerHTML" in obj &&
    typeof obj.outerHTML === "string"
  );
}

/** Check to see if an object already contains a `Symbol.for("Jupyter.display") */
function hasDisplaySymbol(obj) {
  return (
    obj !== null &&
    typeof obj === "object" &&
    $display in obj &&
    typeof obj[$display] === "function"
  );
}

function makeDisplayable(obj) {
  return {
    [$display]: () => obj,
  };
}

/**
 * Format an object for displaying in Deno
 *
 * @param obj - The object to be displayed
 * @returns MediaBundle
 */
async function format(obj) {
  if (hasDisplaySymbol(obj)) {
    return await obj[$display]();
  }
  if (typeof obj !== "object") {
    return {
      "text/plain": Deno[Deno.internal].inspectArgs(["%o", obj], {
        colors: !Deno.noColor,
      }),
    };
  }

  if (isCanvasLike(obj)) {
    const dataURL = obj.toDataURL();
    const parts = dataURL.split(",");
    const mime = parts[0].split(":")[1].split(";")[0];
    const data = parts[1];
    return {
      [mime]: data,
    };
  }
  if (isVegaLike(obj)) {
    return extractVega(obj);
  }
  if (isDataFrameLike(obj)) {
    return extractDataFrame(obj);
  }
  if (isSVGElementLike(obj)) {
    return {
      "image/svg+xml": obj.outerHTML,
    };
  }
  if (isHTMLElementLike(obj)) {
    return {
      "text/html": obj.outerHTML,
    };
  }
  return {
    "text/plain": Deno[Deno.internal].inspectArgs(["%o", obj], {
      colors: !Deno.noColor,
    }),
  };
}

/**
 * This function creates a tagged template function for a given media type.
 * The tagged template function takes a template string and returns a displayable object.
 *
 * @param mediatype - The media type for the tagged template function.
 * @returns A function that takes a template string and returns a displayable object.
 */
function createTaggedTemplateDisplayable(mediatype) {
  return (strings, ...values) => {
    const payload = strings.reduce(
      (acc, string, i) =>
        acc + string + (values[i] !== undefined ? values[i] : ""),
      "",
    );
    return makeDisplayable({ [mediatype]: payload });
  };
}

/**
 * Show Markdown in Jupyter frontends with a tagged template function.
 *
 * Takes a template string and returns a displayable object for Jupyter frontends.
 *
 * @example
 * Create a Markdown view.
 *
 * ```typescript
 * md`# Notebooks in TypeScript via Deno ![Deno logo](https://github.com/denoland.png?size=32)
 *
 * * TypeScript ${Deno.version.typescript}
 * * V8 ${Deno.version.v8}
 * * Deno ${Deno.version.deno}
 *
 * Interactive compute with Jupyter _built into Deno_!
 * `
 * ```
 */
const md = createTaggedTemplateDisplayable("text/markdown");

/**
 * Show HTML in Jupyter frontends with a tagged template function.
 *
 * Takes a template string and returns a displayable object for Jupyter frontends.
 *
 * @example
 * Create an HTML view.
 * ```typescript
 * html`<h1>Hello, world!</h1>`
 * ```
 */
const html = createTaggedTemplateDisplayable("text/html");
/**
 * SVG Tagged Template Function.
 *
 * Takes a template string and returns a displayable object for Jupyter frontends.
 *
 * Example usage:
 *
 * svg`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
 *      <circle cx="50" cy="50" r="40" stroke="green" stroke-width="4" fill="yellow" />
 *    </svg>`
 */
const svg = createTaggedTemplateDisplayable("image/svg+xml");

function isMediaBundle(obj) {
  if (obj == null || typeof obj !== "object" || Array.isArray(obj)) {
    return false;
  }
  for (const key in obj) {
    if (typeof key !== "string") {
      return false;
    }
  }
  return true;
}

async function formatInner(obj, raw) {
  if (raw && isMediaBundle(obj)) {
    return obj;
  } else {
    return await format(obj);
  }
}

internals.jupyter = { formatInner };

/**
class CommMessage(TypedDict):
    header: dict
    # typically UUID, must be unique per message
    msg_id: str
    msg_type: str
    parent_header: dict
    metadata: dict
    content: <custom payload>
    buffers: list[memoryview]

((async) => {
  const data = await Deno.jupyter.comms.recv("1234-5678");
})();
((async) => {
  const data = await Deno.jupyter.comms.recv("1234-5678");
})();

const comm = await Deno.jupyter.comms.open("1234-5678");
const data = await comm.recv();

const data = await Deno.jupyter.comms.recv("1234-5678");

c = Comm("1234-5678")

c.on("update", data => {
    console.log(data);
    Deno.jupyter.broadcast(...);
});


{
    msg_type: "comm_msg",
    content: {
        comm_id: "1234-5678",
        data: {

        }
    }
}
*/

function enableJupyter() {
  const {
    op_jupyter_broadcast,
    op_jupyter_input,
    op_jupyter_comm_recv,
    op_jupyter_comm_open,
  } = core.ops;

  function input(
    prompt,
    password,
  ) {
    return op_jupyter_input(prompt, password);
  }

  function comm(commId, targetName, msgCallback) {
    op_jupyter_comm_open(commId, targetName);

    let closed = false;

    // TODO(bartlomieju): return something, so we can close this comm.
    (async () => {
      while (true) {
        const [data, buffers] = await op_jupyter_comm_recv(commId);

        if (!data) {
          closed = true;
          break;
        }

        msgCallback?.({
          ...data,
          buffers,
        });
      }
    })();

    return {
      close() {
        if (closed) {
          return;
        }

        closed = true;
      },
      send(data, buffers = []) {
        return broadcast("comm_msg", {
          comm_id: commId,
          data: data,
        }, { buffers });
      },
    };
  }

  async function broadcast(
    msgType,
    content,
    { metadata = { __proto__: null }, buffers = [] } = { __proto__: null },
  ) {
    await op_jupyter_broadcast(msgType, content, metadata, buffers);
  }

  async function broadcastResult(executionCount, result) {
    try {
      if (result === undefined) {
        return;
      }

      const data = await format(result);
      await broadcast("execute_result", {
        execution_count: executionCount,
        data,
        metadata: {},
      });
    } catch (err) {
      if (err instanceof Error) {
        const stack = err.stack || "";
        await broadcast("error", {
          ename: err.name,
          evalue: err.message,
          traceback: stack.split("\n"),
        });
      } else if (typeof err == "string") {
        await broadcast("error", {
          ename: "Error",
          evalue: err,
          traceback: [],
        });
      } else {
        await broadcast("error", {
          ename: "Error",
          evalue:
            "An error occurred while formatting a result, but it could not be identified",
          traceback: [],
        });
      }
    }
  }

  internals.jupyter.broadcastResult = broadcastResult;

  /**
   * Display function for Jupyter Deno Kernel.
   * Mimics the behavior of IPython's `display(obj, raw=True)` function to allow
   * asynchronous displaying of objects in Jupyter.
   *
   * @param obj - The object to be displayed
   * @param options - Display options
   */
  async function display(obj, options = { raw: false, update: false }) {
    const bundle = await formatInner(obj, options.raw);
    let messageType = "display_data";
    if (options.update) {
      messageType = "update_display_data";
    }
    let transient = { __proto__: null };
    if (options.display_id) {
      transient = { display_id: options.display_id };
    }
    await broadcast(messageType, {
      data: bundle,
      metadata: {},
      transient,
    });
    return;
  }

  /**
   * Prompt for user confirmation (in Jupyter Notebook context)
   * Override confirm and prompt because they depend on a tty
   * and in the Deno.jupyter environment that doesn't exist.
   * @param {string} message - The message to display.
   * @returns {Promise<boolean>} User confirmation.
   */
  function confirm(message = "Confirm") {
    const answer = input(`${message} [y/N] `, false);
    return answer === "Y" || answer === "y";
  }

  /**
   * Prompt for user input (in Jupyter Notebook context)
   * @param {string} message - The message to display.
   * @param {string} defaultValue - The value used if none is provided.
   * @param {object} options Options
   * @param {boolean} options.password Hide the output characters
   * @returns {Promise<string>} The user input.
   */
  function prompt(
    message = "Prompt",
    defaultValue = "",
    { password = false } = {},
  ) {
    if (defaultValue != "") {
      message += ` [${defaultValue}]`;
    }
    const answer = input(`${message}`, password);

    if (answer === "") {
      return defaultValue;
    }

    return answer;
  }

  globalThis.confirm = confirm;
  globalThis.prompt = prompt;
  globalThis.Deno.jupyter = {
    broadcast,
    comm,
    display,
    format,
    md,
    html,
    svg,
    $display,
  };
}

internals.enableJupyter = enableJupyter;
