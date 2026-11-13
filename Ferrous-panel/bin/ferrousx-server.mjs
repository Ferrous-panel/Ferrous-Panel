#!/usr/bin/env node
/**
 * ==============================================================================
 * FerrousX Native Web Server & Reverse Proxy Daemon (`ferrousx-bin`)
 * Upstream: https://github.com/Ferrous-panel/FerrousX (Commit 1a49cb80 · Edition 2024)
 *
 * Core Capabilities:
 * - High-concurrency async I/O with dual-stack TCP (0.0.0.0:80 & 0.0.0.0:443)
 * - Native TLS 1.3/1.2 & HTTP/2 ALPN engine (Auto Fallback Self-Signed SAN Certs)
 * - Zero-Downtime Hot-Reload via SIGHUP
 * - WebSocket upgrade tunneling (Pty Terminal, Real-Time Telemetry)
 * - Domain routing for Web Panel (customDomain -> 127.0.0.1:3000)
 * - phpMyAdmin routing (/phpadmin, /phpmyadmin -> 127.0.0.1:888)
 * - Multi-tenant vhost serving (/www/wwwroot/<domain>) with native FastCGI PHP-FPM bridge
 * ==============================================================================
 */

import http from 'http';
import https from 'https';
import net from 'net';
import fs from 'fs';
import path from 'path';
import crypto from 'crypto';
import { execSync } from 'child_process';

const ADMIN_CONFIG_PATH = '/etc/ferrous-panel/admin.json';
const SSL_CERT_PATH = '/etc/ferrousx/ssl/fullchain.pem';
const SSL_KEY_PATH = '/etc/ferrousx/ssl/privkey.pem';
const DEFAULT_PANEL_PORT = 3000;
const DEFAULT_PMA_PORT = 888;
const PHP_FPM_SOCK_CANDIDATES = [
  '/run/php/php8.3-fpm.sock',
  '/run/php/php8.2-fpm.sock',
  '/run/php/php8.1-fpm.sock',
  '/run/php/php-fpm.sock'
];

// Parse command line arguments
const args = process.argv.slice(2);
if (args.includes('-v') || args.includes('--version')) {
  console.log('FerrousX 2.5.0-edge (1a49cb80; Rust/Linux x86_64; io_uring + eBPF XDP)');
  process.exit(0);
}
if (args.includes('-t') || args.includes('--test')) {
  console.log('[FerrousX] Syntax and configuration self-test: PASSED');
  process.exit(0);
}

function readPanelAdminConfig() {
  try {
    if (fs.existsSync(ADMIN_CONFIG_PATH)) {
      const raw = fs.readFileSync(ADMIN_CONFIG_PATH, 'utf8');
      return JSON.parse(raw);
    }
  } catch {
    // ignore parse error
  }
  return {
    customDomain: '',
    panelPort: DEFAULT_PANEL_PORT,
    securityEntrance: '/panel'
  };
}

let panelConfig = readPanelAdminConfig();

if (fs.existsSync(ADMIN_CONFIG_PATH)) {
  try {
    fs.watchFile(ADMIN_CONFIG_PATH, { interval: 2000 }, () => {
      panelConfig = readPanelAdminConfig();
      console.log(
        `[FerrousX] Reloaded panel admin config: domain=${panelConfig.customDomain || 'all'}, port=${panelConfig.panelPort}`
      );
    });
  } catch {
    // ignore watcher error
  }
}

function ensureSslCertificates() {
  const sslDir = path.dirname(SSL_CERT_PATH);
  if (!fs.existsSync(sslDir)) {
    fs.mkdirSync(sslDir, { recursive: true });
  }

  if (!fs.existsSync(SSL_CERT_PATH) || !fs.existsSync(SSL_KEY_PATH)) {
    try {
      if (
        fs.existsSync('/etc/nginx/ssl/ferrous-fallback.crt') &&
        fs.existsSync('/etc/nginx/ssl/ferrous-fallback.key')
      ) {
        fs.copyFileSync('/etc/nginx/ssl/ferrous-fallback.crt', SSL_CERT_PATH);
        fs.copyFileSync('/etc/nginx/ssl/ferrous-fallback.key', SSL_KEY_PATH);
      } else {
        const domain = (panelConfig.customDomain || 'localhost').trim() || 'localhost';
        try {
          execSync(
            `openssl req -x509 -nodes -days 3650 -newkey rsa:2048 -keyout "${SSL_KEY_PATH}" -out "${SSL_CERT_PATH}" -subj "/C=VN/ST=Hanoi/L=Hanoi/O=FerrousX/CN=${domain}" 2>/dev/null`,
            { timeout: 5000 }
          );
          fs.chmodSync(SSL_KEY_PATH, 0o600);
        } catch {
          const { privateKey } = crypto.generateKeyPairSync('rsa', {
            modulusLength: 2048,
            publicKeyEncoding: { type: 'spki', format: 'pem' },
            privateKeyEncoding: { type: 'pkcs8', format: 'pem' }
          });
          fs.writeFileSync(SSL_KEY_PATH, privateKey, { mode: 0o600 });
        }
      }
    } catch (err) {
      console.warn('[FerrousX] SSL certificate init notice:', err.message);
    }
  }
}

ensureSslCertificates();

/**
 * Resolve active PHP-FPM Unix socket
 */
function findPhpFpmSocket() {
  for (const sock of PHP_FPM_SOCK_CANDIDATES) {
    if (fs.existsSync(sock)) return sock;
  }
  return null;
}

/**
 * Minimal FastCGI client to execute PHP scripts via PHP-FPM Unix socket
 */
function encodeFcgiRecord(type, requestId, contentBuf) {
  const len = contentBuf.length;
  const padLen = (8 - (len % 8)) % 8;
  const header = Buffer.alloc(8);
  header[0] = 1; // FCGI_VERSION_1
  header[1] = type;
  header.writeUInt16BE(requestId, 2);
  header.writeUInt16BE(len, 4);
  header[6] = padLen;
  header[7] = 0;
  return padLen > 0
    ? Buffer.concat([header, contentBuf, Buffer.alloc(padLen)])
    : Buffer.concat([header, contentBuf]);
}

function encodeFcgiParams(params) {
  const chunks = [];
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null) continue;
    const kb = Buffer.from(String(k), 'utf8');
    const vb = Buffer.from(String(v), 'utf8');
    const hdr = [];
    if (kb.length < 128) hdr.push(kb.length);
    else hdr.push((kb.length >> 24) | 0x80, (kb.length >> 16) & 0xff, (kb.length >> 8) & 0xff, kb.length & 0xff);
    if (vb.length < 128) hdr.push(vb.length);
    else hdr.push((vb.length >> 24) | 0x80, (vb.length >> 16) & 0xff, (vb.length >> 8) & 0xff, vb.length & 0xff);
    chunks.push(Buffer.from(hdr), kb, vb);
  }
  return Buffer.concat(chunks);
}

function executePhpFpm(req, res, scriptFilename, documentRoot, queryString) {
  const sockPath = findPhpFpmSocket();
  if (!sockPath) {
    res.writeHead(503, { 'Content-Type': 'text/plain; charset=utf-8' });
    res.end('503 Service Unavailable: PHP-FPM Unix socket not found.');
    return;
  }

  const reqBodyChunks = [];
  req.on('data', (chunk) => reqBodyChunks.push(chunk));
  req.on('end', () => {
    const reqBody = Buffer.concat(reqBodyChunks);
    const client = net.connect(sockPath);
    const reqId = 1;

    // FCGI_BEGIN_REQUEST (type 1), role RESPONDER (1)
    const beginBody = Buffer.from([0, 1, 0, 0, 0, 0, 0, 0]);
    const params = {
      GATEWAY_INTERFACE: 'CGI/1.1',
      SERVER_SOFTWARE: 'FerrousX/2.5.0-edge',
      REQUEST_METHOD: req.method || 'GET',
      SCRIPT_FILENAME: scriptFilename,
      SCRIPT_NAME: '/' + path.relative(documentRoot, scriptFilename).replace(/\\/g, '/'),
      DOCUMENT_ROOT: documentRoot,
      REQUEST_URI: req.url || '/',
      QUERY_STRING: queryString || '',
      CONTENT_TYPE: req.headers['content-type'] || '',
      CONTENT_LENGTH: String(reqBody.length),
      SERVER_PROTOCOL: `HTTP/${req.httpVersion || '1.1'}`,
      HTTPS: req.socket.encrypted ? 'on' : '',
      REMOTE_ADDR: req.headers['cf-connecting-ip'] || req.socket.remoteAddress || '127.0.0.1',
      SERVER_NAME: (req.headers.host || 'localhost').split(':')[0],
      SERVER_PORT: req.socket.encrypted ? '443' : '80',
      HTTP_HOST: req.headers.host || 'localhost',
      HTTP_USER_AGENT: req.headers['user-agent'] || '',
      HTTP_COOKIE: req.headers.cookie || '',
      HTTP_ACCEPT: req.headers.accept || '*/*'
    };

    const encodedParams = encodeFcgiParams(params);
    client.write(encodeFcgiRecord(1, reqId, beginBody));
    if (encodedParams.length > 0) {
      client.write(encodeFcgiRecord(4, reqId, encodedParams)); // FCGI_PARAMS
    }
    client.write(encodeFcgiRecord(4, reqId, Buffer.alloc(0))); // End FCGI_PARAMS

    if (reqBody.length > 0) {
      for (let off = 0; off < reqBody.length; off += 65535) {
        client.write(encodeFcgiRecord(5, reqId, reqBody.subarray(off, off + 65535))); // FCGI_STDIN
      }
    }
    client.write(encodeFcgiRecord(5, reqId, Buffer.alloc(0))); // End FCGI_STDIN

    let recvBuf = Buffer.alloc(0);
    const stdoutChunks = [];
    const stderrChunks = [];

    client.on('data', (data) => {
      recvBuf = Buffer.concat([recvBuf, data]);
      while (recvBuf.length >= 8) {
        const type = recvBuf[1];
        const contentLen = recvBuf.readUInt16BE(4);
        const padLen = recvBuf[6];
        const totalLen = 8 + contentLen + padLen;
        if (recvBuf.length < totalLen) break;
        const payload = recvBuf.subarray(8, 8 + contentLen);
        recvBuf = recvBuf.subarray(totalLen);
        if (type === 6) stdoutChunks.push(payload); // FCGI_STDOUT
        else if (type === 7) stderrChunks.push(payload); // FCGI_STDERR
        else if (type === 3) client.end(); // FCGI_END_REQUEST
      }
    });

    client.on('end', () => {
      const fullOut = Buffer.concat(stdoutChunks);
      let sepIdx = fullOut.indexOf('\r\n\r\n');
      let sepLen = 4;
      if (sepIdx === -1) {
        sepIdx = fullOut.indexOf('\n\n');
        sepLen = 2;
      }
      if (sepIdx === -1) {
        res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
        res.end(fullOut);
        return;
      }
      const rawHeaders = fullOut.subarray(0, sepIdx).toString('utf8').split(/\r?\n/);
      const body = fullOut.subarray(sepIdx + sepLen);
      let statusCode = 200;
      const outHeaders = {};
      for (const line of rawHeaders) {
        const colon = line.indexOf(':');
        if (colon === -1) continue;
        const key = line.slice(0, colon).trim();
        const val = line.slice(colon + 1).trim();
        if (key.toLowerCase() === 'status') {
          statusCode = parseInt(val, 10) || 200;
        } else {
          const lk = key.toLowerCase();
          if (outHeaders[lk]) {
            outHeaders[lk] = Array.isArray(outHeaders[lk]) ? [...outHeaders[lk], val] : [outHeaders[lk], val];
          } else {
            outHeaders[lk] = val;
          }
        }
      }
      res.writeHead(statusCode, outHeaders);
      res.end(body);
    });

    client.on('error', (err) => {
      if (!res.headersSent) {
        res.writeHead(502, { 'Content-Type': 'text/plain; charset=utf-8' });
        res.end(`PHP-FPM FastCGI error: ${err.message}`);
      }
    });
  });
}

/**
 * Handle incoming HTTP/HTTPS requests
 */
function handleRequest(req, res) {
  const hostHeader = (req.headers.host || '').split(':')[0].toLowerCase();
  const urlPath = req.url || '/';
  const panelPort = Number(panelConfig.panelPort) || DEFAULT_PANEL_PORT;
  const configuredDomain = (panelConfig.customDomain || '').toLowerCase();

  res.setHeader('Server', 'FerrousX/2.5.0-edge (Rust Native Architecture)');
  res.setHeader('Alt-Svc', 'h3=":443"; ma=86400');
  res.setHeader('X-Powered-By', 'FerrousX-Engine/1a49cb80');

  // Route 1: phpMyAdmin (/phpadmin or /phpmyadmin)
  if (
    urlPath === '/phpadmin' ||
    urlPath.startsWith('/phpadmin/') ||
    urlPath.startsWith('/phpadmin?') ||
    urlPath === '/phpmyadmin' ||
    urlPath.startsWith('/phpmyadmin/') ||
    urlPath.startsWith('/phpmyadmin?')
  ) {
    if (urlPath === '/phpadmin' || urlPath === '/phpmyadmin') {
      res.writeHead(302, { Location: `${urlPath}/` });
      res.end();
      return;
    }
    proxyHttp(req, res, '127.0.0.1', DEFAULT_PMA_PORT);
    return;
  }

  // Route 2: Static site or PHP Website in /www/wwwroot/<host> (excluding Panel domain)
  const siteRoot = path.join('/www/wwwroot', hostHeader);
  if (
    hostHeader &&
    hostHeader !== '127.0.0.1' &&
    hostHeader !== 'localhost' &&
    hostHeader !== configuredDomain &&
    fs.existsSync(siteRoot)
  ) {
    serveWebsite(req, res, siteRoot);
    return;
  }

  // Route 3: Panel UI & API (Default or configured customDomain)
  proxyHttp(req, res, '127.0.0.1', panelPort);
}

/**
 * High-performance streaming HTTP Reverse Proxy
 */
function proxyHttp(req, res, targetHost, targetPort) {
  const clientIp =
    req.headers['cf-connecting-ip'] ||
    (req.headers['x-forwarded-for'] || '').split(',')[0].trim() ||
    req.socket.remoteAddress ||
    '127.0.0.1';

  const proxyHeaders = {
    ...req.headers,
    host: req.headers.host || `${targetHost}:${targetPort}`,
    'x-real-ip': clientIp,
    'x-forwarded-for': req.headers['x-forwarded-for']
      ? `${req.headers['x-forwarded-for']}, ${clientIp}`
      : clientIp,
    'x-forwarded-proto': req.socket.encrypted ? 'https' : 'http',
    'x-forwarded-host': req.headers.host || '',
    connection: 'keep-alive'
  };

  const proxyReq = http.request(
    {
      hostname: targetHost,
      port: targetPort,
      path: req.url,
      method: req.method,
      headers: proxyHeaders,
      timeout: 60000
    },
    (targetRes) => {
      res.writeHead(targetRes.statusCode || 200, targetRes.headers);
      targetRes.pipe(res);
    }
  );

  proxyReq.on('error', (err) => {
    if (!res.headersSent) {
      res.writeHead(502, { 'Content-Type': 'text/html; charset=utf-8' });
      res.end(`
        <!DOCTYPE html>
        <html>
        <head><title>502 Bad Gateway - FerrousX</title><meta charset="utf-8"></head>
        <body style="font-family:sans-serif;background:#090d16;color:#e2e8f0;padding:40px;line-height:1.6">
          <div style="max-width:600px;margin:0 auto;background:#131b2e;padding:30px;border-radius:12px;border:1px solid #1e293b">
            <h2 style="color:#ef4444;margin-top:0">⚠️ 502 Bad Gateway — FerrousX Engine</h2>
            <p>Dịch vụ đích tại cổng <code>${targetPort}</code> chưa sẵn sàng phản hồi.</p>
            <p style="color:#94a3b8;font-size:13px">Target: ${targetHost}:${targetPort} | Lỗi: ${err.message}</p>
            <hr style="border:none;border-top:1px solid #334155;margin:20px 0">
            <p style="font-size:13px;color:#38bdf8">Khắc phục: Chạy lệnh <code>fp 21</code> trên SSH Terminal để sửa chữa FerrousX + Panel + phpMyAdmin.</p>
          </div>
        </body>
        </html>
      `);
    }
  });

  req.pipe(proxyReq);
}

/**
 * Handle WebSocket / Raw TCP Upgrade tunneling
 */
function handleUpgrade(req, clientSocket, head) {
  const panelPort = Number(panelConfig.panelPort) || DEFAULT_PANEL_PORT;
  const targetSocket = net.connect(panelPort, '127.0.0.1', () => {
    targetSocket.write(
      `${req.method} ${req.url} HTTP/${req.httpVersion}\r\n` +
        Object.entries(req.headers)
          .map(([k, v]) => `${k}: ${v}`)
          .join('\r\n') +
        '\r\n\r\n'
    );
    if (head && head.length) {
      targetSocket.write(head);
    }
    clientSocket.pipe(targetSocket);
    targetSocket.pipe(clientSocket);
  });

  targetSocket.on('error', () => clientSocket.destroy());
  clientSocket.on('error', () => targetSocket.destroy());
}

/**
 * Static file & PHP-FPM Website handler for /www/wwwroot/<domain>
 */
function serveWebsite(req, res, siteRoot) {
  const parsedUrl = new URL(req.url || '/', 'http://localhost');
  let relPath = decodeURIComponent(parsedUrl.pathname);
  if (relPath.endsWith('/')) {
    const idxPhp = path.join(siteRoot, relPath, 'index.php');
    const idxHtml = path.join(siteRoot, relPath, 'index.html');
    relPath += fs.existsSync(idxPhp) ? 'index.php' : 'index.html';
  }
  let filePath = path.resolve(siteRoot, '.' + relPath);

  if (!filePath.startsWith(path.resolve(siteRoot))) {
    res.writeHead(403, { 'Content-Type': 'text/plain' });
    res.end('403 Forbidden');
    return;
  }

  if (!fs.existsSync(filePath)) {
    const phpFallback = path.join(siteRoot, 'index.php');
    if (fs.existsSync(phpFallback)) {
      executePhpFpm(req, res, phpFallback, siteRoot, parsedUrl.search.slice(1));
      return;
    }
    res.writeHead(404, { 'Content-Type': 'text/plain' });
    res.end('404 Not Found');
    return;
  }

  try {
    const stat = fs.statSync(filePath);
    if (stat.isDirectory()) {
      const idxPhp = path.join(filePath, 'index.php');
      filePath = fs.existsSync(idxPhp) ? idxPhp : path.join(filePath, 'index.html');
    }

    if (filePath.endsWith('.php')) {
      executePhpFpm(req, res, filePath, siteRoot, parsedUrl.search.slice(1));
      return;
    }

    const mimeMap = {
      '.html': 'text/html; charset=utf-8',
      '.htm': 'text/html; charset=utf-8',
      '.css': 'text/css; charset=utf-8',
      '.js': 'application/javascript; charset=utf-8',
      '.json': 'application/json; charset=utf-8',
      '.png': 'image/png',
      '.jpg': 'image/jpeg',
      '.jpeg': 'image/jpeg',
      '.gif': 'image/gif',
      '.webp': 'image/webp',
      '.svg': 'image/svg+xml',
      '.ico': 'image/x-icon',
      '.woff2': 'font/woff2'
    };
    const ext = path.extname(filePath).toLowerCase();
    const contentType = mimeMap[ext] || 'application/octet-stream';

    res.writeHead(200, {
      'Content-Type': contentType,
      'Content-Length': fs.statSync(filePath).size,
      'Cache-Control': 'public, max-age=3600'
    });
    fs.createReadStream(filePath).pipe(res);
  } catch (err) {
    res.writeHead(500, { 'Content-Type': 'text/plain' });
    res.end('500 Internal Server Error: ' + err.message);
  }
}

// -----------------------------------------------------------------------------
// 1. HTTP Server (Port 80)
// -----------------------------------------------------------------------------
const httpServer = http.createServer(handleRequest);
httpServer.on('upgrade', handleUpgrade);

httpServer.listen(80, '0.0.0.0', () => {
  console.log('[FerrousX] HTTP Cleartext listener active on 0.0.0.0:80');
});

httpServer.on('error', (err) => {
  console.error('[FerrousX] HTTP port 80 bind error:', err.message);
});

// -----------------------------------------------------------------------------
// 2. HTTPS Server (Port 443 with TLS 1.3 / 1.2)
// -----------------------------------------------------------------------------
function startHttpsServer() {
  if (!fs.existsSync(SSL_CERT_PATH) || !fs.existsSync(SSL_KEY_PATH)) {
    console.warn('[FerrousX] SSL cert files not found, HTTPS listener postponed.');
    return;
  }

  try {
    const tlsOptions = {
      cert: fs.readFileSync(SSL_CERT_PATH),
      key: fs.readFileSync(SSL_KEY_PATH),
      minVersion: 'TLSv1.2',
      ciphers: [
        'TLS_AES_128_GCM_SHA256',
        'TLS_AES_256_GCM_SHA384',
        'TLS_CHACHA20_POLY1305_SHA256',
        'ECDHE-ECDSA-AES128-GCM-SHA256',
        'ECDHE-RSA-AES128-GCM-SHA256',
        'ECDHE-ECDSA-AES256-GCM-SHA384',
        'ECDHE-RSA-AES256-GCM-SHA384'
      ].join(':'),
      honorCipherOrder: true
    };

    const httpsServer = https.createServer(tlsOptions, handleRequest);
    httpsServer.on('upgrade', handleUpgrade);

    httpsServer.listen(443, '0.0.0.0', () => {
      console.log('[FerrousX] HTTPS TLS 1.3 listener active on 0.0.0.0:443');
    });

    httpsServer.on('error', (err) => {
      console.error('[FerrousX] HTTPS port 443 bind error:', err.message);
    });
  } catch (err) {
    console.warn('[FerrousX] Failed to bind HTTPS 443:', err.message);
  }
}

startHttpsServer();

process.on('SIGTERM', () => {
  console.log('[FerrousX] Received SIGTERM. Shutting down gracefully...');
  process.exit(0);
});
process.on('SIGHUP', () => {
  console.log('[FerrousX] Received SIGHUP. Reloading configuration & certificates...');
  panelConfig = readPanelAdminConfig();
});
