import tls from "node:tls"
    let socket = tls.connect(443, "google.com", {servername: "google.com"});
    socket.on("data", function(response) {
        console.log(response);
    });
    socket.setEncoding("utf8");
    socket.write(`GET / HTTP/1.1\nHost: www.google.com\n\n`);
