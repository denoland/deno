/**
 * @license React
 * react-dom-server.browser.production.min.js
 *
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
const escapeHTML = Bun.escapeHTML;
var aa = require("react");
function k(a) {
  for (
    var b = "https://reactjs.org/docs/error-decoder.html?invariant=" + a, c = 1;
    c < arguments.length;
    c++
  )
    b += "&args[]=" + encodeURIComponent(arguments[c]);
  return (
    "Minified React error #" +
    a +
    "; visit " +
    b +
    " for the full message or use the non-minified dev environment for full errors and additional helpful warnings."
  );
}
var l = null,
  n = 0;
function p(controller, b) {
  if (0 !== b.length) controller.write(b);
}
function t(a, b) {
  p(a, b);
  return !0;
}
function ba(a) {
  a.flush(false);
}
var ca = new TextEncoder();
function u(a) {
  return a;
}
function v(a) {
  return a;
}
function da(a, b) {
  "function" === typeof a.error ? a.close(b) : a.end(b);
}
var w = Object.prototype.hasOwnProperty,
  ea =
    /^[:A-Z_a-z\u00C0-\u00D6\u00D8-\u00F6\u00F8-\u02FF\u0370-\u037D\u037F-\u1FFF\u200C-\u200D\u2070-\u218F\u2C00-\u2FEF\u3001-\uD7FF\uF900-\uFDCF\uFDF0-\uFFFD][:A-Z_a-z\u00C0-\u00D6\u00D8-\u00F6\u00F8-\u02FF\u0370-\u037D\u037F-\u1FFF\u200C-\u200D\u2070-\u218F\u2C00-\u2FEF\u3001-\uD7FF\uF900-\uFDCF\uFDF0-\uFFFD\-.0-9\u00B7\u0300-\u036F\u203F-\u2040]*$/,
  fa = {},
  ha = {};
function ia(a) {
  if (w.call(ha, a)) return !0;
  if (w.call(fa, a)) return !1;
  if (ea.test(a)) return (ha[a] = !0);
  fa[a] = !0;
  return !1;
}
function x(a, b, c, d, f, e, g) {
  this.acceptsBooleans = 2 === b || 3 === b || 4 === b;
  this.attributeName = d;
  this.attributeNamespace = f;
  this.mustUseProperty = c;
  this.propertyName = a;
  this.type = b;
  this.sanitizeURL = e;
  this.removeEmptyString = g;
}
var y = {};
"children dangerouslySetInnerHTML defaultValue defaultChecked innerHTML suppressContentEditableWarning suppressHydrationWarning style"
  .split(" ")
  .forEach(function (a) {
    y[a] = new x(a, 0, !1, a, null, !1, !1);
  });
[
  ["acceptCharset", "accept-charset"],
  ["className", "class"],
  ["htmlFor", "for"],
  ["httpEquiv", "http-equiv"],
].forEach(function (a) {
  var b = a[0];
  y[b] = new x(b, 1, !1, a[1], null, !1, !1);
});
["contentEditable", "draggable", "spellCheck", "value"].forEach(function (a) {
  y[a] = new x(a, 2, !1, a.toLowerCase(), null, !1, !1);
});
[
  "autoReverse",
  "externalResourcesRequired",
  "focusable",
  "preserveAlpha",
].forEach(function (a) {
  y[a] = new x(a, 2, !1, a, null, !1, !1);
});
"allowFullScreen async autoFocus autoPlay controls default defer disabled disablePictureInPicture disableRemotePlayback formNoValidate hidden loop noModule noValidate open playsInline readOnly required reversed scoped seamless itemScope"
  .split(" ")
  .forEach(function (a) {
    y[a] = new x(a, 3, !1, a.toLowerCase(), null, !1, !1);
  });
["checked", "multiple", "muted", "selected"].forEach(function (a) {
  y[a] = new x(a, 3, !0, a, null, !1, !1);
});
["capture", "download"].forEach(function (a) {
  y[a] = new x(a, 4, !1, a, null, !1, !1);
});
["cols", "rows", "size", "span"].forEach(function (a) {
  y[a] = new x(a, 6, !1, a, null, !1, !1);
});
["rowSpan", "start"].forEach(function (a) {
  y[a] = new x(a, 5, !1, a.toLowerCase(), null, !1, !1);
});
var ja = /[\-:]([a-z])/g;
function ka(a) {
  return a[1].toUpperCase();
}
"accent-height alignment-baseline arabic-form baseline-shift cap-height clip-path clip-rule color-interpolation color-interpolation-filters color-profile color-rendering dominant-baseline enable-background fill-opacity fill-rule flood-color flood-opacity font-family font-size font-size-adjust font-stretch font-style font-variant font-weight glyph-name glyph-orientation-horizontal glyph-orientation-vertical horiz-adv-x horiz-origin-x image-rendering letter-spacing lighting-color marker-end marker-mid marker-start overline-position overline-thickness paint-order panose-1 pointer-events rendering-intent shape-rendering stop-color stop-opacity strikethrough-position strikethrough-thickness stroke-dasharray stroke-dashoffset stroke-linecap stroke-linejoin stroke-miterlimit stroke-opacity stroke-width text-anchor text-decoration text-rendering underline-position underline-thickness unicode-bidi unicode-range units-per-em v-alphabetic v-hanging v-ideographic v-mathematical vector-effect vert-adv-y vert-origin-x vert-origin-y word-spacing writing-mode xmlns:xlink x-height"
  .split(" ")
  .forEach(function (a) {
    var b = a.replace(ja, ka);
    y[b] = new x(b, 1, !1, a, null, !1, !1);
  });
"xlink:actuate xlink:arcrole xlink:role xlink:show xlink:title xlink:type"
  .split(" ")
  .forEach(function (a) {
    var b = a.replace(ja, ka);
    y[b] = new x(b, 1, !1, a, "http://www.w3.org/1999/xlink", !1, !1);
  });
["xml:base", "xml:lang", "xml:space"].forEach(function (a) {
  var b = a.replace(ja, ka);
  y[b] = new x(b, 1, !1, a, "http://www.w3.org/XML/1998/namespace", !1, !1);
});
["tabIndex", "crossOrigin"].forEach(function (a) {
  y[a] = new x(a, 1, !1, a.toLowerCase(), null, !1, !1);
});
y.xlinkHref = new x(
  "xlinkHref",
  1,
  !1,
  "xlink:href",
  "http://www.w3.org/1999/xlink",
  !0,
  !1
);
["src", "href", "action", "formAction"].forEach(function (a) {
  y[a] = new x(a, 1, !1, a.toLowerCase(), null, !0, !0);
});
var z = {
    animationIterationCount: !0,
    aspectRatio: !0,
    borderImageOutset: !0,
    borderImageSlice: !0,
    borderImageWidth: !0,
    boxFlex: !0,
    boxFlexGroup: !0,
    boxOrdinalGroup: !0,
    columnCount: !0,
    columns: !0,
    flex: !0,
    flexGrow: !0,
    flexPositive: !0,
    flexShrink: !0,
    flexNegative: !0,
    flexOrder: !0,
    gridArea: !0,
    gridRow: !0,
    gridRowEnd: !0,
    gridRowSpan: !0,
    gridRowStart: !0,
    gridColumn: !0,
    gridColumnEnd: !0,
    gridColumnSpan: !0,
    gridColumnStart: !0,
    fontWeight: !0,
    lineClamp: !0,
    lineHeight: !0,
    opacity: !0,
    order: !0,
    orphans: !0,
    tabSize: !0,
    widows: !0,
    zIndex: !0,
    zoom: !0,
    fillOpacity: !0,
    floodOpacity: !0,
    stopOpacity: !0,
    strokeDasharray: !0,
    strokeDashoffset: !0,
    strokeMiterlimit: !0,
    strokeOpacity: !0,
    strokeWidth: !0,
  },
  la = ["Webkit", "ms", "Moz", "O"];
Object.keys(z).forEach(function (a) {
  la.forEach(function (b) {
    b = b + a.charAt(0).toUpperCase() + a.substring(1);
    z[b] = z[a];
  });
});
var ma = /["'&<>]/;
function A(a) {
  if ("boolean" === typeof a || "number" === typeof a) return "" + a;
  return escapeHTML(a);
}
var na = /([A-Z])/g,
  oa = /^ms-/,
  pa = Array.isArray,
  qa = v("<script>"),
  ra = v("\x3c/script>"),
  sa = v('<script src="'),
  ta = v('<script type="module" src="'),
  ua = v('" async="">\x3c/script>'),
  va = /(<\/|<)(s)(cript)/gi;
function wa(a, b, c, d) {
  return "" + b + ("s" === c ? "\\u0073" : "\\u0053") + d;
}
function xa(a, b, c, d, f) {
  a = void 0 === a ? "" : a;
  b = void 0 === b ? qa : v('<script nonce="' + A(b) + '">');
  var e = [];
  void 0 !== c && e.push(b, u(("" + c).replace(va, wa)), ra);
  if (void 0 !== d) for (c = 0; c < d.length; c++) e.push(sa, u(A(d[c])), ua);
  if (void 0 !== f) for (d = 0; d < f.length; d++) e.push(ta, u(A(f[d])), ua);
  return {
    bootstrapChunks: e,
    startInlineScript: b,
    placeholderPrefix: v(a + "P:"),
    segmentPrefix: v(a + "S:"),
    boundaryPrefix: a + "B:",
    idPrefix: a,
    nextSuspenseID: 0,
    sentCompleteSegmentFunction: !1,
    sentCompleteBoundaryFunction: !1,
    sentClientRenderFunction: !1,
  };
}
function B(a, b) {
  return { insertionMode: a, selectedValue: b };
}
function ya(a) {
  return B(
    "http://www.w3.org/2000/svg" === a
      ? 2
      : "http://www.w3.org/1998/Math/MathML" === a
      ? 3
      : 0,
    null
  );
}
function za(a, b, c) {
  switch (b) {
    case "select":
      return B(1, null != c.value ? c.value : c.defaultValue);
    case "svg":
      return B(2, null);
    case "math":
      return B(3, null);
    case "foreignObject":
      return B(1, null);
    case "table":
      return B(4, null);
    case "thead":
    case "tbody":
    case "tfoot":
      return B(5, null);
    case "colgroup":
      return B(7, null);
    case "tr":
      return B(6, null);
  }
  return 4 <= a.insertionMode || 0 === a.insertionMode ? B(1, null) : a;
}
var Aa = v("\x3c!-- --\x3e"),
  Ba = new Map(),
  Ca = v(' style="'),
  Da = v(":"),
  Ea = v(";");
function Fa(a, b, c) {
  if ("object" !== typeof c) throw Error(k(62));
  b = !0;
  for (var d in c)
    if (w.call(c, d)) {
      var f = c[d];
      if (null != f && "boolean" !== typeof f && "" !== f) {
        if (0 === d.indexOf("--")) {
          var e = u(A(d));
          f = u(A(("" + f).trim()));
        } else {
          e = d;
          var g = Ba.get(e);
          void 0 !== g
            ? (e = g)
            : ((g = v(
                A(e.replace(na, "-$1").toLowerCase().replace(oa, "-ms-"))
              )),
              Ba.set(e, g),
              (e = g));
          f =
            "number" === typeof f
              ? 0 === f || w.call(z, d)
                ? u("" + f)
                : u(f + "px")
              : u(A(("" + f).trim()));
        }
        b ? ((b = !1), a.push(Ca, e, Da, f)) : a.push(Ea, e, Da, f);
      }
    }
  b || a.push(D);
}
var G = v(" "),
  H = v('="'),
  D = v('"'),
  Ga = v('=""');
function I(a, b, c, d) {
  switch (c) {
    case "style":
      Fa(a, b, d);
      return;
    case "defaultValue":
    case "defaultChecked":
    case "innerHTML":
    case "suppressContentEditableWarning":
    case "suppressHydrationWarning":
      return;
  }
  if (
    !(2 < c.length) ||
    ("o" !== c[0] && "O" !== c[0]) ||
    ("n" !== c[1] && "N" !== c[1])
  )
    if (((b = y.hasOwnProperty(c) ? y[c] : null), null !== b)) {
      switch (typeof d) {
        case "function":
        case "symbol":
          return;
        case "boolean":
          if (!b.acceptsBooleans) return;
      }
      c = u(b.attributeName);
      switch (b.type) {
        case 3:
          d && a.push(G, c, Ga);
          break;
        case 4:
          !0 === d ? a.push(G, c, Ga) : !1 !== d && a.push(G, c, H, u(A(d)), D);
          break;
        case 5:
          isNaN(d) || a.push(G, c, H, u(A(d)), D);
          break;
        case 6:
          !isNaN(d) && 1 <= d && a.push(G, c, H, u(A(d)), D);
          break;
        default:
          b.sanitizeURL && (d = "" + d), a.push(G, c, H, u(A(d)), D);
      }
    } else if (ia(c)) {
      switch (typeof d) {
        case "function":
        case "symbol":
          return;
        case "boolean":
          if (
            ((b = c.toLowerCase().slice(0, 5)), "data-" !== b && "aria-" !== b)
          )
            return;
      }
      a.push(G, u(c), H, u(A(d)), D);
    }
}
var J = v(">"),
  Ha = v("/>");
function K(a, b, c) {
  if (null != b) {
    if (null != c) throw Error(k(60));
    if ("object" !== typeof b || !("__html" in b)) throw Error(k(61));
    b = b.__html;
    null !== b && void 0 !== b && a.push(u("" + b));
  }
}
function Ia(a) {
  var b = "";
  aa.Children.forEach(a, function (a) {
    null != a && (b += a);
  });
  return b;
}
var Ja = v(' selected=""');
function Ka(a, b, c, d) {
  a.push(L(c));
  var f = (c = null),
    e;
  for (e in b)
    if (w.call(b, e)) {
      var g = b[e];
      if (null != g)
        switch (e) {
          case "children":
            c = g;
            break;
          case "dangerouslySetInnerHTML":
            f = g;
            break;
          default:
            I(a, d, e, g);
        }
    }
  a.push(J);
  K(a, f, c);
  return "string" === typeof c ? (a.push(u(A(c))), null) : c;
}
var La = v("\n"),
  Ma = /^[a-zA-Z][a-zA-Z:_\.\-\d]*$/,
  Na = new Map();
function L(a) {
  var b = Na.get(a);
  if (void 0 === b) {
    if (!Ma.test(a)) throw Error(k(65, a));
    b = v("<" + a);
    Na.set(a, b);
  }
  return b;
}
var Oa = v("<!DOCTYPE html>");
function Pa(a, b, c, d, f) {
  switch (b) {
    case "select":
      a.push(L("select"));
      var e = null,
        g = null;
      for (r in c)
        if (w.call(c, r)) {
          var h = c[r];
          if (null != h)
            switch (r) {
              case "children":
                e = h;
                break;
              case "dangerouslySetInnerHTML":
                g = h;
                break;
              case "defaultValue":
              case "value":
                break;
              default:
                I(a, d, r, h);
            }
        }
      a.push(J);
      K(a, g, e);
      return e;
    case "option":
      g = f.selectedValue;
      a.push(L("option"));
      var m = (h = null),
        q = null;
      var r = null;
      for (e in c)
        if (w.call(c, e) && ((b = c[e]), null != b))
          switch (e) {
            case "children":
              h = b;
              break;
            case "selected":
              q = b;
              break;
            case "dangerouslySetInnerHTML":
              r = b;
              break;
            case "value":
              m = b;
            default:
              I(a, d, e, b);
          }
      if (null != g)
        if (((c = null !== m ? "" + m : Ia(h)), pa(g)))
          for (d = 0; d < g.length; d++) {
            if ("" + g[d] === c) {
              a.push(Ja);
              break;
            }
          }
        else "" + g === c && a.push(Ja);
      else q && a.push(Ja);
      a.push(J);
      K(a, r, h);
      return h;
    case "textarea":
      a.push(L("textarea"));
      r = g = e = null;
      for (h in c)
        if (w.call(c, h) && ((m = c[h]), null != m))
          switch (h) {
            case "children":
              r = m;
              break;
            case "value":
              e = m;
              break;
            case "defaultValue":
              g = m;
              break;
            case "dangerouslySetInnerHTML":
              throw Error(k(91));
            default:
              I(a, d, h, m);
          }
      null === e && null !== g && (e = g);
      a.push(J);
      if (null != r) {
        if (null != e) throw Error(k(92));
        if (pa(r) && 1 < r.length) throw Error(k(93));
        e = "" + r;
      }
      "string" === typeof e && "\n" === e[0] && a.push(La);
      null !== e && a.push(u(A("" + e)));
      return null;
    case "input":
      a.push(L("input"));
      m = r = h = e = null;
      for (g in c)
        if (w.call(c, g) && ((q = c[g]), null != q))
          switch (g) {
            case "children":
            case "dangerouslySetInnerHTML":
              throw Error(k(399, "input"));
            case "defaultChecked":
              m = q;
              break;
            case "defaultValue":
              h = q;
              break;
            case "checked":
              r = q;
              break;
            case "value":
              e = q;
              break;
            default:
              I(a, d, g, q);
          }
      null !== r ? I(a, d, "checked", r) : null !== m && I(a, d, "checked", m);
      null !== e ? I(a, d, "value", e) : null !== h && I(a, d, "value", h);
      a.push(Ha);
      return null;
    case "menuitem":
      a.push(L("menuitem"));
      for (var E in c)
        if (w.call(c, E) && ((e = c[E]), null != e))
          switch (E) {
            case "children":
            case "dangerouslySetInnerHTML":
              throw Error(k(400));
            default:
              I(a, d, E, e);
          }
      a.push(J);
      return null;
    case "listing":
    case "pre":
      a.push(L(b));
      g = e = null;
      for (m in c)
        if (w.call(c, m) && ((h = c[m]), null != h))
          switch (m) {
            case "children":
              e = h;
              break;
            case "dangerouslySetInnerHTML":
              g = h;
              break;
            default:
              I(a, d, m, h);
          }
      a.push(J);
      if (null != g) {
        if (null != e) throw Error(k(60));
        if ("object" !== typeof g || !("__html" in g)) throw Error(k(61));
        c = g.__html;
        null !== c &&
          void 0 !== c &&
          ("string" === typeof c && 0 < c.length && "\n" === c[0]
            ? a.push(La, u(c))
            : a.push(u("" + c)));
      }
      "string" === typeof e && "\n" === e[0] && a.push(La);
      return e;
    case "area":
    case "base":
    case "br":
    case "col":
    case "embed":
    case "hr":
    case "img":
    case "keygen":
    case "link":
    case "meta":
    case "param":
    case "source":
    case "track":
    case "wbr":
      a.push(L(b));
      for (var F in c)
        if (w.call(c, F) && ((e = c[F]), null != e))
          switch (F) {
            case "children":
            case "dangerouslySetInnerHTML":
              throw Error(k(399, b));
            default:
              I(a, d, F, e);
          }
      a.push(Ha);
      return null;
    case "annotation-xml":
    case "color-profile":
    case "font-face":
    case "font-face-src":
    case "font-face-uri":
    case "font-face-format":
    case "font-face-name":
    case "missing-glyph":
      return Ka(a, c, b, d);
    case "html":
      return 0 === f.insertionMode && a.push(Oa), Ka(a, c, b, d);
    default:
      if (-1 === b.indexOf("-") && "string" !== typeof c.is)
        return Ka(a, c, b, d);
      a.push(L(b));
      g = e = null;
      for (q in c)
        if (w.call(c, q) && ((h = c[q]), null != h))
          switch (q) {
            case "children":
              e = h;
              break;
            case "dangerouslySetInnerHTML":
              g = h;
              break;
            case "style":
              Fa(a, d, h);
              break;
            case "suppressContentEditableWarning":
            case "suppressHydrationWarning":
              break;
            default:
              ia(q) &&
                "function" !== typeof h &&
                "symbol" !== typeof h &&
                a.push(G, u(q), H, u(A(h)), D);
          }
      a.push(J);
      K(a, g, e);
      return e;
  }
}
var Qa = v("</"),
  Ra = v(">"),
  Sa = v('<template id="'),
  Ta = v('"></template>'),
  Ua = v("\x3c!--$--\x3e"),
  Va = v('\x3c!--$?--\x3e<template id="'),
  Wa = v('"></template>'),
  Xa = v("\x3c!--$!--\x3e"),
  Ya = v("\x3c!--/$--\x3e");
function Za(a, b, c) {
  p(a, Va);
  if (null === c) throw Error(k(395));
  p(a, c);
  return t(a, Wa);
}
var $a = v('<div hidden id="'),
  ab = v('">'),
  bb = v("</div>"),
  cb = v('<svg aria-hidden="true" style="display:none" id="'),
  db = v('">'),
  eb = v("</svg>"),
  fb = v('<math aria-hidden="true" style="display:none" id="'),
  gb = v('">'),
  hb = v("</math>"),
  ib = v('<table hidden id="'),
  jb = v('">'),
  kb = v("</table>"),
  lb = v('<table hidden><tbody id="'),
  mb = v('">'),
  nb = v("</tbody></table>"),
  ob = v('<table hidden><tr id="'),
  pb = v('">'),
  qb = v("</tr></table>"),
  rb = v('<table hidden><colgroup id="'),
  sb = v('">'),
  tb = v("</colgroup></table>");
function ub(a, b, c, d) {
  switch (c.insertionMode) {
    case 0:
    case 1:
      return p(a, $a), p(a, b.segmentPrefix), p(a, u(d.toString(16))), t(a, ab);
    case 2:
      return p(a, cb), p(a, b.segmentPrefix), p(a, u(d.toString(16))), t(a, db);
    case 3:
      return p(a, fb), p(a, b.segmentPrefix), p(a, u(d.toString(16))), t(a, gb);
    case 4:
      return p(a, ib), p(a, b.segmentPrefix), p(a, u(d.toString(16))), t(a, jb);
    case 5:
      return p(a, lb), p(a, b.segmentPrefix), p(a, u(d.toString(16))), t(a, mb);
    case 6:
      return p(a, ob), p(a, b.segmentPrefix), p(a, u(d.toString(16))), t(a, pb);
    case 7:
      return p(a, rb), p(a, b.segmentPrefix), p(a, u(d.toString(16))), t(a, sb);
    default:
      throw Error(k(397));
  }
}
function vb(a, b) {
  switch (b.insertionMode) {
    case 0:
    case 1:
      return t(a, bb);
    case 2:
      return t(a, eb);
    case 3:
      return t(a, hb);
    case 4:
      return t(a, kb);
    case 5:
      return t(a, nb);
    case 6:
      return t(a, qb);
    case 7:
      return t(a, tb);
    default:
      throw Error(k(397));
  }
}
var wb = v(
    'function $RS(a,b){a=document.getElementById(a);b=document.getElementById(b);for(a.parentNode.removeChild(a);a.firstChild;)b.parentNode.insertBefore(a.firstChild,b);b.parentNode.removeChild(b)};$RS("'
  ),
  xb = v('$RS("'),
  yb = v('","'),
  zb = v('")\x3c/script>'),
  Ab = v(
    'function $RC(a,b){a=document.getElementById(a);b=document.getElementById(b);b.parentNode.removeChild(b);if(a){a=a.previousSibling;var f=a.parentNode,c=a.nextSibling,e=0;do{if(c&&8===c.nodeType){var d=c.data;if("/$"===d)if(0===e)break;else e--;else"$"!==d&&"$?"!==d&&"$!"!==d||e++}d=c.nextSibling;f.removeChild(c);c=d}while(c);for(;b.firstChild;)f.insertBefore(b.firstChild,c);a.data="$";a._reactRetry&&a._reactRetry()}};$RC("'
  ),
  Bb = v('$RC("'),
  Cb = v('","'),
  Db = v('")\x3c/script>'),
  Eb = v(
    'function $RX(a){if(a=document.getElementById(a))a=a.previousSibling,a.data="$!",a._reactRetry&&a._reactRetry()};$RX("'
  ),
  Fb = v('$RX("'),
  Gb = v('")\x3c/script>'),
  M = Object.assign,
  Hb = Symbol.for("react.element"),
  Ib = Symbol.for("react.portal"),
  Jb = Symbol.for("react.fragment"),
  Kb = Symbol.for("react.strict_mode"),
  Lb = Symbol.for("react.profiler"),
  Mb = Symbol.for("react.provider"),
  Nb = Symbol.for("react.context"),
  Ob = Symbol.for("react.forward_ref"),
  Pb = Symbol.for("react.suspense"),
  Qb = Symbol.for("react.suspense_list"),
  Rb = Symbol.for("react.memo"),
  Sb = Symbol.for("react.lazy"),
  Tb = Symbol.for("react.scope"),
  Ub = Symbol.for("react.debug_trace_mode"),
  Vb = Symbol.for("react.legacy_hidden"),
  Wb = Symbol.for("react.default_value"),
  Xb = Symbol.iterator;
function Yb(a) {
  if (null == a) return null;
  if ("function" === typeof a) return a.displayName || a.name || null;
  if ("string" === typeof a) return a;
  switch (a) {
    case Jb:
      return "Fragment";
    case Ib:
      return "Portal";
    case Lb:
      return "Profiler";
    case Kb:
      return "StrictMode";
    case Pb:
      return "Suspense";
    case Qb:
      return "SuspenseList";
  }
  if ("object" === typeof a)
    switch (a.$$typeof) {
      case Nb:
        return (a.displayName || "Context") + ".Consumer";
      case Mb:
        return (a._context.displayName || "Context") + ".Provider";
      case Ob:
        var b = a.render;
        a = a.displayName;
        a ||
          ((a = b.displayName || b.name || ""),
          (a = "" !== a ? "ForwardRef(" + a + ")" : "ForwardRef"));
        return a;
      case Rb:
        return (
          (b = a.displayName || null), null !== b ? b : Yb(a.type) || "Memo"
        );
      case Sb:
        b = a._payload;
        a = a._init;
        try {
          return Yb(a(b));
        } catch (c) {}
    }
  return null;
}
var Zb = {};
function $b(a, b) {
  a = a.contextTypes;
  if (!a) return Zb;
  var c = {},
    d;
  for (d in a) c[d] = b[d];
  return c;
}
var N = null;
function O(a, b) {
  if (a !== b) {
    a.context._currentValue = a.parentValue;
    a = a.parent;
    var c = b.parent;
    if (null === a) {
      if (null !== c) throw Error(k(401));
    } else {
      if (null === c) throw Error(k(401));
      O(a, c);
    }
    b.context._currentValue = b.value;
  }
}
function ac(a) {
  a.context._currentValue = a.parentValue;
  a = a.parent;
  null !== a && ac(a);
}
function bc(a) {
  var b = a.parent;
  null !== b && bc(b);
  a.context._currentValue = a.value;
}
function cc(a, b) {
  a.context._currentValue = a.parentValue;
  a = a.parent;
  if (null === a) throw Error(k(402));
  a.depth === b.depth ? O(a, b) : cc(a, b);
}
function dc(a, b) {
  var c = b.parent;
  if (null === c) throw Error(k(402));
  a.depth === c.depth ? O(a, c) : dc(a, c);
  b.context._currentValue = b.value;
}
function P(a) {
  var b = N;
  b !== a &&
    (null === b
      ? bc(a)
      : null === a
      ? ac(b)
      : b.depth === a.depth
      ? O(b, a)
      : b.depth > a.depth
      ? cc(b, a)
      : dc(b, a),
    (N = a));
}
var ec = {
  isMounted: function () {
    return !1;
  },
  enqueueSetState: function (a, b) {
    a = a._reactInternals;
    null !== a.queue && a.queue.push(b);
  },
  enqueueReplaceState: function (a, b) {
    a = a._reactInternals;
    a.replace = !0;
    a.queue = [b];
  },
  enqueueForceUpdate: function () {},
};
function fc(a, b, c, d) {
  var f = void 0 !== a.state ? a.state : null;
  a.updater = ec;
  a.props = c;
  a.state = f;
  var e = { queue: [], replace: !1 };
  a._reactInternals = e;
  var g = b.contextType;
  a.context = "object" === typeof g && null !== g ? g._currentValue : d;
  g = b.getDerivedStateFromProps;
  "function" === typeof g &&
    ((g = g(c, f)),
    (f = null === g || void 0 === g ? f : M({}, f, g)),
    (a.state = f));
  if (
    "function" !== typeof b.getDerivedStateFromProps &&
    "function" !== typeof a.getSnapshotBeforeUpdate &&
    ("function" === typeof a.UNSAFE_componentWillMount ||
      "function" === typeof a.componentWillMount)
  )
    if (
      ((b = a.state),
      "function" === typeof a.componentWillMount && a.componentWillMount(),
      "function" === typeof a.UNSAFE_componentWillMount &&
        a.UNSAFE_componentWillMount(),
      b !== a.state && ec.enqueueReplaceState(a, a.state, null),
      null !== e.queue && 0 < e.queue.length)
    )
      if (
        ((b = e.queue),
        (g = e.replace),
        (e.queue = null),
        (e.replace = !1),
        g && 1 === b.length)
      )
        a.state = b[0];
      else {
        e = g ? b[0] : a.state;
        f = !0;
        for (g = g ? 1 : 0; g < b.length; g++) {
          var h = b[g];
          h = "function" === typeof h ? h.call(a, e, c, d) : h;
          null != h && (f ? ((f = !1), (e = M({}, e, h))) : M(e, h));
        }
        a.state = e;
      }
    else e.queue = null;
}
var gc = { id: 1, overflow: "" };
function hc(a, b, c) {
  var d = a.id;
  a = a.overflow;
  var f = 32 - Q(d) - 1;
  d &= ~(1 << f);
  c += 1;
  var e = 32 - Q(b) + f;
  if (30 < e) {
    var g = f - (f % 5);
    e = (d & ((1 << g) - 1)).toString(32);
    d >>= g;
    f -= g;
    return { id: (1 << (32 - Q(b) + f)) | (c << f) | d, overflow: e + a };
  }
  return { id: (1 << e) | (c << f) | d, overflow: a };
}
var Q = Math.clz32 ? Math.clz32 : ic,
  jc = Math.log,
  kc = Math.LN2;
function ic(a) {
  a >>>= 0;
  return 0 === a ? 32 : (31 - ((jc(a) / kc) | 0)) | 0;
}
function lc(a, b) {
  return (a === b && (0 !== a || 1 / a === 1 / b)) || (a !== a && b !== b);
}
var mc = "function" === typeof Object.is ? Object.is : lc,
  R = null,
  nc = null,
  oc = null,
  S = null,
  T = !1,
  pc = !1,
  U = 0,
  V = null,
  qc = 0;
function W() {
  if (null === R) throw Error(k(321));
  return R;
}
function rc() {
  if (0 < qc) throw Error(k(312));
  return { memoizedState: null, queue: null, next: null };
}
function sc() {
  null === S
    ? null === oc
      ? ((T = !1), (oc = S = rc()))
      : ((T = !0), (S = oc))
    : null === S.next
    ? ((T = !1), (S = S.next = rc()))
    : ((T = !0), (S = S.next));
  return S;
}
function tc() {
  nc = R = null;
  pc = !1;
  oc = null;
  qc = 0;
  S = V = null;
}
function uc(a, b) {
  return "function" === typeof b ? b(a) : b;
}
function vc(a, b, c) {
  R = W();
  S = sc();
  if (T) {
    var d = S.queue;
    b = d.dispatch;
    if (null !== V && ((c = V.get(d)), void 0 !== c)) {
      V.delete(d);
      d = S.memoizedState;
      do (d = a(d, c.action)), (c = c.next);
      while (null !== c);
      S.memoizedState = d;
      return [d, b];
    }
    return [S.memoizedState, b];
  }
  a = a === uc ? ("function" === typeof b ? b() : b) : void 0 !== c ? c(b) : b;
  S.memoizedState = a;
  a = S.queue = { last: null, dispatch: null };
  a = a.dispatch = wc.bind(null, R, a);
  return [S.memoizedState, a];
}
function xc(a, b) {
  R = W();
  S = sc();
  b = void 0 === b ? null : b;
  if (null !== S) {
    var c = S.memoizedState;
    if (null !== c && null !== b) {
      var d = c[1];
      a: if (null === d) d = !1;
      else {
        for (var f = 0; f < d.length && f < b.length; f++)
          if (!mc(b[f], d[f])) {
            d = !1;
            break a;
          }
        d = !0;
      }
      if (d) return c[0];
    }
  }
  a = a();
  S.memoizedState = [a, b];
  return a;
}
function wc(a, b, c) {
  if (25 <= qc) throw Error(k(301));
  if (a === R)
    if (
      ((pc = !0),
      (a = { action: c, next: null }),
      null === V && (V = new Map()),
      (c = V.get(b)),
      void 0 === c)
    )
      V.set(b, a);
    else {
      for (b = c; null !== b.next; ) b = b.next;
      b.next = a;
    }
}
function yc() {
  throw Error(k(394));
}
function zc() {}
var Bc = {
    readContext: function (a) {
      return a._currentValue;
    },
    useContext: function (a) {
      W();
      return a._currentValue;
    },
    useMemo: xc,
    useReducer: vc,
    useRef: function (a) {
      R = W();
      S = sc();
      var b = S.memoizedState;
      return null === b ? ((a = { current: a }), (S.memoizedState = a)) : b;
    },
    useState: function (a) {
      return vc(uc, a);
    },
    useInsertionEffect: zc,
    useLayoutEffect: function () {},
    useCallback: function (a, b) {
      return xc(function () {
        return a;
      }, b);
    },
    useImperativeHandle: zc,
    useEffect: zc,
    useDebugValue: zc,
    useDeferredValue: function (a) {
      W();
      return a;
    },
    useTransition: function () {
      W();
      return [!1, yc];
    },
    useId: function () {
      var a = nc.treeContext;
      var b = a.overflow;
      a = a.id;
      a = (a & ~(1 << (32 - Q(a) - 1))).toString(32) + b;
      var c = Ac;
      if (null === c) throw Error(k(404));
      b = U++;
      a = ":" + c.idPrefix + "R" + a;
      0 < b && (a += "H" + b.toString(32));
      return a + ":";
    },
    useMutableSource: function (a, b) {
      W();
      return b(a._source);
    },
    useSyncExternalStore: function (a, b, c) {
      if (void 0 === c) throw Error(k(407));
      return c();
    },
  },
  Ac = null,
  Cc =
    aa.__SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED
      .ReactCurrentDispatcher;
function Dc(a) {
  console.error(a);
}
function X() {}
function Ec(a, b, c, d, f, e, g, h, m) {
  var q = [],
    r = new Set();
  b = {
    destination: null,
    responseState: b,
    progressiveChunkSize: void 0 === d ? 12800 : d,
    status: 0,
    fatalError: null,
    nextSegmentId: 0,
    allPendingTasks: 0,
    pendingRootTasks: 0,
    completedRootSegment: null,
    abortableTasks: r,
    pingedTasks: q,
    clientRenderedBoundaries: [],
    completedBoundaries: [],
    partialBoundaries: [],
    onError: void 0 === f ? Dc : f,
    onAllReady: void 0 === e ? X : e,
    onShellReady: void 0 === g ? X : g,
    onShellError: void 0 === h ? X : h,
    onFatalError: void 0 === m ? X : m,
  };
  c = Fc(b, 0, null, c);
  c.parentFlushed = !0;
  a = Gc(b, a, null, c, r, Zb, null, gc);
  q.push(a);
  return b;
}
function Gc(a, b, c, d, f, e, g, h) {
  a.allPendingTasks++;
  null === c ? a.pendingRootTasks++ : c.pendingTasks++;
  var m = {
    node: b,
    ping: function () {
      var b = a.pingedTasks;
      b.push(m);
      1 === b.length && Hc(a);
    },
    blockedBoundary: c,
    blockedSegment: d,
    abortSet: f,
    legacyContext: e,
    context: g,
    treeContext: h,
  };
  f.add(m);
  return m;
}
function Fc(a, b, c, d) {
  return {
    status: 0,
    id: -1,
    index: b,
    parentFlushed: !1,
    chunks: [],
    children: [],
    formatContext: d,
    boundary: c,
  };
}
function Y(a, b) {
  a = a.onError;
  a(b);
}
function Ic(a, b) {
  var c = a.onShellError;
  c(b);
  c = a.onFatalError;
  c(b);
  null !== a.destination
    ? ((a.status = 2), da(a.destination, b))
    : ((a.status = 1), (a.fatalError = b));
}
function Jc(a, b, c, d, f) {
  R = {};
  nc = b;
  U = 0;
  for (a = c(d, f); pc; )
    (pc = !1), (U = 0), (qc += 1), (S = null), (a = c(d, f));
  tc();
  return a;
}
function Kc(a, b, c, d) {
  var f = c.render(),
    e = d.childContextTypes;
  if (null !== e && void 0 !== e) {
    var g = b.legacyContext;
    if ("function" !== typeof c.getChildContext) d = g;
    else {
      c = c.getChildContext();
      for (var h in c)
        if (!(h in e)) throw Error(k(108, Yb(d) || "Unknown", h));
      d = M({}, g, c);
    }
    b.legacyContext = d;
    Z(a, b, f);
    b.legacyContext = g;
  } else Z(a, b, f);
}
function Lc(a, b) {
  if (a && a.defaultProps) {
    b = M({}, b);
    a = a.defaultProps;
    for (var c in a) void 0 === b[c] && (b[c] = a[c]);
    return b;
  }
  return b;
}
function Mc(a, b, c, d, f) {
  if ("function" === typeof c)
    if (c.prototype && c.prototype.isReactComponent) {
      f = $b(c, b.legacyContext);
      var e = c.contextType;
      e = new c(d, "object" === typeof e && null !== e ? e._currentValue : f);
      fc(e, c, d, f);
      Kc(a, b, e, c);
    } else {
      e = $b(c, b.legacyContext);
      f = Jc(a, b, c, d, e);
      var g = 0 !== U;
      if (
        "object" === typeof f &&
        null !== f &&
        "function" === typeof f.render &&
        void 0 === f.$$typeof
      )
        fc(f, c, d, e), Kc(a, b, f, c);
      else if (g) {
        d = b.treeContext;
        b.treeContext = hc(d, 1, 0);
        try {
          Z(a, b, f);
        } finally {
          b.treeContext = d;
        }
      } else Z(a, b, f);
    }
  else if ("string" === typeof c)
    switch (
      ((f = b.blockedSegment),
      (e = Pa(f.chunks, c, d, a.responseState, f.formatContext)),
      (g = f.formatContext),
      (f.formatContext = za(g, c, d)),
      Nc(a, b, e),
      (f.formatContext = g),
      c)
    ) {
      case "area":
      case "base":
      case "br":
      case "col":
      case "embed":
      case "hr":
      case "img":
      case "input":
      case "keygen":
      case "link":
      case "meta":
      case "param":
      case "source":
      case "track":
      case "wbr":
        break;
      default:
        f.chunks.push(Qa, u(c), Ra);
    }
  else {
    switch (c) {
      case Vb:
      case Ub:
      case Kb:
      case Lb:
      case Jb:
        Z(a, b, d.children);
        return;
      case Qb:
        Z(a, b, d.children);
        return;
      case Tb:
        throw Error(k(343));
      case Pb:
        a: {
          c = b.blockedBoundary;
          f = b.blockedSegment;
          e = d.fallback;
          d = d.children;
          g = new Set();
          var h = {
              id: null,
              rootSegmentID: -1,
              parentFlushed: !1,
              pendingTasks: 0,
              forceClientRender: !1,
              completedSegments: [],
              byteSize: 0,
              fallbackAbortableTasks: g,
            },
            m = Fc(a, f.chunks.length, h, f.formatContext);
          f.children.push(m);
          var q = Fc(a, 0, null, f.formatContext);
          q.parentFlushed = !0;
          b.blockedBoundary = h;
          b.blockedSegment = q;
          try {
            if ((Nc(a, b, d), (q.status = 1), Oc(h, q), 0 === h.pendingTasks))
              break a;
          } catch (r) {
            (q.status = 4), Y(a, r), (h.forceClientRender = !0);
          } finally {
            (b.blockedBoundary = c), (b.blockedSegment = f);
          }
          b = Gc(a, e, c, m, g, b.legacyContext, b.context, b.treeContext);
          a.pingedTasks.push(b);
        }
        return;
    }
    if ("object" === typeof c && null !== c)
      switch (c.$$typeof) {
        case Ob:
          d = Jc(a, b, c.render, d, f);
          if (0 !== U) {
            c = b.treeContext;
            b.treeContext = hc(c, 1, 0);
            try {
              Z(a, b, d);
            } finally {
              b.treeContext = c;
            }
          } else Z(a, b, d);
          return;
        case Rb:
          c = c.type;
          d = Lc(c, d);
          Mc(a, b, c, d, f);
          return;
        case Mb:
          f = d.children;
          c = c._context;
          d = d.value;
          e = c._currentValue;
          c._currentValue = d;
          g = N;
          N = d = {
            parent: g,
            depth: null === g ? 0 : g.depth + 1,
            context: c,
            parentValue: e,
            value: d,
          };
          b.context = d;
          Z(a, b, f);
          a = N;
          if (null === a) throw Error(k(403));
          d = a.parentValue;
          a.context._currentValue = d === Wb ? a.context._defaultValue : d;
          a = N = a.parent;
          b.context = a;
          return;
        case Nb:
          d = d.children;
          d = d(c._currentValue);
          Z(a, b, d);
          return;
        case Sb:
          f = c._init;
          c = f(c._payload);
          d = Lc(c, d);
          Mc(a, b, c, d, void 0);
          return;
      }
    throw Error(k(130, null == c ? c : typeof c, ""));
  }
}
function Z(a, b, c) {
  b.node = c;
  if ("object" === typeof c && null !== c) {
    switch (c.$$typeof) {
      case Hb:
        Mc(a, b, c.type, c.props, c.ref);
        return;
      case Ib:
        throw Error(k(257));
      case Sb:
        var d = c._init;
        c = d(c._payload);
        Z(a, b, c);
        return;
    }
    if (pa(c)) {
      Pc(a, b, c);
      return;
    }
    null === c || "object" !== typeof c
      ? (d = null)
      : ((d = (Xb && c[Xb]) || c["@@iterator"]),
        (d = "function" === typeof d ? d : null));
    if (d && (d = d.call(c))) {
      c = d.next();
      if (!c.done) {
        var f = [];
        do f.push(c.value), (c = d.next());
        while (!c.done);
        Pc(a, b, f);
      }
      return;
    }
    b = Object.prototype.toString.call(c);
    throw Error(
      k(
        31,
        "[object Object]" === b
          ? "object with keys {" + Object.keys(c).join(", ") + "}"
          : b
      )
    );
  }
  "string" === typeof c
    ? "" !== c && b.blockedSegment.chunks.push(u(A(c)), Aa)
    : "number" === typeof c &&
      ((a = "" + c), "" !== a && b.blockedSegment.chunks.push(u(A(a)), Aa));
}
function Pc(a, b, c) {
  for (var d = c.length, f = 0; f < d; f++) {
    var e = b.treeContext;
    b.treeContext = hc(e, d, f);
    try {
      Nc(a, b, c[f]);
    } finally {
      b.treeContext = e;
    }
  }
}
function Nc(a, b, c) {
  var d = b.blockedSegment.formatContext,
    f = b.legacyContext,
    e = b.context;
  try {
    return Z(a, b, c);
  } catch (m) {
    if (
      (tc(),
      "object" === typeof m && null !== m && "function" === typeof m.then)
    ) {
      c = m;
      var g = b.blockedSegment,
        h = Fc(a, g.chunks.length, null, g.formatContext);
      g.children.push(h);
      a = Gc(
        a,
        b.node,
        b.blockedBoundary,
        h,
        b.abortSet,
        b.legacyContext,
        b.context,
        b.treeContext
      ).ping;
      c.then(a, a);
      b.blockedSegment.formatContext = d;
      b.legacyContext = f;
      b.context = e;
      P(e);
    } else
      throw (
        ((b.blockedSegment.formatContext = d),
        (b.legacyContext = f),
        (b.context = e),
        P(e),
        m)
      );
  }
}
function Qc(a) {
  var b = a.blockedBoundary;
  a = a.blockedSegment;
  a.status = 3;
  Rc(this, b, a);
}
function Sc(a) {
  var b = a.blockedBoundary;
  a.blockedSegment.status = 3;
  null === b
    ? (this.allPendingTasks--,
      2 !== this.status &&
        ((this.status = 2),
        null !== this.destination && this.destination.close()))
    : (b.pendingTasks--,
      b.forceClientRender ||
        ((b.forceClientRender = !0),
        b.parentFlushed && this.clientRenderedBoundaries.push(b)),
      b.fallbackAbortableTasks.forEach(Sc, this),
      b.fallbackAbortableTasks.clear(),
      this.allPendingTasks--,
      0 === this.allPendingTasks && ((a = this.onAllReady), a()));
}
function Oc(a, b) {
  if (
    0 === b.chunks.length &&
    1 === b.children.length &&
    null === b.children[0].boundary
  ) {
    var c = b.children[0];
    c.id = b.id;
    c.parentFlushed = !0;
    1 === c.status && Oc(a, c);
  } else a.completedSegments.push(b);
}
function Rc(a, b, c) {
  if (null === b) {
    if (c.parentFlushed) {
      if (null !== a.completedRootSegment) throw Error(k(389));
      a.completedRootSegment = c;
    }
    a.pendingRootTasks--;
    0 === a.pendingRootTasks &&
      ((a.onShellError = X), (b = a.onShellReady), b());
  } else
    b.pendingTasks--,
      b.forceClientRender ||
        (0 === b.pendingTasks
          ? (c.parentFlushed && 1 === c.status && Oc(b, c),
            b.parentFlushed && a.completedBoundaries.push(b),
            b.fallbackAbortableTasks.forEach(Qc, a),
            b.fallbackAbortableTasks.clear())
          : c.parentFlushed &&
            1 === c.status &&
            (Oc(b, c),
            1 === b.completedSegments.length &&
              b.parentFlushed &&
              a.partialBoundaries.push(b)));
  a.allPendingTasks--;
  0 === a.allPendingTasks && ((a = a.onAllReady), a());
}
function Hc(a) {
  if (2 !== a.status) {
    var b = N,
      c = Cc.current;
    Cc.current = Bc;
    var d = Ac;
    Ac = a.responseState;
    try {
      var f = a.pingedTasks,
        e;
      for (e = 0; e < f.length; e++) {
        var g = f[e];
        var h = a,
          m = g.blockedSegment;
        if (0 === m.status) {
          P(g.context);
          try {
            Z(h, g, g.node),
              g.abortSet.delete(g),
              (m.status = 1),
              Rc(h, g.blockedBoundary, m);
          } catch (C) {
            if (
              (tc(),
              "object" === typeof C &&
                null !== C &&
                "function" === typeof C.then)
            ) {
              var q = g.ping;
              C.then(q, q);
            } else {
              g.abortSet.delete(g);
              m.status = 4;
              var r = g.blockedBoundary,
                E = C;
              Y(h, E);
              null === r
                ? Ic(h, E)
                : (r.pendingTasks--,
                  r.forceClientRender ||
                    ((r.forceClientRender = !0),
                    r.parentFlushed && h.clientRenderedBoundaries.push(r)));
              h.allPendingTasks--;
              if (0 === h.allPendingTasks) {
                var F = h.onAllReady;
                F();
              }
            }
          } finally {
          }
        }
      }
      f.splice(0, e);
      null !== a.destination && Tc(a, a.destination);
    } catch (C) {
      Y(a, C), Ic(a, C);
    } finally {
      (Ac = d), (Cc.current = c), c === Bc && P(b);
    }
  }
}
function Uc(a, b, c) {
  c.parentFlushed = !0;
  switch (c.status) {
    case 0:
      var d = (c.id = a.nextSegmentId++);
      a = a.responseState;
      p(b, Sa);
      p(b, a.placeholderPrefix);
      a = u(d.toString(16));
      p(b, a);
      return t(b, Ta);
    case 1:
      c.status = 2;
      var f = !0;
      d = c.chunks;
      var e = 0;
      c = c.children;
      for (var g = 0; g < c.length; g++) {
        for (f = c[g]; e < f.index; e++) p(b, d[e]);
        f = Vc(a, b, f);
      }
      for (; e < d.length - 1; e++) p(b, d[e]);
      e < d.length && (f = t(b, d[e]));
      return f;
    default:
      throw Error(k(390));
  }
}
function Vc(a, b, c) {
  var d = c.boundary;
  if (null === d) return Uc(a, b, c);
  d.parentFlushed = !0;
  if (d.forceClientRender) t(b, Xa), Uc(a, b, c);
  else if (0 < d.pendingTasks) {
    d.rootSegmentID = a.nextSegmentId++;
    0 < d.completedSegments.length && a.partialBoundaries.push(d);
    var f = a.responseState;
    var e = f.nextSuspenseID++;
    f = v(f.boundaryPrefix + e.toString(16));
    d = d.id = f;
    Za(b, a.responseState, d);
    Uc(a, b, c);
  } else if (d.byteSize > a.progressiveChunkSize)
    (d.rootSegmentID = a.nextSegmentId++),
      a.completedBoundaries.push(d),
      Za(b, a.responseState, d.id),
      Uc(a, b, c);
  else {
    t(b, Ua);
    c = d.completedSegments;
    if (1 !== c.length) throw Error(k(391));
    Vc(a, b, c[0]);
  }
  return t(b, Ya);
}
function Wc(a, b, c) {
  ub(b, a.responseState, c.formatContext, c.id);
  Vc(a, b, c);
  return vb(b, c.formatContext);
}
function Xc(a, b, c) {
  for (var d = c.completedSegments, f = 0; f < d.length; f++) Yc(a, b, c, d[f]);
  d.length = 0;
  a = a.responseState;
  d = c.id;
  c = c.rootSegmentID;
  p(b, a.startInlineScript);
  a.sentCompleteBoundaryFunction
    ? p(b, Bb)
    : ((a.sentCompleteBoundaryFunction = !0), p(b, Ab));
  if (null === d) throw Error(k(395));
  c = u(c.toString(16));
  p(b, d);
  p(b, Cb);
  p(b, a.segmentPrefix);
  p(b, c);
  return t(b, Db);
}
function Yc(a, b, c, d) {
  if (2 === d.status) return !0;
  var f = d.id;
  if (-1 === f) {
    if (-1 === (d.id = c.rootSegmentID)) throw Error(k(392));
    return Wc(a, b, d);
  }
  Wc(a, b, d);
  a = a.responseState;
  p(b, a.startInlineScript);
  a.sentCompleteSegmentFunction
    ? p(b, xb)
    : ((a.sentCompleteSegmentFunction = !0), p(b, wb));
  p(b, a.segmentPrefix);
  f = u(f.toString(16));
  p(b, f);
  p(b, yb);
  p(b, a.placeholderPrefix);
  p(b, f);
  return t(b, zb);
}
function Tc(a, b) {
  n = 0;
  try {
    var c = a.completedRootSegment;
    if (null !== c && 0 === a.pendingRootTasks) {
      Vc(a, b, c);
      a.completedRootSegment = null;
      var d = a.responseState.bootstrapChunks;
      for (c = 0; c < d.length - 1; c++) p(b, d[c]);
      c < d.length && t(b, d[c]);
    }
    var f = a.clientRenderedBoundaries,
      e;
    for (e = 0; e < f.length; e++) {
      d = b;
      var g = a.responseState,
        h = f[e].id;
      p(d, g.startInlineScript);
      g.sentClientRenderFunction
        ? p(d, Fb)
        : ((g.sentClientRenderFunction = !0), p(d, Eb));
      if (null === h) throw Error(k(395));
      p(d, h);
      if (!t(d, Gb)) {
        a.destination = null;
        e++;
        f.splice(0, e);
        return;
      }
    }
    f.splice(0, e);
    var m = a.completedBoundaries;
    for (e = 0; e < m.length; e++)
      if (!Xc(a, b, m[e])) {
        a.destination = null;
        e++;
        m.splice(0, e);
        return;
      }
    m.splice(0, e);
    ba(b);
    n = 0;
    var q = a.partialBoundaries;
    for (e = 0; e < q.length; e++) {
      var r = q[e];
      a: {
        f = a;
        g = b;
        var E = r.completedSegments;
        for (h = 0; h < E.length; h++)
          if (!Yc(f, g, r, E[h])) {
            h++;
            E.splice(0, h);
            var F = !1;
            break a;
          }
        E.splice(0, h);
        F = !0;
      }
      if (!F) {
        a.destination = null;
        e++;
        q.splice(0, e);
        return;
      }
    }
    q.splice(0, e);
    var C = a.completedBoundaries;
    for (e = 0; e < C.length; e++)
      if (!Xc(a, b, C[e])) {
        a.destination = null;
        e++;
        C.splice(0, e);
        return;
      }
    C.splice(0, e);
  } finally {
    ba(b),
      0 === a.allPendingTasks &&
        0 === a.pingedTasks.length &&
        0 === a.clientRenderedBoundaries.length &&
        0 === a.completedBoundaries.length &&
        b.close();
  }
}
function Zc(a) {
  try {
    var b = a.abortableTasks;
    b.forEach(Sc, a);
    b.clear();
    null !== a.destination && Tc(a, a.destination);
  } catch (c) {
    Y(a, c), Ic(a, c);
  }
}
export function renderToReadableStream(a, b) {
  return new Promise(function (c, d) {
    var f,
      e,
      g = new Promise(function (a, b) {
        e = a;
        f = b;
      }),
      h = Ec(
        a,
        xa(
          b ? b.identifierPrefix : void 0,
          b ? b.nonce : void 0,
          b ? b.bootstrapScriptContent : void 0,
          b ? b.bootstrapScripts : void 0,
          b ? b.bootstrapModules : void 0
        ),
        ya(b ? b.namespaceURI : void 0),
        b ? b.progressiveChunkSize : void 0,
        b ? b.onError : void 0,
        e,
        function () {
          var a = new ReadableStream(
            {
              type: "direct",
              pull: function (a) {
                if (1 === h.status) (h.status = 2), da(a, h.fatalError);
                else if (2 !== h.status && null === h.destination) {
                  h.destination = a;
                  try {
                    Tc(h, a);
                  } catch (F) {
                    Y(h, F), Ic(h, F);
                  }
                }
              },
              cancel: function () {
                Zc(h);
              },
            },
            { highWaterMark: 2048 }
          );
          a.allReady = g;
          c(a);
        },
        function (a) {
          g.catch(function () {});
          d(a);
        },
        f
      );
    if (b && b.signal) {
      var m = b.signal,
        q = function () {
          Zc(h);
          m.removeEventListener("abort", q);
        };
      m.addEventListener("abort", q);
    }
    Hc(h);
  });
}
export const version = "18.1.0";
