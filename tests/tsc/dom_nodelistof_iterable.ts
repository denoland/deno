const div = document.createElement("div");
div.innerHTML = "<p></p><p></p>";

for (const child of div.childNodes) {
  child.nodeName;
}
