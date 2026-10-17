import { createServer, request as httpRequest } from 'node:http';
import { createReadStream, existsSync, statSync } from 'node:fs';
import { extname, resolve, sep } from 'node:path';
import { pathToFileURL } from 'node:url';

const types = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8', '.svg': 'image/svg+xml', '.png': 'image/png', '.woff2': 'font/woff2' };

export function createE2EGateway({ adminDir = 'apps/admin/dist', apiPort = 28080, webPort = 23000 } = {}) {
  const root = resolve(adminDir);
  return createServer((incoming, outgoing) => {
    let pathname;
    try {
      if (/%(?:2e|2f|5c)/i.test(incoming.url ?? '')) throw new Error('unsafe encoded path');
      pathname = decodeURIComponent(new URL(incoming.url ?? '/', 'http://gateway.invalid').pathname);
      if (pathname.includes('\\') || pathname.split('/').includes('..')) throw new Error('unsafe path');
    } catch {
      outgoing.writeHead(400).end('bad request');
      return;
    }

    if (pathname === '/admin' || pathname.startsWith('/admin/')) {
      const relative = pathname.replace(/^\/admin\/?/, '');
      const candidate = resolve(root, relative || 'index.html');
      if (candidate !== root && !candidate.startsWith(root + sep)) {
        outgoing.writeHead(400).end('bad request');
        return;
      }
      const file = existsSync(candidate) && statSync(candidate).isFile() ? candidate : resolve(root, 'index.html');
      if (!existsSync(file)) { outgoing.writeHead(404).end('admin bundle missing'); return; }
      outgoing.writeHead(200, { 'Content-Type': types[extname(file)] ?? 'application/octet-stream', 'X-Content-Type-Options': 'nosniff' });
      createReadStream(file).pipe(outgoing);
      return;
    }

    const targetPort = pathname.startsWith('/api/') ? apiPort : webPort;
    const upstream = httpRequest({ hostname: '127.0.0.1', port: targetPort, method: incoming.method, path: incoming.url, headers: { ...incoming.headers, host: incoming.headers.host, 'x-forwarded-host': incoming.headers.host, 'x-forwarded-proto': 'http' } }, (response) => {
      outgoing.writeHead(response.statusCode ?? 502, response.headers);
      response.pipe(outgoing);
    });
    upstream.on('error', () => { if (!outgoing.headersSent) outgoing.writeHead(502); outgoing.end('upstream unavailable'); });
    incoming.pipe(upstream);
  });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const server = createE2EGateway();
  server.listen(28088, '127.0.0.1', () => process.stdout.write('E2E gateway ready on 127.0.0.1:28088\n'));
  process.on('SIGTERM', () => server.close(() => process.exit(0)));
  process.on('SIGINT', () => server.close(() => process.exit(0)));
}
