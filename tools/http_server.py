#!/usr/bin/env python
# Many tests expect there to be an http server on port 4545 servering the deno
# root directory.
import os
from threading import Thread
import SimpleHTTPServer
import SocketServer
from util import root_path
from time import sleep

PORT = 4545
REDIRECT_PORT = 4546


def server():
    os.chdir(root_path)  # Hopefully the main thread doesn't also chdir.
    Handler = SimpleHTTPServer.SimpleHTTPRequestHandler
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
    print "Deno redirect server http://localhost:%d/ -> http://localhost:%d/" % (
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


if __name__ == '__main__':
    spawn().join()
