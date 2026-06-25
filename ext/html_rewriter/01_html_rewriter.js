// Copyright 2018-2026 the Deno authors. MIT license.

/// <reference path="../../core/internal.d.ts" />

(function () {
const { core, primordials } = __bootstrap;
const {
  op_html_rewriter_abort,
  op_html_rewriter_comment_text,
  op_html_rewriter_doctype_info,
  op_html_rewriter_element_attributes,
  op_html_rewriter_element_get_attribute,
  op_html_rewriter_element_has_attribute,
  op_html_rewriter_element_namespace_uri,
  op_html_rewriter_element_on_end_tag,
  op_html_rewriter_element_remove_attribute,
  op_html_rewriter_element_set_attribute,
  op_html_rewriter_element_set_tag_name,
  op_html_rewriter_element_tag_name,
  op_html_rewriter_end,
  op_html_rewriter_end_tag_name,
  op_html_rewriter_parse_selector,
  op_html_rewriter_pump,
  op_html_rewriter_pump_sync,
  op_html_rewriter_set_comment_text,
  op_html_rewriter_set_end_tag_name,
  op_html_rewriter_start,
  op_html_rewriter_text_info,
  op_html_rewriter_token_content,
  op_html_rewriter_token_done,
  op_html_rewriter_token_error,
  op_html_rewriter_token_remove,
  op_html_rewriter_token_removed,
  op_html_rewriter_write,
} = core.ops;
const {
  ArrayPrototypePush,
  FunctionPrototypeCall,
  ObjectPrototypeIsPrototypeOf,
  SafeArrayIterator,
  Symbol,
  SymbolFor,
  TypeError,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeSet,
  Uint8Array,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);
const { TransformStream } = core.loadExtScript("ext:deno_web/06_streams.js");
const { ResponsePrototype, fromInnerResponse, getInnerResponse } = core
  .loadExtScript("ext:deno_fetch/23_response.js");
const { InnerBody } = core.loadExtScript("ext:deno_fetch/22_body.js");

const CONTENT_BEFORE = 0;
const CONTENT_AFTER = 1;
const CONTENT_PREPEND = 2;
const CONTENT_APPEND = 3;
const CONTENT_REPLACE = 4;
const CONTENT_SET_INNER_CONTENT = 5;

function contentOptionsIsHtml(options) {
  return options !== undefined && options !== null && options.html === true;
}

function convertContent(content, prefix) {
  return webidl.converters.DOMString(content, prefix, "Argument 1");
}

const _state = Symbol("[[state]]");

function assertValidToken(token) {
  const state = token[_state];
  if (!state.valid) {
    throw new TypeError("This content token is no longer valid.");
  }
  return state;
}

class Element {
  constructor(state) {
    this[_state] = state;
  }

  get tagName() {
    const { transform, generation } = assertValidToken(this);
    return op_html_rewriter_element_tag_name(transform, generation);
  }

  set tagName(name) {
    const { transform, generation } = assertValidToken(this);
    name = webidl.converters.DOMString(name, "Failed to set 'tagName'");
    op_html_rewriter_element_set_tag_name(transform, generation, name);
  }

  get namespaceURI() {
    const { transform, generation } = assertValidToken(this);
    return op_html_rewriter_element_namespace_uri(transform, generation);
  }

  get attributes() {
    const { transform, generation } = assertValidToken(this);
    const attributes = op_html_rewriter_element_attributes(
      transform,
      generation,
    );
    return new SafeArrayIterator(attributes);
  }

  get removed() {
    const { transform, generation } = assertValidToken(this);
    return op_html_rewriter_token_removed(transform, generation);
  }

  getAttribute(name) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'getAttribute' on 'Element'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    name = webidl.converters.DOMString(name, prefix, "Argument 1");
    return op_html_rewriter_element_get_attribute(
      transform,
      generation,
      name,
    );
  }

  hasAttribute(name) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'hasAttribute' on 'Element'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    name = webidl.converters.DOMString(name, prefix, "Argument 1");
    return op_html_rewriter_element_has_attribute(
      transform,
      generation,
      name,
    );
  }

  setAttribute(name, value) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'setAttribute' on 'Element'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    name = webidl.converters.DOMString(name, prefix, "Argument 1");
    value = webidl.converters.DOMString(value, prefix, "Argument 2");
    op_html_rewriter_element_set_attribute(
      transform,
      generation,
      name,
      value,
    );
    return this;
  }

  removeAttribute(name) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'removeAttribute' on 'Element'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    name = webidl.converters.DOMString(name, prefix, "Argument 1");
    op_html_rewriter_element_remove_attribute(transform, generation, name);
    return this;
  }

  before(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'before' on 'Element'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_BEFORE,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  after(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'after' on 'Element'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_AFTER,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  prepend(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'prepend' on 'Element'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_PREPEND,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  append(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'append' on 'Element'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_APPEND,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  replace(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'replace' on 'Element'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_REPLACE,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  setInnerContent(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'setInnerContent' on 'Element'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_SET_INNER_CONTENT,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  remove() {
    const { transform, generation } = assertValidToken(this);
    op_html_rewriter_token_remove(transform, generation, false);
    return this;
  }

  removeAndKeepContent() {
    const { transform, generation } = assertValidToken(this);
    op_html_rewriter_token_remove(transform, generation, true);
    return this;
  }

  onEndTag(handler) {
    const state = assertValidToken(this);
    const prefix = "Failed to execute 'onEndTag' on 'Element'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    if (typeof handler !== "function") {
      throw new TypeError(`${prefix}: Argument 1 must be a function`);
    }
    const handlerId = state.handlers.length;
    ArrayPrototypePush(
      state.handlers,
      (endTag) => FunctionPrototypeCall(handler, undefined, endTag),
    );
    op_html_rewriter_element_on_end_tag(
      state.transform,
      state.generation,
      handlerId,
    );
  }
}

class Text {
  constructor(state) {
    this[_state] = state;
  }

  get text() {
    const { transform, generation } = assertValidToken(this);
    return op_html_rewriter_text_info(transform, generation)[0];
  }

  get lastInTextNode() {
    const { transform, generation } = assertValidToken(this);
    return op_html_rewriter_text_info(transform, generation)[1];
  }

  get removed() {
    const { transform, generation } = assertValidToken(this);
    return op_html_rewriter_token_removed(transform, generation);
  }

  before(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'before' on 'Text'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_BEFORE,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  after(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'after' on 'Text'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_AFTER,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  replace(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'replace' on 'Text'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_REPLACE,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  remove() {
    const { transform, generation } = assertValidToken(this);
    op_html_rewriter_token_remove(transform, generation, false);
    return this;
  }
}

class Comment {
  constructor(state) {
    this[_state] = state;
  }

  get text() {
    const { transform, generation } = assertValidToken(this);
    return op_html_rewriter_comment_text(transform, generation);
  }

  set text(text) {
    const { transform, generation } = assertValidToken(this);
    text = webidl.converters.DOMString(text, "Failed to set 'text'");
    op_html_rewriter_set_comment_text(transform, generation, text);
  }

  get removed() {
    const { transform, generation } = assertValidToken(this);
    return op_html_rewriter_token_removed(transform, generation);
  }

  before(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'before' on 'Comment'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_BEFORE,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  after(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'after' on 'Comment'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_AFTER,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  replace(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'replace' on 'Comment'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_REPLACE,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  remove() {
    const { transform, generation } = assertValidToken(this);
    op_html_rewriter_token_remove(transform, generation, false);
    return this;
  }
}

class Doctype {
  constructor(state) {
    this[_state] = state;
  }

  get name() {
    const { transform, generation } = assertValidToken(this);
    return op_html_rewriter_doctype_info(transform, generation)[0];
  }

  get publicId() {
    const { transform, generation } = assertValidToken(this);
    return op_html_rewriter_doctype_info(transform, generation)[1];
  }

  get systemId() {
    const { transform, generation } = assertValidToken(this);
    return op_html_rewriter_doctype_info(transform, generation)[2];
  }

  get removed() {
    const { transform, generation } = assertValidToken(this);
    return op_html_rewriter_token_removed(transform, generation);
  }

  remove() {
    const { transform, generation } = assertValidToken(this);
    op_html_rewriter_token_remove(transform, generation, false);
    return this;
  }
}

class DocumentEnd {
  constructor(state) {
    this[_state] = state;
  }

  append(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'append' on 'DocumentEnd'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_APPEND,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }
}

class EndTag {
  constructor(state) {
    this[_state] = state;
  }

  get name() {
    const { transform, generation } = assertValidToken(this);
    return op_html_rewriter_end_tag_name(transform, generation);
  }

  set name(name) {
    const { transform, generation } = assertValidToken(this);
    name = webidl.converters.DOMString(name, "Failed to set 'name'");
    op_html_rewriter_set_end_tag_name(transform, generation, name);
  }

  before(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'before' on 'EndTag'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_BEFORE,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  after(content, options) {
    const { transform, generation } = assertValidToken(this);
    const prefix = "Failed to execute 'after' on 'EndTag'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    op_html_rewriter_token_content(
      transform,
      generation,
      CONTENT_AFTER,
      convertContent(content, prefix),
      contentOptionsIsHtml(options),
    );
    return this;
  }

  remove() {
    const { transform, generation } = assertValidToken(this);
    op_html_rewriter_token_remove(transform, generation, false);
    return this;
  }
}

const TOKEN_CONSTRUCTORS = {
  __proto__: null,
  element: Element,
  text: Text,
  comment: Comment,
  doctype: Doctype,
  documentEnd: DocumentEnd,
  endTag: EndTag,
};

function createToken(transform, handlers, msg) {
  const state = {
    valid: true,
    transform,
    generation: msg.generation,
    handlers,
  };
  const Constructor = TOKEN_CONSTRUCTORS[msg.tokenKind];
  return { token: new Constructor(state), state };
}

function isThenable(value) {
  return value !== null && value !== undefined &&
    typeof value.then === "function";
}

function throwPumpError(msg) {
  // `msg.handler === true` means a JS handler aborted the rewriter; the
  // original exception has already been rethrown by the dispatch that failed,
  // so this is only reachable when the transform was aborted out of band.
  throw new TypeError(msg.message ?? "The rewriter transform was aborted");
}

/**
 * Runs the JS handler for a dispatched token in async mode. Always settles
 * the dispatch with `op_html_rewriter_token_done` / `_token_error` so the
 * parked rewriter thread is never left parked.
 */
async function runHandler(transform, handlers, msg) {
  const { token, state } = createToken(transform, handlers, msg);
  try {
    const result = handlers[msg.handlerId](token);
    if (isThenable(result)) {
      await result;
    }
  } catch (error) {
    state.valid = false;
    op_html_rewriter_token_error(transform);
    throw error;
  }
  state.valid = false;
  op_html_rewriter_token_done(transform);
}

function runHandlerSync(transform, handlers, msg) {
  const { token, state } = createToken(transform, handlers, msg);
  let result;
  try {
    result = handlers[msg.handlerId](token);
  } catch (error) {
    state.valid = false;
    op_html_rewriter_token_error(transform);
    throw error;
  }
  if (isThenable(result)) {
    state.valid = false;
    op_html_rewriter_token_error(transform);
    throw new TypeError(
      "Async handlers are not supported when transforming a string; transform a Response instead",
    );
  }
  state.valid = false;
  op_html_rewriter_token_done(transform);
}

/**
 * Pulls messages from the rewriter thread, running handlers for dispatched
 * tokens, until the pending write or end operation completes. Returns the
 * output bytes produced by that operation.
 */
async function pump(transform, handlers) {
  while (true) {
    const msg = await op_html_rewriter_pump(transform);
    switch (msg.kind) {
      case "dispatch":
        await runHandler(transform, handlers, msg);
        break;
      case "writeDone":
      case "endDone":
        return msg.output;
      case "aborted":
        // The transform was cancelled; resolve so stream teardown can
        // complete.
        return new Uint8Array(0);
      case "error":
        throwPumpError(msg);
    }
  }
}

function concatOutput(chunks) {
  let length = 0;
  for (let i = 0; i < chunks.length; ++i) {
    length += TypedArrayPrototypeGetByteLength(chunks[i]);
  }
  const result = new Uint8Array(length);
  let offset = 0;
  for (let i = 0; i < chunks.length; ++i) {
    TypedArrayPrototypeSet(result, chunks[i], offset);
    offset += TypedArrayPrototypeGetByteLength(chunks[i]);
  }
  return result;
}

const _elementHandlers = Symbol("[[elementHandlers]]");
const _documentHandlers = Symbol("[[documentHandlers]]");

class HTMLRewriter {
  [_elementHandlers] = [];
  [_documentHandlers] = [];

  constructor() {
    this[webidl.brand] = webidl.brand;
  }

  on(selector, handlers) {
    webidl.assertBranded(this, HTMLRewriterPrototype);
    const prefix = "Failed to execute 'on' on 'HTMLRewriter'";
    webidl.requiredArguments(arguments.length, 2, prefix);
    selector = webidl.converters.DOMString(selector, prefix, "Argument 1");
    // Validate the selector eagerly so `on()` throws, not `transform()`.
    op_html_rewriter_parse_selector(selector);
    ArrayPrototypePush(this[_elementHandlers], { selector, handlers });
    return this;
  }

  onDocument(handlers) {
    webidl.assertBranded(this, HTMLRewriterPrototype);
    const prefix = "Failed to execute 'onDocument' on 'HTMLRewriter'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    ArrayPrototypePush(this[_documentHandlers], { handlers });
    return this;
  }

  transform(input) {
    webidl.assertBranded(this, HTMLRewriterPrototype);
    const prefix = "Failed to execute 'transform' on 'HTMLRewriter'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    if (typeof input === "string") {
      return transformString(this, input);
    }
    if (ObjectPrototypeIsPrototypeOf(ResponsePrototype, input)) {
      return transformResponse(this, input);
    }
    throw new TypeError(
      `${prefix}: Argument 1 must be a string or a Response`,
    );
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(HTMLRewriterPrototype, this),
        keys: [],
      }),
      inspectOptions,
    );
  }
}

webidl.configureInterface(HTMLRewriter);
const HTMLRewriterPrototype = HTMLRewriter.prototype;

/**
 * Snapshots the registered handlers into a flat function table (indexed by
 * the numeric handler ids used in the transform spec), so the rewriter can
 * be reused and mutated while transforms are in flight.
 */
function buildSpec(rewriter, syncMode) {
  const handlers = [];
  const elementHandlers = [];
  const documentHandlers = [];

  const registry = rewriter[_elementHandlers];
  for (let i = 0; i < registry.length; ++i) {
    const { selector, handlers: handlerObject } = registry[i];
    const row = { selector, element: null, comments: null, text: null };
    const element = handlerObject.element;
    if (element !== undefined) {
      row.element = handlers.length;
      ArrayPrototypePush(
        handlers,
        (token) => FunctionPrototypeCall(element, handlerObject, token),
      );
    }
    const comments = handlerObject.comments;
    if (comments !== undefined) {
      row.comments = handlers.length;
      ArrayPrototypePush(
        handlers,
        (token) => FunctionPrototypeCall(comments, handlerObject, token),
      );
    }
    const text = handlerObject.text;
    if (text !== undefined) {
      row.text = handlers.length;
      ArrayPrototypePush(
        handlers,
        (token) => FunctionPrototypeCall(text, handlerObject, token),
      );
    }
    ArrayPrototypePush(elementHandlers, row);
  }

  const documentRegistry = rewriter[_documentHandlers];
  for (let i = 0; i < documentRegistry.length; ++i) {
    const { handlers: handlerObject } = documentRegistry[i];
    const row = { doctype: null, comments: null, text: null, end: null };
    const doctype = handlerObject.doctype;
    if (doctype !== undefined) {
      row.doctype = handlers.length;
      ArrayPrototypePush(
        handlers,
        (token) => FunctionPrototypeCall(doctype, handlerObject, token),
      );
    }
    const comments = handlerObject.comments;
    if (comments !== undefined) {
      row.comments = handlers.length;
      ArrayPrototypePush(
        handlers,
        (token) => FunctionPrototypeCall(comments, handlerObject, token),
      );
    }
    const text = handlerObject.text;
    if (text !== undefined) {
      row.text = handlers.length;
      ArrayPrototypePush(
        handlers,
        (token) => FunctionPrototypeCall(text, handlerObject, token),
      );
    }
    const end = handlerObject.end;
    if (end !== undefined) {
      row.end = handlers.length;
      ArrayPrototypePush(
        handlers,
        (token) => FunctionPrototypeCall(end, handlerObject, token),
      );
    }
    ArrayPrototypePush(documentHandlers, row);
  }

  return {
    handlers,
    spec: { elementHandlers, documentHandlers, syncMode },
  };
}

/**
 * Pumps the sync rewriter until the pending write or end completes, running
 * handlers for dispatched tokens and pushing the produced output onto
 * `chunks`. Returns `true` once the final `endDone` is seen.
 */
function pumpSync(transform, handlers, chunks) {
  while (true) {
    const msg = op_html_rewriter_pump_sync(transform);
    switch (msg.kind) {
      case "dispatch":
        runHandlerSync(transform, handlers, msg);
        break;
      case "writeDone":
        ArrayPrototypePush(chunks, msg.output);
        return false;
      case "endDone":
        ArrayPrototypePush(chunks, msg.output);
        return true;
      case "aborted":
      case "error":
        throwPumpError(msg);
    }
  }
}

function transformString(rewriter, input) {
  const { handlers, spec } = buildSpec(rewriter, true);
  const transform = op_html_rewriter_start(spec);
  const chunks = [];
  // Drive write and end one at a time: each runs on a blocking task that owns
  // the rewriter for its duration, so the end must not be issued until the
  // write has completed and handed the rewriter back.
  op_html_rewriter_write(transform, core.encode(input));
  pumpSync(transform, handlers, chunks);
  op_html_rewriter_end(transform);
  pumpSync(transform, handlers, chunks);
  return core.decode(concatOutput(chunks));
}

function copyInnerResponse(inner) {
  return {
    type: inner.type,
    body: null,
    headerList: [...new SafeArrayIterator(inner.headerList)],
    urlList: inner.urlList,
    status: inner.status,
    statusMessage: inner.statusMessage,
    aborted: inner.aborted,
    url() {
      if (this.urlList.length == 0) return null;
      return this.urlList[this.urlList.length - 1];
    },
  };
}

function transformResponse(rewriter, response) {
  if (response.bodyUsed) {
    throw new TypeError("Response body is already used");
  }
  // Reading `body` also materializes a lazy static body on the inner
  // response, so it must happen before `getInnerResponse`.
  const body = response.body;
  const newInner = copyInnerResponse(getInnerResponse(response));

  if (body === null) {
    // Null body (e.g. 204 or 304 responses): nothing to rewrite.
    return fromInnerResponse(newInner, "response");
  }

  const { handlers, spec } = buildSpec(rewriter, false);
  const transform = op_html_rewriter_start(spec);
  const prefix = "Failed to execute 'transform' on 'HTMLRewriter'";

  const transformStream = new TransformStream({
    async transform(chunk, controller) {
      chunk = webidl.converters.BufferSource(chunk, prefix, "chunk");
      op_html_rewriter_write(transform, chunk);
      const output = await pump(transform, handlers);
      if (TypedArrayPrototypeGetByteLength(output) > 0) {
        controller.enqueue(output);
      }
    },
    async flush(controller) {
      op_html_rewriter_end(transform);
      const output = await pump(transform, handlers);
      if (TypedArrayPrototypeGetByteLength(output) > 0) {
        controller.enqueue(output);
      }
    },
    cancel() {
      op_html_rewriter_abort(transform);
    },
  });

  newInner.body = new InnerBody(body.pipeThrough(transformStream));
  return fromInnerResponse(newInner, "response");
}

return { HTMLRewriter };
})();
