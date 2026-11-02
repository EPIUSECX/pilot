"""Stand-in for bin/pilot used by lifecycle tests. Behaviour comes from FAKE_* environment variables."""

import json
import os
import signal
import subprocess
import sys
import time
from http.server import BaseHTTPRequestHandler, HTTPServer


def bench_path():
    return os.environ["FAKE_PATH"]


def cleanup_and_exit(child):
    def handler(signum, frame):
        if child:
            try:
                os.killpg(os.getpgid(child.pid), signal.SIGTERM)
            except OSError:
                pass
        try:
            os.remove(os.path.join(bench_path(), "pids", "bench.pid"))
        except OSError:
            pass
        os._exit(0)

    return handler


def serve(name):
    port = int(os.environ["FAKE_PORT"])
    mode = os.environ.get("FAKE_MODE", "normal")
    os.makedirs(os.path.join(bench_path(), "pids"), exist_ok=True)
    with open(os.path.join(bench_path(), "spawns.log"), "a") as spawns:
        spawns.write("spawned\n")
    with open(os.path.join(bench_path(), "pids", "bench.pid"), "w") as pid_file:
        pid_file.write(str(os.getpid()))
    if mode == "crash":
        print("Traceback: boom password=hunter2", flush=True)
        sys.exit(3)
    child = subprocess.Popen(["sleep", "300"], start_new_session=True)
    with open(os.path.join(bench_path(), "child.pid"), "w") as child_file:
        child_file.write(str(child.pid))
    if mode == "stubborn":
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
    else:
        signal.signal(signal.SIGTERM, cleanup_and_exit(child))
    if mode == "slow":
        time.sleep(300)

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            body = {"/api/v1/health": {"status": "ok"}, "/api/v1/bootstrap": {"mode": "admin", "enabled": True, "name": name}}.get(self.path)
            self.send_response(200)
            self.end_headers()
            self.wfile.write(json.dumps(body).encode() if body else b"<!doctype html><title>Pilot</title>")

        def log_message(self, *args):
            pass

    print(f"Open http://localhost:{port}/?sid=SECRETTOKEN in your browser", flush=True)
    HTTPServer(("127.0.0.1", port), Handler).serve_forever()


def stop():
    with open(os.path.join(bench_path(), "pids", "bench.pid")) as pid_file:
        os.kill(int(pid_file.read()), signal.SIGTERM)


if __name__ == "__main__":
    arguments = sys.argv[1:]
    if arguments[-1] == "start":
        serve(arguments[1])
    elif arguments[-1] == "stop":
        stop()
    elif arguments[0] == "new":
        print("Creating bench")
        print("\nGenerated admin password: Gen3rated!Pass")
        os.makedirs(os.path.join(os.environ["FAKE_BENCHES"], arguments[1]))
