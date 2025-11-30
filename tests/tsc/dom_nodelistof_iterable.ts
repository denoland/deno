const div = document.createElement("div");
div.innerHTML = "<p>a</p><p>b</p>";

for (const child of div.childNodes) {
  void child.nodeName;
}


