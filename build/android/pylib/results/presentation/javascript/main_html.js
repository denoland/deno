// Copyright 2017 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

function getArguments() {
  // Returns the URL arguments as a dictionary.
  args = {}
  var s = location.search;
  if (s) {
    var vals = s.substring(1).split('&');
    for (var i = 0; i < vals.length; i++) {
      var pair = vals[i].split('=');
      args[pair[0]] = pair[1];
    }
  }
  return args;
}

function showSuiteTable(show_the_table) {
    document.getElementById('suite-table').style.display = (
        show_the_table ? 'table' : 'none');
}

function showTestTable(show_the_table) {
    document.getElementById('test-table').style.display = (
        show_the_table ? 'table' : 'none');
}

function showTestsOfOneSuiteOnly(suite_name) {
  setTitle('Test Results of Suite: ' + suite_name)
  show_all = (suite_name == 'TOTAL')
  var testTableBlocks = document.getElementById('test-table')
      .getElementsByClassName('row_block');
  Array.prototype.slice.call(testTableBlocks)
      .forEach(function(testTableBlock) {
        if (!show_all) {
          var table_block_in_suite = (testTableBlock.firstElementChild
            .firstElementChild.firstElementChild.innerHTML)
            .startsWith(suite_name);
          if (!table_block_in_suite) {
            testTableBlock.style.display = 'none';
            return;
          }
        }
        testTableBlock.style.display = 'table-row-group';
      });
  showTestTable(true);
  showSuiteTable(false);
  window.scrollTo(0, 0);
}

function showTestsOfOneSuiteOnlyWithNewState(suite_name) {
  showTestsOfOneSuiteOnly(suite_name);
  history.pushState({suite: suite_name}, suite_name, '');
}

function showSuiteTableOnly() {
  setTitle('Suites Summary')
  showTestTable(false);
  showSuiteTable(true);
  window.scrollTo(0, 0);
}

function showSuiteTableOnlyWithReplaceState() {
  showSuiteTableOnly();
  history.replaceState({}, 'suite_table', '');
}

function setBrowserBackButtonLogic() {
  window.onpopstate = function(event) {
    if (!event.state || !event.state.suite) {
      showSuiteTableOnly();
    } else {
      showTestsOfOneSuiteOnly(event.state.suite);
    }
  };
}

function setTitle(title) {
  document.getElementById('summary-header').textContent = title;
}

function sortByColumn(head) {
  var table = head.parentNode.parentNode.parentNode;
  var rowBlocks = Array.prototype.slice.call(
      table.getElementsByTagName('tbody'));

  // Determine whether to asc or desc and set arrows.
  var headers = head.parentNode.getElementsByTagName('th');
  var headIndex = Array.prototype.slice.call(headers).indexOf(head);
  var asc = -1;
  for (var i = 0; i < headers.length; i++) {
    if (headers[i].dataset.ascSorted != 0) {
      if (headers[i].dataset.ascSorted == 1) {
          headers[i].getElementsByClassName('up')[0]
              .style.display = 'none';
      } else {
        headers[i].getElementsByClassName('down')[0]
            .style.display = 'none';
      }
      if (headers[i] == head) {
        asc = headers[i].dataset.ascSorted * -1;
      } else {
        headers[i].dataset.ascSorted = 0;
      }
      break;
    }
  }
  headers[headIndex].dataset.ascSorted = asc;
  if (asc == 1) {
      headers[headIndex].getElementsByClassName('up')[0]
          .style.display = 'inline';
  } else {
      headers[headIndex].getElementsByClassName('down')[0]
          .style.display = 'inline';
  }

  // Sort the array by the specified column number (col) and order (asc).
  rowBlocks.sort(function (a, b) {
    if (a.style.display == 'none') {
      return -1;
    } else if (b.style.display == 'none') {
      return 1;
    }
    var a_rows = Array.prototype.slice.call(a.children);
    var b_rows = Array.prototype.slice.call(b.children);
    if (head.className == "text") {
      // If sorting by text, we only compare the entry on the first row.
      var aInnerHTML = a_rows[0].children[headIndex].innerHTML;
      var bInnerHTML = b_rows[0].children[headIndex].innerHTML;
      return (aInnerHTML == bInnerHTML) ? 0 : (
          (aInnerHTML > bInnerHTML) ? asc : -1 * asc);
    } else if (head.className == "number") {
      // If sorting by number, for example, duration,
      // we will sum up the durations of different test runs
      // for one specific test case and sort by the sum.
      var avalue = 0;
      var bvalue = 0;
      a_rows.forEach(function (row, i) {
        var index = (i > 0) ? headIndex - 1 : headIndex;
        avalue += Number(row.children[index].innerHTML);
      });
      b_rows.forEach(function (row, i) {
        var index = (i > 0) ? headIndex - 1 : headIndex;
        bvalue += Number(row.children[index].innerHTML);
      });
    } else if (head.className == "flaky") {
      // Flakiness = (#total - #success - #skipped) / (#total - #skipped)
      var a_success_or_skipped = 0;
      var a_skipped = 0;
      var b_success_or_skipped = 0;
      var b_skipped = 0;
      a_rows.forEach(function (row, i) {
        var index = (i > 0) ? headIndex - 1 : headIndex;
        var status = row.children[index].innerHTML.trim();
        if (status == 'SUCCESS') {
          a_success_or_skipped += 1;
        }
        if (status == 'SKIPPED') {
          a_success_or_skipped += 1;
          a_skipped += 1;
        }
      });
      b_rows.forEach(function (row, i) {
        var index = (i > 0) ? headIndex - 1 : headIndex;
        var status = row.children[index].innerHTML.trim();
        if (status == 'SUCCESS') {
          b_success_or_skipped += 1;
        }
        if (status == 'SKIPPED') {
          b_success_or_skipped += 1;
          b_skipped += 1;
        }
      });
      var atotal_minus_skipped = a_rows.length - a_skipped;
      var btotal_minus_skipped = b_rows.length - b_skipped;

      var avalue = ((atotal_minus_skipped == 0) ? -1 :
          (a_rows.length - a_success_or_skipped) / atotal_minus_skipped);
      var bvalue = ((btotal_minus_skipped == 0) ? -1 :
          (b_rows.length - b_success_or_skipped) / btotal_minus_skipped);
    }
    return asc * (avalue - bvalue);
  });

  for (var i = 0; i < rowBlocks.length; i++) {
    table.appendChild(rowBlocks[i]);
  }
}

function sortSuiteTableByFailedTestCases() {
  sortByColumn(document.getElementById('number_fail_tests'));
}

function setTableCellsAsClickable() {
  const tableCells = document.getElementsByTagName('td');
  for(let i = 0; i < tableCells.length; i++) {
    const links = tableCells[i].getElementsByTagName('a');
    // Only make the cell clickable if there is only one link.
    if (links.length == 1) {
      tableCells[i].addEventListener('click', function() {
          links[0].click();
      });
      tableCells[i].addEventListener('mouseover', function() {
          tableCells[i].style.cursor = 'pointer';
          links[0].style.textDecoration = 'underline';
      });
      tableCells[i].addEventListener('mouseout', function() {
          tableCells[i].style.cursor = 'initial';
          links[0].style.textDecoration = 'initial';
      });
    }
  }
}
