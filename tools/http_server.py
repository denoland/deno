#!/usr/bin/env python
# Copyright 2018 the Deno authors. All rights reserved. MIT license.
# Many tests expect there to be an http server on port 4545 servering the deno
# root directory.
import os
import sys
from threading import Thread
import SimpleHTTPServer
import SocketServer
from util import root_path
from time import sleep

PORT = 4545
REDIRECT_PORT = 4546


class ContentTypeHandler(SimpleHTTPServer.SimpleHTTPRequestHandler):
    def guess_type(self, path):
        if ".t1." in path:
            return "text/typescript"
        if ".t2." in path:
            return "video/vnd.dlna.mpeg-tts"
        if ".t3." in path:
            return "video/mp2t"
        if ".t4." in path:
            return "application/x-typescript"
        if ".j1." in path:
            return "text/javascript"
        if ".j2." in path:
            return "application/ecmascript"
        if ".j3." in path:
            return "text/ecmascript"
        if ".j4." in path:
            return "application/x-javascript"
        return SimpleHTTPServer.SimpleHTTPRequestHandler.guess_type(self, path)


def server():
    os.chdir(root_path)  # Hopefully the main thread doesn't also chdir.
    Handler = ContentTypeHandler
    Handler.extensions_map.update({
        ".ts": "application/typescript",
        ".js": "application/javascript",
        ".json": "application/json",
    })
    SocketServer.TCPServer.allow_reuse_address = True
    s = SocketServer.TCPServer(("", PORT), Handler)
    print "Deno test server http://localhost:%d/" % PORT
    return s


def redirect_server():
    os.chdir(root_path)
    target_host = "http://localhost:%d" % PORT

    class RedirectHandler(SimpleHTTPServer.SimpleHTTPRequestHandler):
        def do_GET(self):
            self.send_response(301)
            self.send_header('Location', target_host + self.path)
            self.end_headers()

    Handler = RedirectHandler
    SocketServer.TCPServer.allow_reuse_address = True
    s = SocketServer.TCPServer(("", REDIRECT_PORT), Handler)
    print "redirect server http://localhost:%d/ -> http://localhost:%d/" % (
        REDIRECT_PORT, PORT)
    return s


def spawn():
    # Main http server
    s = server()
    thread = Thread(target=s.serve_forever)
    thread.daemon = True
    thread.start()
    # Redirect server
    rs = redirect_server()
    r_thread = Thread(target=rs.serve_forever)
    r_thread.daemon = True
    r_thread.start()
    sleep(1)  # TODO I'm too lazy to figure out how to do this properly.
    return thread


def main():
    try:
        thread = spawn()
        while thread.is_alive():
            sleep(10)
    except KeyboardInterrupt:
        pass
    sys.exit(1)


if __name__ == '__main__':
    main()
