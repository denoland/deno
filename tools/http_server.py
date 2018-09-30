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


def server():
    os.chdir(root_path)  # Hopefully the main thread doesn't also chdir.
    Handler = SimpleHTTPServer.SimpleHTTPRequestHandler
    SocketServer.TCPServer.allow_reuse_address = True
    s = SocketServer.TCPServer(("", PORT), Handler)
    print "Deno test server http://localhost:%d/" % PORT
    return s


def spawn():
    s = server()
    thread = Thread(target=s.serve_forever)
    thread.daemon = True
    thread.start()
    sleep(1)  # TODO I'm too lazy to figure out how to do this properly.
    return thread


if __name__ == '__main__':
    s = server()
    s.serve_forever()
