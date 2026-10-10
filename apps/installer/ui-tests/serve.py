#!/usr/bin/env python3
"""Serve the Installer UI and its validation harness, locally, for the browser
pane: /ui-tests/* from this folder, everything else from ../ui. Development
only; binds 127.0.0.1."""
import http.server, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
UI = os.path.join(HERE, "..", "ui")
class H(http.server.SimpleHTTPRequestHandler):
    def translate_path(self, path):
        path = path.split("?", 1)[0].split("#", 1)[0]
        if path.startswith("/ui-tests/"):
            return os.path.join(HERE, os.path.normpath(path[len("/ui-tests/"):]).lstrip("./"))
        return os.path.join(UI, os.path.normpath(path).lstrip("/"))
    def log_message(self, *a): pass
port = int(sys.argv[1]) if len(sys.argv) > 1 else 8765
http.server.ThreadingHTTPServer(("127.0.0.1", port), H).serve_forever()
