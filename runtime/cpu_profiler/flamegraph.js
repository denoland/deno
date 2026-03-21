// Copyright 2018-2026 the Deno authors. MIT license.
var details,
  searchbtn,
  unzoombtn,
  matchedtxt,
  svg,
  searching,
  frames,
  total_samples,
  known_font_width;
var orig_height, detailsEl, matchedEl, update_for_resize;
function init(evt) {
  detailsEl = document.getElementById("details");
  details = detailsEl.firstChild;
  searchbtn = document.getElementById("search");
  unzoombtn = document.getElementById("unzoom");
  matchedEl = document.getElementById("matched");
  matchedtxt = matchedEl;
  svg = document.getElementsByTagName("svg")[0];
  frames = document.getElementById("frames");
  total_samples = parseInt(frames.attributes.total_samples.value);
  known_font_width = get_monospace_width(frames);
  searching = 0;
  inverted = false;
  var cb = document.getElementById("invert_cb");
  if (cb) {
    cb.addEventListener("change", function () {
      toggle_invert();
    });
  }
  orig_height = parseFloat(frames.getAttribute("fg:content_height"));
  // Fluid: fill viewport width and height
  svg.removeAttribute("width");
  update_for_resize = function () {
    // Width
    frames.attributes.width.value = svg.width.baseVal.value - xpad * 2;
    var svgWidth = svg.width.baseVal.value;
    searchbtn.attributes.x.value = svgWidth - xpad;
    matchedEl.attributes.x.value = svgWidth - xpad;
    // Height: use viewport height if larger than content
    var vh = window.innerHeight;
    var h = Math.max(orig_height, vh);
    svg.setAttribute("height", h);
    svg.setAttribute("viewBox", "0 0 " + svgWidth + " " + h);
    // Shift frames down so they sit at the bottom (normal) or top (inverted)
    var extraSpace = h - orig_height;
    if (extraSpace > 0 && !inverted) {
      frames.setAttribute("transform", "translate(0," + extraSpace + ")");
    } else {
      frames.removeAttribute("transform");
    }
    // Reposition details/matched bar at bottom
    var detailsY = h - 15;
    detailsEl.attributes.y.value = detailsY;
    matchedEl.attributes.y.value = detailsY;
    // Update text
    update_text_for_elements(frames.children);
  };
  window.addEventListener("resize", update_for_resize);
  setTimeout(function () {
    unzoom();
    update_for_resize();
  }, 0);
}
window.addEventListener("click", function (e) {
  var target = find_group(e.target);
  if (target) {
    if (target.classList.contains("parent")) unzoom();
    zoom(target);
  } else if (e.target.id == "unzoom") {
    unzoom();
  } else if (e.target.id == "search") {
    search_prompt();
  }
}, false);
window.addEventListener("mouseover", function (e) {
  var target = find_group(e.target);
  if (target) details.nodeValue = nametype + " " + g_to_text(target);
}, false);
window.addEventListener("mouseout", function (e) {
  var target = find_group(e.target);
  if (target) details.nodeValue = "\u00a0";
}, false);
window.addEventListener("keydown", function (e) {
  if (e.keyCode === 114 || (e.ctrlKey && e.keyCode === 70)) {
    e.preventDefault();
    search_prompt();
  }
}, false);
function find_child(node, selector) {
  var c = node.querySelectorAll(selector);
  if (c.length) return c[0];
}
function find_group(node) {
  var parent = node.parentElement;
  if (!parent) return;
  if (parent.id == "frames") return node;
  return find_group(parent);
}
function orig_save(e, attr, val) {
  if (e.attributes["fg:orig_" + attr] != undefined) return;
  if (e.attributes[attr] == undefined) return;
  if (val == undefined) val = e.attributes[attr].value;
  e.setAttribute("fg:orig_" + attr, val);
}
function orig_load(e, attr) {
  if (e.attributes["fg:orig_" + attr] == undefined) return;
  e.attributes[attr].value = e.attributes["fg:orig_" + attr].value;
  e.removeAttribute("fg:orig_" + attr);
}
function g_to_text(e) {
  return find_child(e, "title").firstChild.nodeValue;
}
function g_to_func(e) {
  return g_to_text(e);
}
function get_monospace_width(frames) {
  if (!frames.children[0]) return 0;
  var text = find_child(frames.children[0], "text");
  if (!text) return 0;
  var orig = text.textContent;
  text.textContent = "!";
  var w1 = text.getComputedTextLength();
  text.textContent = "W";
  var w2 = text.getComputedTextLength();
  text.textContent = orig;
  return (w1 === w2) ? w1 : 0;
}
function update_text_for_elements(elements) {
  if (known_font_width === 0) {
    for (var i = 0; i < elements.length; i++) update_text(elements[i]);
    return;
  }
  var attrs = [];
  for (var i = 0; i < elements.length; i++) {
    var e = elements[i];
    var r = find_child(e, "rect");
    var t = find_child(e, "text");
    if (!r || !t) {
      attrs.push(null);
      continue;
    }
    var w =
      parseFloat(r.attributes.width.value) * frames.attributes.width.value /
        100 - 3;
    var txt = find_child(e, "title").textContent.replace(/\([^(]*\)$/, "");
    var newX = format_percent(
      parseFloat(r.attributes.x.value) +
        100 * 3 / frames.attributes.width.value,
    );
    if (w < 2 * known_font_width) {
      attrs.push([newX, ""]);
      continue;
    }
    if (txt.length * known_font_width < w) {
      attrs.push([newX, txt]);
      continue;
    }
    var len = Math.floor(w / known_font_width) - 2;
    attrs.push([newX, txt.substring(0, len) + ".."]);
  }
  for (var i = 0; i < elements.length; i++) {
    if (!attrs[i]) continue;
    var t = find_child(elements[i], "text");
    if (t) {
      t.attributes.x.value = attrs[i][0];
      t.textContent = attrs[i][1];
    }
  }
}
function update_text(e) {
  var r = find_child(e, "rect"), t = find_child(e, "text");
  if (!r || !t) return;
  var w =
    parseFloat(r.attributes.width.value) * frames.attributes.width.value / 100 -
    3;
  var txt = find_child(e, "title").textContent.replace(/\([^(]*\)$/, "");
  t.attributes.x.value = format_percent(
    parseFloat(r.attributes.x.value) + 100 * 3 / frames.attributes.width.value,
  );
  if (w < 2 * fontsize * fontwidth) {
    t.textContent = "";
    return;
  }
  t.textContent = txt;
  if (t.getComputedTextLength() < w) return;
  for (var x = txt.length - 2; x > 0; x--) {
    if (t.getSubStringLength(0, x + 2) <= w) {
      t.textContent = txt.substring(0, x) + "..";
      return;
    }
  }
  t.textContent = "";
}
function zoom_reset(e) {
  if (e.tagName == "rect") {
    e.attributes.x.value = format_percent(
      100 * parseInt(e.attributes["fg:x"].value) / total_samples,
    );
    e.attributes.width.value = format_percent(
      100 * parseInt(e.attributes["fg:w"].value) / total_samples,
    );
  }
  if (e.childNodes == undefined) return;
  for (var i = 0, c = e.childNodes; i < c.length; i++) zoom_reset(c[i]);
}
function zoom_child(e, x, zoomed_width_samples) {
  if (e.tagName == "text") {
    var px = parseFloat(find_child(e.parentNode, "rect[x]").attributes.x.value);
    e.attributes.x.value = format_percent(
      px + 100 * 3 / frames.attributes.width.value,
    );
  } else if (e.tagName == "rect") {
    e.attributes.x.value = format_percent(
      100 * (parseInt(e.attributes["fg:x"].value) - x) / zoomed_width_samples,
    );
    e.attributes.width.value = format_percent(
      100 * parseInt(e.attributes["fg:w"].value) / zoomed_width_samples,
    );
  }
  if (e.childNodes == undefined) return;
  for (var i = 0, c = e.childNodes; i < c.length; i++) {
    zoom_child(c[i], x, zoomed_width_samples);
  }
}
function zoom_parent(e) {
  if (e.attributes) {
    if (e.attributes.x != undefined) e.attributes.x.value = "0.0%";
    if (e.attributes.width != undefined) e.attributes.width.value = "100.0%";
  }
  if (e.childNodes == undefined) return;
  for (var i = 0, c = e.childNodes; i < c.length; i++) zoom_parent(c[i]);
}
function zoom(node) {
  var attr = find_child(node, "rect").attributes;
  var width = parseInt(attr["fg:w"].value);
  var xmin = parseInt(attr["fg:x"].value);
  var xmax = xmin + width;
  var ymin = parseFloat(attr.y.value);
  unzoombtn.classList.remove("hide");
  var el = frames.children;
  var to_update = [];
  for (var i = 0; i < el.length; i++) {
    var e = el[i];
    var a = find_child(e, "rect").attributes;
    var ex = parseInt(a["fg:x"].value);
    var ew = parseInt(a["fg:w"].value);
    var upstack = inverted
      ? parseFloat(a.y.value) < ymin
      : parseFloat(a.y.value) > ymin;
    if (upstack) {
      if (ex <= xmin && (ex + ew) >= xmax) {
        e.classList.add("parent");
        zoom_parent(e);
        to_update.push(e);
      } else e.classList.add("hide");
    } else {
      if (ex < xmin || ex >= xmax) e.classList.add("hide");
      else {
        zoom_child(e, xmin, width);
        to_update.push(e);
      }
    }
  }
  update_text_for_elements(to_update);
}
function unzoom() {
  unzoombtn.classList.add("hide");
  var el = frames.children;
  for (var i = 0; i < el.length; i++) {
    el[i].classList.remove("parent");
    el[i].classList.remove("hide");
    zoom_reset(el[i]);
  }
  update_text_for_elements(el);
}
function reset_search() {
  var el = document.querySelectorAll("#frames rect");
  for (var i = 0; i < el.length; i++) orig_load(el[i], "fill");
}
function search_prompt() {
  if (!searching) {
    var term = prompt("Enter a search term (regexp allowed, eg: ^fib)", "");
    if (term != null) search(term);
  } else {
    reset_search();
    searching = 0;
    searchbtn.classList.remove("show");
    searchbtn.firstChild.nodeValue = "Search";
    matchedtxt.classList.add("hide");
    matchedtxt.firstChild.nodeValue = "";
  }
}
function search(term) {
  var re = new RegExp(term);
  var el = frames.children;
  var matches = {}, maxwidth = 0;
  for (var i = 0; i < el.length; i++) {
    var e = el[i];
    if (e.classList.contains("hide") || e.classList.contains("parent")) {
      continue;
    }
    var func = g_to_func(e);
    var rect = find_child(e, "rect");
    if (!func || !rect) continue;
    var w = parseInt(rect.attributes["fg:w"].value);
    if (w > maxwidth) maxwidth = w;
    if (func.match(re)) {
      var x = parseInt(rect.attributes["fg:x"].value);
      orig_save(rect, "fill");
      rect.attributes.fill.value = searchcolor;
      if (matches[x] == undefined) matches[x] = w;
      else if (w > matches[x]) matches[x] = w;
      searching = 1;
    }
  }
  if (!searching) return;
  searchbtn.classList.add("show");
  searchbtn.firstChild.nodeValue = "Reset Search";
  var count = 0, lastx = -1, lastw = 0;
  var keys = [];
  for (var k in matches) if (matches.hasOwnProperty(k)) keys.push(k);
  keys.sort(function (a, b) {
    return a - b;
  });
  for (var k in keys) {
    var x = parseInt(keys[k]), w = matches[keys[k]];
    if (x >= lastx + lastw) {
      count += w;
      lastx = x;
      lastw = w;
    }
  }
  matchedtxt.classList.remove("hide");
  var pct = 100 * count / maxwidth;
  if (pct != 100) pct = pct.toFixed(1);
  matchedtxt.firstChild.nodeValue = "Matched: " + pct + "%";
}
function toggle_invert() {
  inverted = !inverted;
  unzoom();
  var md = parseInt(frames.attributes["fg:max_depth"].value);
  var fh = parseInt(frames.attributes["fg:frame_height"].value);
  var ypt = parseInt(frames.attributes["fg:y_pad_top"].value);
  var el = frames.children;
  for (var i = 0; i < el.length; i++) {
    var r = find_child(el[i], "rect");
    var t = find_child(el[i], "text");
    if (!r) continue;
    var oy = parseInt(r.attributes["fg:y"].value);
    // fg:y stores the normal (non-inverted) Y; flip only when inverted
    var ny = inverted ? ypt + md * fh - (oy - ypt) : oy;
    r.attributes.y.value = ny;
    if (t) t.attributes.y.value = ny + fh - 4;
  }
  update_for_resize();
}
function format_percent(n) {
  return n.toFixed(4) + "%";
}
