"""An unrelated service that answers Pilot's identity routes, to prove the answer alone is not trusted."""

import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        body = {"/health": {"status": "ok"}, "/bootstrap": {"mode": "admin", "enabled": True, "name": "main"}}.get(self.path, {})
        self.send_response(200)
        self.end_headers()
        self.wfile.write(json.dumps(body).encode())

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
