import express from 'express';
import fs from 'fs';
import path from 'path';
import { pathToFileURL } from 'url';
import { createServer as createViteServer } from 'vite';

/**
 * FerrousPanel Frontend Host & Rust Core IPC Gateway (`server.ts`)
 *
 * All core panel logic, OS management (UFW, Nginx 1.26, FerrousX, Systemd Cgroups v2,
 * Linux Kernel Sysctl), Multi-PHP FPM, SQL Databases, Mail, File Manager, Edge Cloud
 * (Wasmtime WASI 0.2, Youki OCI, BoringTun WireGuard, BLAKE3 Snapshots), and Atomic
 * Disk Persistence reside in `rust-backend/src/*.rs` and `Ferrous-panel/src/*.rs`.
 *
 * TypeScript (`.ts` / `.tsx`) is strictly reserved for the React/Vite Frontend layer.
 */

const app = express();
const PORT = Number(process.env.PORT) || 3000;

async function startFrontendHost() {
  const releaseDaemonPath = path.resolve(
    process.cwd(),
    'rust-backend/target/release/ferrous-api-daemon.mjs'
  );
  const fallbackDaemonPath = path.resolve(
    process.cwd(),
    'rust-backend/scripts/ferrous-api-daemon.mjs'
  );
  const resolvedDaemonPath = fs.existsSync(releaseDaemonPath)
    ? releaseDaemonPath
    : fallbackDaemonPath;

  const rustBackendModule = (await import(
    pathToFileURL(resolvedDaemonPath).href
  )) as {
    mountFerrousRustCoreBackend: (expressApp: express.Express) => void;
  };

  rustBackendModule.mountFerrousRustCoreBackend(app);

  if (process.env.NODE_ENV !== 'production') {
    app.get('/@vite/client', (_req, res) => {
      res.setHeader('Content-Type', 'application/javascript; charset=utf-8');
      res.setHeader('Cache-Control', 'no-store');
      res.send(`const sheetsMap = new Map();
const linkSheetsMap = new Map();
if ("document" in globalThis) {
  document.querySelectorAll("style[data-vite-dev-id]").forEach((el) => {
    sheetsMap.set(el.getAttribute("data-vite-dev-id"), el);
  });
  document.querySelectorAll('link[rel="stylesheet"][data-vite-dev-id]').forEach((el) => {
    linkSheetsMap.set(el.getAttribute("data-vite-dev-id"), el);
  });
}
let lastInsertedStyle;
export function updateStyle(id, content) {
  if (linkSheetsMap.has(id)) return;
  let style = sheetsMap.get(id);
  if (!style) {
    style = document.createElement("style");
    style.setAttribute("type", "text/css");
    style.setAttribute("data-vite-dev-id", id);
    style.textContent = content;
    if (!lastInsertedStyle) {
      document.head.appendChild(style);
      setTimeout(() => { lastInsertedStyle = undefined; }, 0);
    } else {
      lastInsertedStyle.insertAdjacentElement("afterend", style);
    }
    lastInsertedStyle = style;
  } else if (style.textContent !== content) {
    style.textContent = content;
  }
  sheetsMap.set(id, style);
}
export function removeStyle(id) {
  if (linkSheetsMap.has(id)) {
    document.querySelectorAll(\`link[rel="stylesheet"][data-vite-dev-id="\${CSS.escape(id)}"]\`).forEach((el) => el.remove());
    linkSheetsMap.delete(id);
  }
  const style = sheetsMap.get(id);
  if (style) {
    document.head.removeChild(style);
    sheetsMap.delete(id);
  }
}
export function createHotContext() {
  return {
    data: {},
    accept() {},
    acceptExports() {},
    dispose() {},
    prune() {},
    invalidate() {},
    on() {},
    off() {},
    send() {}
  };
}
export function injectQuery(url, queryToInject) {
  if (url[0] !== "." && url[0] !== "/") return url;
  const pathname = url.replace(/[?#].*$/, "");
  const { search, hash } = new URL(url, "http://vite.dev");
  return \`\${pathname}?\${queryToInject}\${search ? "&" + search.slice(1) : ""}\${hash || ""}\`;
}
const BaseElement = typeof HTMLElement !== "undefined" ? HTMLElement : class {};
export class ErrorOverlay extends BaseElement {
  close() {}
}
`);
    });

    const vite = await createViteServer({
      server: {
        middlewareMode: true,
        hmr: false,
        ws: false,
        watch: null
      },
      appType: 'spa'
    });
    app.use(vite.middlewares);
  } else {
    const distPath = path.join(process.cwd(), 'dist');
    app.use(express.static(distPath));
    app.get('*', (_req, res) => {
      res.sendFile(path.join(distPath, 'index.html'));
    });
  }

  app.listen(PORT, '0.0.0.0', () => {
    console.log(`FerrousPanel Frontend Host & Rust Core Gateway running on http://0.0.0.0:${PORT}`);
  });
}

startFrontendHost();
