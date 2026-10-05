#!/usr/bin/env python3
"""假的上游服务：记录收到的请求，返回一个固定的 Anthropic messages 响应。

用法：fake-upstream.py <端口> <日志文件>
"""
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

PORT = int(sys.argv[1])
LOG = sys.argv[2]


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_POST(self):  # noqa: N802
        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length)
        with open(LOG, "a", encoding="utf-8") as log:
            log.write(
                json.dumps(
                    {
                        "path": self.path,
                        "x-api-key": self.headers.get("x-api-key"),
                        "authorization": self.headers.get("authorization"),
                        "anthropic-version": self.headers.get("anthropic-version"),
                        "body": body.decode("utf-8", "replace")[:1500],
                    }
                )
                + "\n"
            )

        payload = json.dumps(
            {
                "id": "msg_fake_upstream",
                "type": "message",
                "role": "assistant",
                "model": "claude-fake",
                "content": [{"type": "text", "text": "pong from fake upstream"}],
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 3, "output_tokens": 4},
            }
        ).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *args):
        pass


HTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
