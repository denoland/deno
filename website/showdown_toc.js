// Extension loading compatible with AMD and CommonJs
(function(extension) {
  "use strict";

  if (typeof showdown === "object") {
    // global (browser or nodejs global)
    showdown.extension("toc", extension());
  } else if (typeof define === "function" && define.amd) {
    // AMD
    define("toc", extension());
  } else if (typeof exports === "object") {
    // Node, CommonJS-like
    module.exports = extension();
  } else {
    // showdown was not found so we throw
    throw Error("Could not find showdown library");
  }
})(function() {
  function getHeaderEntries(sourceHtml) {
    if (typeof window === "undefined") {
      return getHeaderEntriesInNodeJs(sourceHtml);
    } else {
      return getHeaderEntriesInBrowser(sourceHtml);
    }
  }

  function getHeaderEntriesInNodeJs(sourceHtml) {
    var cheerio = require("cheerio");
    var $ = cheerio.load(sourceHtml);
    var headers = $("h1, h2, h3, h4, h5, h6");

    var headerList = [];
    for (var i = 0; i < headers.length; i++) {
      var el = headers[i];
      headerList.push(new TocEntry(el.name, $(el).text(), $(el).attr("id")));
    }

    return headerList;
  }

  function getHeaderEntriesInBrowser(sourceHtml) {
    // Generate dummy element
    var source = document.createElement("div");
    source.innerHTML = sourceHtml;

    // Find headers
    var headers = source.querySelectorAll("h1, h2, h3, h4, h5, h6");
    var headerList = [];
    for (var i = 0; i < headers.length; i++) {
      var el = headers[i];
      headerList.push(new TocEntry(el.tagName, el.textContent, el.id));
    }

    return headerList;
  }

  function TocEntry(tagName, text, anchor) {
    this.tagName = tagName;
    this.text = text;
    this.anchor = anchor;
    this.children = [];
  }

  TocEntry.prototype.childrenToString = function() {
    if (this.children.length === 0) {
      return "";
    }
    var result = "<ul>\n";
    for (var i = 0; i < this.children.length; i++) {
      result += this.children[i].toString();
    }
    result += "</ul>\n";
    return result;
  };

  TocEntry.prototype.toString = function() {
    var result = "<li>";
    if (this.text) {
      result += '<a href="#' + this.anchor + '">' + this.text + "</a>";
    }
    result += this.childrenToString();
    result += "</li>\n";
    return result;
  };

  function sortHeader(tocEntries, level) {
    level = level || 1;
    var tagName = "H" + level,
      result = [],
      currentTocEntry;

    function push(tocEntry) {
      if (tocEntry !== undefined) {
        if (tocEntry.children.length > 0) {
          tocEntry.children = sortHeader(tocEntry.children, level + 1);
        }
        result.push(tocEntry);
      }
    }

    for (var i = 0; i < tocEntries.length; i++) {
      var tocEntry = tocEntries[i];
      if (tocEntry.tagName.toUpperCase() !== tagName) {
        if (currentTocEntry === undefined) {
          currentTocEntry = new TocEntry();
        }
        currentTocEntry.children.push(tocEntry);
      } else {
        push(currentTocEntry);
        currentTocEntry = tocEntry;
      }
    }

    push(currentTocEntry);
    return result;
  }

  return {
    type: "output",
    filter: function(sourceHtml) {
      var headerList = getHeaderEntries(sourceHtml);

      // No header found
      if (headerList.length === 0) {
        return sourceHtml;
      }

      // Sort header
      headerList = sortHeader(headerList);

      // Skip the title.
      if (headerList.length == 1) {
        headerList = headerList[0].children;
      }

      // Build result and replace all [toc]
      var result =
        '<div class="toc">\n<ul>\n' + headerList.join("") + "</ul>\n</div>\n";
      return sourceHtml.replace(/\[toc\]/gi, result);
    }
  };
});
