// Copyright 2018-2026 the Deno authors. MIT license.

import {
  assert,
  assertEquals,
  assertRejects,
  assertStringIncludes,
  assertThrows,
} from "./test_util.ts";

Deno.test({ permissions: "none" }, function htmlRewriterStringTransform() {
  const output = new HTMLRewriter()
    .on("a[href]", {
      element(element) {
        element.setAttribute("href", "https://deno.com");
        element.setInnerContent("Deno");
      },
    })
    .transform('<p>See <a href="http://example.com">example</a>!</p>');
  assertEquals(output, '<p>See <a href="https://deno.com">Deno</a>!</p>');
});

Deno.test(
  { permissions: "none" },
  async function htmlRewriterResponseTransform() {
    const response = new Response("<h1>hello</h1>", {
      status: 201,
      statusText: "Created",
      headers: { "content-type": "text/html", "x-test": "1" },
    });
    const transformed = new HTMLRewriter()
      .on("h1", {
        element(element) {
          element.setInnerContent("bye");
        },
      })
      .transform(response);
    assertEquals(transformed.status, 201);
    assertEquals(transformed.statusText, "Created");
    assertEquals(transformed.headers.get("content-type"), "text/html");
    assertEquals(transformed.headers.get("x-test"), "1");
    assertEquals(await transformed.text(), "<h1>bye</h1>");
  },
);

Deno.test(
  { permissions: "none" },
  async function htmlRewriterStreamingChunks() {
    const encoder = new TextEncoder();
    const parts = ["<p>He", "llo wo", "rld</p>"];
    const body = new ReadableStream({
      start(controller) {
        for (const part of parts) {
          controller.enqueue(encoder.encode(part));
        }
        controller.close();
      },
    });
    const textChunks: [string, boolean][] = [];
    const transformed = new HTMLRewriter()
      .on("p", {
        text(text) {
          textChunks.push([text.text, text.lastInTextNode]);
        },
      })
      .transform(new Response(body));
    assertEquals(await transformed.text(), "<p>Hello world</p>");
    // The text node is split across input chunks; the final chunk is an
    // empty one with `lastInTextNode` set.
    assertEquals(textChunks.length > 1, true);
    assertEquals(textChunks[textChunks.length - 1], ["", true]);
  },
);

Deno.test({ permissions: "none" }, async function htmlRewriterAsyncHandlers() {
  const order: string[] = [];
  const transformed = new HTMLRewriter()
    .on("p", {
      async element(element) {
        order.push("start");
        await new Promise((resolve) => setTimeout(resolve, 10));
        element.setInnerContent("rewritten");
        order.push("end");
      },
    })
    .transform(new Response("<p>one</p><p>two</p>"));
  assertEquals(await transformed.text(), "<p>rewritten</p><p>rewritten</p>");
  assertEquals(order, ["start", "end", "start", "end"]);
});

Deno.test({ permissions: "none" }, function htmlRewriterElementApi() {
  const output = new HTMLRewriter()
    .on("div", {
      element(element) {
        assertEquals(element.tagName, "div");
        assertEquals(element.namespaceURI, "http://www.w3.org/1999/xhtml");
        assertEquals([...element.attributes], [["class", "a"], ["id", "b"]]);
        assertEquals(element.getAttribute("class"), "a");
        assertEquals(element.getAttribute("missing"), null);
        assert(element.hasAttribute("id"));
        assert(!element.hasAttribute("missing"));
        element.removeAttribute("class");
        element.setAttribute("data-x", "1");
        element.tagName = "section";
        element.before("[before]");
        element.prepend("[prepend]");
        element.append("[append]");
        element.after("[after]");
        assertEquals(element.removed, false);
      },
    })
    .transform('<div class="a" id="b">content</div>');
  assertEquals(
    output,
    '[before]<section id="b" data-x="1">[prepend]content[append]</section>[after]',
  );
});

Deno.test({ permissions: "none" }, function htmlRewriterElementRemove() {
  assertEquals(
    new HTMLRewriter()
      .on("b", {
        element(element) {
          element.remove();
          assertEquals(element.removed, true);
        },
      })
      .transform("<p>a<b>removed</b>c</p>"),
    "<p>ac</p>",
  );
  assertEquals(
    new HTMLRewriter()
      .on("b", {
        element(element) {
          element.removeAndKeepContent();
        },
      })
      .transform("<p>a<b>kept</b>c</p>"),
    "<p>akeptc</p>",
  );
});

Deno.test({ permissions: "none" }, function htmlRewriterEndTag() {
  const output = new HTMLRewriter()
    .on("h1", {
      element(element) {
        element.onEndTag((endTag) => {
          assertEquals(endTag.name, "h1");
          endTag.before("!", { html: false });
        });
      },
    })
    .transform("<h1>title</h1>");
  assertEquals(output, "<h1>title!</h1>");
});

Deno.test({ permissions: "none" }, function htmlRewriterComments() {
  const output = new HTMLRewriter()
    .on("p", {
      comments(comment) {
        assertEquals(comment.text, " inner ");
        comment.text = " replaced ";
      },
    })
    .transform("<p><!-- inner --></p>");
  assertEquals(output, "<p><!-- replaced --></p>");
});

Deno.test({ permissions: "none" }, function htmlRewriterDocumentHandlers() {
  const output = new HTMLRewriter()
    .onDocument({
      doctype(doctype) {
        assertEquals(doctype.name, "html");
        assertEquals(doctype.publicId, null);
        assertEquals(doctype.systemId, null);
      },
      comments(comment) {
        comment.remove();
        assertEquals(comment.removed, true);
      },
      text(text) {
        if (text.text === "hi") {
          text.replace("bye");
        }
      },
      end(end) {
        end.append("<!-- end -->", { html: true });
      },
    })
    .transform("<!DOCTYPE html><!-- remove me --><p>hi</p>");
  assertEquals(output, "<!DOCTYPE html><p>bye</p><!-- end -->");
});

Deno.test({ permissions: "none" }, function htmlRewriterContentEscaping() {
  assertEquals(
    new HTMLRewriter()
      .on("p", {
        element(element) {
          element.setInnerContent("<b>bold</b>");
        },
      })
      .transform("<p>x</p>"),
    "<p>&lt;b&gt;bold&lt;/b&gt;</p>",
  );
  assertEquals(
    new HTMLRewriter()
      .on("p", {
        element(element) {
          element.setInnerContent("<b>bold</b>", { html: true });
        },
      })
      .transform("<p>x</p>"),
    "<p><b>bold</b></p>",
  );
});

Deno.test({ permissions: "none" }, function htmlRewriterInvalidSelector() {
  const rewriter = new HTMLRewriter();
  assertThrows(
    () => rewriter.on("p[", {}),
    TypeError,
    "Invalid selector",
  );
  assertThrows(
    () => rewriter.on("p ~ a", {}),
    TypeError,
    "Invalid selector",
  );
});

Deno.test(
  { permissions: "none" },
  async function htmlRewriterHandlerError() {
    const transformed = new HTMLRewriter()
      .on("p", {
        element() {
          throw new Error("boom");
        },
      })
      .transform(new Response("<p>x</p>"));
    await assertRejects(() => transformed.text(), Error, "boom");
  },
);

Deno.test({ permissions: "none" }, function htmlRewriterHandlerErrorSync() {
  assertThrows(
    () =>
      new HTMLRewriter()
        .on("p", {
          element() {
            throw new Error("sync boom");
          },
        })
        .transform("<p>x</p>"),
    Error,
    "sync boom",
  );
});

Deno.test(
  { permissions: "none" },
  function htmlRewriterAsyncHandlerInStringTransform() {
    assertThrows(
      () =>
        new HTMLRewriter()
          .on("p", {
            async element() {},
          })
          .transform("<p>x</p>"),
      TypeError,
      "Async handlers are not supported when transforming a string",
    );
  },
);

Deno.test({ permissions: "none" }, function htmlRewriterTokenInvalidation() {
  // deno-lint-ignore no-explicit-any
  let leaked: any;
  new HTMLRewriter()
    .on("p", {
      element(element) {
        leaked = element;
      },
    })
    .transform("<p>x</p>");
  assertThrows(
    () => leaked.setAttribute("a", "b"),
    TypeError,
    "This content token is no longer valid",
  );
});

Deno.test(
  { permissions: "none" },
  async function htmlRewriterTokenValidAcrossAwait() {
    const transformed = new HTMLRewriter()
      .on("p", {
        async element(element) {
          await new Promise((resolve) => setTimeout(resolve, 5));
          // The token stays valid until the handler's promise settles.
          element.setInnerContent("late");
        },
      })
      .transform(new Response("<p>x</p>"));
    assertEquals(await transformed.text(), "<p>late</p>");
  },
);

Deno.test({ permissions: "none" }, async function htmlRewriterReuse() {
  const rewriter = new HTMLRewriter().on("b", {
    element(element) {
      element.setInnerContent("R");
    },
  });
  assertEquals(rewriter.transform("<b>one</b>"), "<b>R</b>");
  assertEquals(rewriter.transform("<b>two</b>"), "<b>R</b>");
  const [first, second] = await Promise.all([
    rewriter.transform(new Response("<b>three</b>")).text(),
    rewriter.transform(new Response("<b>four</b>")).text(),
  ]);
  assertEquals(first, "<b>R</b>");
  assertEquals(second, "<b>R</b>");
});

Deno.test({ permissions: "none" }, function htmlRewriterNullBody() {
  let handlerCalled = false;
  const transformed = new HTMLRewriter()
    .on("p", {
      element() {
        handlerCalled = true;
      },
    })
    .transform(new Response(null, { status: 204, headers: { "x-a": "b" } }));
  assertEquals(transformed.status, 204);
  assertEquals(transformed.body, null);
  assertEquals(transformed.headers.get("x-a"), "b");
  assertEquals(handlerCalled, false);
});

Deno.test({ permissions: "none" }, async function htmlRewriterInvalidInput() {
  const rewriter = new HTMLRewriter();
  assertThrows(
    // deno-lint-ignore no-explicit-any
    () => rewriter.transform(42 as any),
    TypeError,
    "must be a string or a Response",
  );
  const used = new Response("<p>x</p>");
  await used.text();
  assertThrows(
    () => rewriter.transform(used),
    TypeError,
    "already used",
  );
});

Deno.test({ permissions: "none" }, function htmlRewriterMalformedHtml() {
  // Lenient parsing: unclosed tags do not error.
  assertEquals(
    new HTMLRewriter().transform("<div><p>unclosed"),
    "<div><p>unclosed",
  );
});

Deno.test({ permissions: "none" }, async function htmlRewriterCancel() {
  const encoder = new TextEncoder();
  // An infinite source; backpressure paces the pulls.
  const body = new ReadableStream({
    pull(controller) {
      controller.enqueue(encoder.encode("<p>chunk</p>"));
    },
  });
  const transformed = new HTMLRewriter()
    .on("p", {
      element(element) {
        element.setAttribute("x", "1");
      },
    })
    .transform(new Response(body));
  const reader = transformed.body!.getReader();
  const { value } = await reader.read();
  assertStringIncludes(new TextDecoder().decode(value), '<p x="1">');
  await reader.cancel();
});

Deno.test({ permissions: "none" }, function htmlRewriterIllegalConstructor() {
  // deno-lint-ignore no-explicit-any
  const rewriter = new HTMLRewriter() as any;
  assertThrows(() => rewriter.on.call({}, "p", {}), TypeError);
});
