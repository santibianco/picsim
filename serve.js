// Minimal static server for the New Proteus runtime.
// Run from the project root:  node serve.js
// Then open http://localhost:8080  (serves the runtime/ folder).
const http = require("http");
const fs = require("fs");
const path = require("path");

const ROOT = path.join(__dirname, "runtime");
const PORT = 8080;
const MIME = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".wasm": "application/wasm",
  ".json": "application/json",
  ".css": "text/css",
  ".svg": "image/svg+xml",
};

http
  .createServer((req, res) => {
    let rel = decodeURIComponent(req.url.split("?")[0]);
    if (rel === "/") rel = "/index.html";
    const file = path.join(ROOT, rel);
    if (!file.startsWith(ROOT)) {
      res.statusCode = 403;
      return res.end("forbidden");
    }
    fs.readFile(file, (err, data) => {
      if (err) {
        res.statusCode = 404;
        return res.end("not found");
      }
      res.setHeader("Content-Type", MIME[path.extname(file)] || "application/octet-stream");
      res.end(data);
    });
  })
  .listen(PORT, () => console.log(`New Proteus runtime: http://localhost:${PORT}`));
