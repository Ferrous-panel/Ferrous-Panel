use crate::error::PanelError;
use crate::state::AppState;
use crate::system::cmd::AllowedBinary;
use crate::webserver::{atomic_write_file, CommandExecutor};
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const FERROUS_ADMIN_HTML: &str = r##"<!DOCTYPE html>
<html lang="vi" class="dark">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>FerrousAdmin — Pure-Rust SQL Studio (/phpadmin/)</title>
  <style>
    :root {
      --bg: #090d16;
      --panel: #111827;
      --panel-2: #0f172a;
      --border: #1e293b;
      --text: #e2e8f0;
      --muted: #94a3b8;
      --accent: #f97316;
      --accent-2: #ea580c;
      --ok: #10b981;
      --err: #ef4444;
      --blue: #3b82f6;
      --mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
      --sans: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: var(--sans);
      background: var(--bg);
      color: var(--text);
      min-height: 100vh;
      display: flex;
      flex-direction: column;
    }
    header {
      background: var(--panel);
      border-bottom: 1px solid var(--border);
      padding: 12px 18px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      flex-wrap: wrap;
    }
    .brand { display: flex; align-items: center; gap: 10px; font-weight: 700; font-size: 15px; }
    .badge {
      font-family: var(--mono);
      font-size: 11px;
      padding: 3px 8px;
      border-radius: 6px;
      background: rgba(249,115,22,0.14);
      color: #fb923c;
      border: 1px solid rgba(249,115,22,0.35);
    }
    .badge-ok {
      background: rgba(16,185,129,0.14);
      color: #34d399;
      border: 1px solid rgba(16,185,129,0.35);
    }
    .row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
    button {
      font-family: inherit;
      font-size: 12px;
      font-weight: 600;
      padding: 7px 12px;
      border-radius: 7px;
      border: 1px solid var(--border);
      background: #1e293b;
      color: var(--text);
      cursor: pointer;
    }
    button:hover { background: #334155; }
    button.primary { background: var(--accent); border-color: var(--accent-2); color: #fff; }
    button.primary:hover { background: var(--accent-2); }
    button.danger { background: rgba(239,68,68,0.15); border-color: rgba(239,68,68,0.4); color: #fca5a5; }
    button.danger:hover { background: rgba(239,68,68,0.28); }
    input, select, textarea {
      font-family: var(--mono);
      font-size: 12px;
      background: #020617;
      color: var(--text);
      border: 1px solid var(--border);
      border-radius: 7px;
      padding: 8px 10px;
      width: 100%;
    }
    textarea { min-height: 140px; resize: vertical; }
    .layout {
      display: grid;
      grid-template-columns: 270px 1fr;
      flex: 1;
      min-height: 0;
    }
    @media (max-width: 900px) {
      .layout { grid-template-columns: 1fr; }
    }
    aside {
      background: var(--panel-2);
      border-right: 1px solid var(--border);
      padding: 14px;
      overflow-y: auto;
    }
    main {
      padding: 18px;
      overflow-y: auto;
    }
    .db-item {
      padding: 9px 10px;
      border-radius: 8px;
      border: 1px solid transparent;
      margin-bottom: 6px;
      cursor: pointer;
      background: rgba(15,23,42,0.5);
    }
    .db-item:hover { border-color: #334155; }
    .db-item.active { border-color: var(--accent); background: rgba(249,115,22,0.12); }
    .db-meta { font-family: var(--mono); font-size: 11px; color: var(--muted); margin-top: 3px; }
    .tbl-list { margin: 6px 0 0 10px; border-left: 1px solid var(--border); padding-left: 8px; }
    .tbl-item {
      font-family: var(--mono);
      font-size: 11.5px;
      padding: 4px 7px;
      border-radius: 5px;
      cursor: pointer;
      color: #cbd5e1;
      display: flex;
      justify-content: space-between;
    }
    .tbl-item:hover, .tbl-item.active { background: #1e293b; color: #fb923c; }
    .tabs { display: flex; gap: 6px; border-bottom: 1px solid var(--border); padding-bottom: 10px; margin-bottom: 14px; flex-wrap: wrap; }
    .tab.active { background: var(--accent); border-color: var(--accent-2); color: #fff; }
    .card {
      background: var(--panel);
      border: 1px solid var(--border);
      border-radius: 10px;
      padding: 16px;
      margin-bottom: 14px;
    }
    table { width: 100%; border-collapse: collapse; font-family: var(--mono); font-size: 12px; }
    th, td { border-bottom: 1px solid var(--border); padding: 8px 10px; text-align: left; vertical-align: top; }
    th { background: #0f172a; color: #94a3b8; font-weight: 600; position: sticky; top: 0; }
    .table-wrap { overflow-x: auto; max-height: 460px; border: 1px solid var(--border); border-radius: 8px; }
    .login-box {
      max-width: 390px;
      margin: 72px auto;
      background: var(--panel);
      border: 1px solid var(--border);
      border-radius: 12px;
      padding: 24px;
    }
    .notice {
      padding: 10px 12px;
      border-radius: 8px;
      font-size: 12px;
      margin-bottom: 12px;
      font-family: var(--mono);
    }
    .notice.ok { background: rgba(16,185,129,0.14); border: 1px solid rgba(16,185,129,0.4); color: #6ee7b7; }
    .notice.err { background: rgba(239,68,68,0.14); border: 1px solid rgba(239,68,68,0.4); color: #fca5a5; }
    .grid-2 { display: grid; grid-template-columns: repeat(auto-fit, minmax(210px, 1fr)); gap: 10px; }
    .muted { color: var(--muted); font-size: 12px; }
    label { display: block; font-size: 11.5px; color: var(--muted); margin-bottom: 4px; }
    .hidden { display: none !important; }
    .modal-backdrop {
      position: fixed;
      inset: 0;
      background: rgba(2, 6, 23, 0.76);
      display: flex;
      align-items: center;
      justify-content: center;
      z-index: 1000;
      padding: 16px;
    }
    .modal-card {
      width: 100%;
      max-width: 440px;
      background: var(--panel);
      border: 1px solid var(--border);
      border-radius: 12px;
      padding: 18px;
      box-shadow: 0 20px 40px rgba(0, 0, 0, 0.5);
    }
  </style>
</head>
<body>
  <header>
    <div class="brand">
      <span>FerrousAdmin</span>
      <span class="badge">100% Pure Rust · Zero PHP</span>
      <span class="badge badge-ok" id="engineStatus">Actix-Web + SQLite WAL + MariaDB + PostgreSQL</span>
    </div>
    <div class="row" id="headerActions">
      <a href="/phpmyadmin/" style="font-size:12px;font-weight:600;padding:7px 12px;border-radius:7px;border:1px solid rgba(59,130,246,0.45);background:rgba(59,130,246,0.16);color:#93c5fd;text-decoration:none;">⇄ phpMyAdmin Chính thức (/phpmyadmin/) ↗</a>
      <span class="muted" id="userLabel"></span>
      <button type="button" id="btnRefresh" class="hidden">Làm mới</button>
      <button type="button" id="btnLogout" class="danger hidden">Đăng xuất</button>
    </div>
  </header>

  <section id="loginView" class="login-box">
    <h2 style="margin:0 0 6px;font-size:18px;">Đăng nhập FerrousAdmin</h2>
    <p class="muted" style="margin:0 0 16px;">Quản trị SQL thuần Rust (thay thế phpMyAdmin PHP). Sử dụng tài khoản quản trị FerrousPanel (Argon2id).</p>
    <div id="loginError" class="notice err hidden"></div>
    <form id="loginForm" style="display:flex;flex-direction:column;gap:12px;">
      <div>
        <label for="loginUser">Tài khoản</label>
        <input id="loginUser" type="text" value="admin_ferrous" required autocomplete="username" />
      </div>
      <div>
        <label for="loginPass">Mật khẩu</label>
        <input id="loginPass" type="password" required autocomplete="current-password" />
      </div>
      <button type="submit" class="primary">Đăng nhập SQL Studio</button>
    </form>
  </section>

  <div id="appView" class="layout hidden">
    <aside>
      <div class="row" style="justify-content:space-between;margin-bottom:10px;">
        <strong style="font-size:13px;">Cơ sở dữ liệu</strong>
        <button type="button" class="primary" id="btnShowCreateDb">+ Tạo DB</button>
      </div>
      <div id="dbList"></div>
    </aside>
    <main>
      <div id="banner" class="notice hidden"></div>
      <div class="tabs" id="mainTabs">
        <button type="button" class="tab active" data-tab="databases">Databases</button>
        <button type="button" class="tab" data-tab="structure">Structure (Bảng &amp; Cột)</button>
        <button type="button" class="tab" data-tab="browse">Browse &amp; Sửa Dòng</button>
        <button type="button" class="tab" data-tab="insert">Chèn Dòng (Insert)</button>
        <button type="button" class="tab" data-tab="sql">SQL Console</button>
        <button type="button" class="tab" data-tab="new_table">Tạo Bảng</button>
        <button type="button" class="tab" data-tab="export">Export .SQL</button>
        <button type="button" class="tab" data-tab="import">Import .SQL</button>
        <button type="button" class="tab" data-tab="server_cfg">Cấu hình MySQL / PMA</button>
      </div>
      <div id="tabContent"></div>
    </main>
  </div>

  <div id="pmaDialog" class="modal-backdrop hidden">
    <div class="modal-card">
      <h3 id="pmaDialogTitle" style="margin:0 0 8px;font-size:15px;"></h3>
      <p id="pmaDialogMsg" class="muted" style="margin:0 0 12px;"></p>
      <div id="pmaDialogInputWrap" class="hidden" style="margin-bottom:12px;">
        <input id="pmaDialogInput" type="text" />
      </div>
      <div class="row" style="justify-content:flex-end;">
        <button type="button" id="pmaDialogCancel">Hủy</button>
        <button type="button" id="pmaDialogOk" class="primary">Xác nhận</button>
      </div>
    </div>
  </div>

  <script>
    (function () {
      const path = window.location.pathname || '/';
      const apiBase = path.startsWith('/phpadmin')
        ? '/phpadmin/api'
        : path.startsWith('/phpmyadmin')
        ? '/phpmyadmin/api'
        : '/api';
      const urlParams = new URLSearchParams(window.location.search || '');
      const initialDbParam = urlParams.get('db') || '';
      const initialTableParam = urlParams.get('table') || '';

      let databases = [];
      let activeDbId = initialDbParam;
      let activeTable = initialTableParam;
      let activeTab = initialTableParam ? 'browse' : (initialDbParam ? 'structure' : 'databases');
      let browseLimit = 100;
      let browseOffset = 0;
      let lastQueryResult = null;
      let lastExportSql = '';
      let mysqlConfig = null;
      let authToken = sessionStorage.getItem('fp_pma_token') || localStorage.getItem('fp_token') || '';

      function esc(v) {
        return String(v === null || v === undefined ? '' : v)
          .replace(/&/g, '&amp;')
          .replace(/</g, '&lt;')
          .replace(/>/g, '&gt;')
          .replace(/"/g, '&quot;')
          .replace(/'/g, '&#39;');
      }

      function sqlQuoteId(ident, isMysql) {
        const clean = String(ident || '').replace(/[`"]/g, '');
        return isMysql ? ('`' + clean + '`') : ('"' + clean + '"');
      }

      function sqlQuoteVal(val, isMysql) {
        if (val === null || val === undefined || val === 'NULL') return 'NULL';
        const s = String(val);
        const escaped = isMysql ? s.replace(/\\/g, '\\\\').replace(/'/g, "''") : s.replace(/'/g, "''");
        return "'" + escaped + "'";
      }

      function askDialog(opts) {
        return new Promise((resolve) => {
          const backdrop = document.getElementById('pmaDialog');
          const titleEl = document.getElementById('pmaDialogTitle');
          const msgEl = document.getElementById('pmaDialogMsg');
          const inputWrap = document.getElementById('pmaDialogInputWrap');
          const inputEl = document.getElementById('pmaDialogInput');
          const btnCancel = document.getElementById('pmaDialogCancel');
          const btnOk = document.getElementById('pmaDialogOk');

          titleEl.textContent = opts.title || 'Xác nhận thao tác';
          msgEl.textContent = opts.message || '';
          btnOk.textContent = opts.confirmText || 'Xác nhận';
          btnOk.className = opts.danger ? 'danger' : 'primary';

          if (opts.prompt) {
            inputWrap.classList.remove('hidden');
            inputEl.type = opts.inputType || 'text';
            inputEl.value = opts.defaultValue || '';
            inputEl.placeholder = opts.placeholder || '';
          } else {
            inputWrap.classList.add('hidden');
            inputEl.value = '';
          }

          backdrop.classList.remove('hidden');
          if (opts.prompt) {
            setTimeout(() => inputEl.focus(), 10);
          }

          function cleanup(result) {
            backdrop.classList.add('hidden');
            btnCancel.removeEventListener('click', onCancel);
            btnOk.removeEventListener('click', onOk);
            inputEl.removeEventListener('keydown', onKey);
            resolve(result);
          }
          function onCancel() { cleanup(null); }
          function onOk() { cleanup(opts.prompt ? inputEl.value : true); }
          function onKey(ev) {
            if (ev.key === 'Enter') { ev.preventDefault(); onOk(); }
            if (ev.key === 'Escape') { ev.preventDefault(); onCancel(); }
          }
          btnCancel.addEventListener('click', onCancel);
          btnOk.addEventListener('click', onOk);
          inputEl.addEventListener('keydown', onKey);
        });
      }

      function showBanner(msg, isErr) {
        const el = document.getElementById('banner');
        if (!msg) { el.classList.add('hidden'); return; }
        el.textContent = msg;
        el.className = 'notice ' + (isErr ? 'err' : 'ok');
        el.classList.remove('hidden');
      }

      async function api(endpoint, options) {
        const opts = Object.assign({ credentials: 'same-origin', headers: {} }, options || {});
        if (authToken) {
          opts.headers['Authorization'] = 'Bearer ' + authToken;
        }
        const res = await fetch(apiBase + endpoint, opts);
        const text = await res.text();
        let data = null;
        try { data = text ? JSON.parse(text) : null; } catch (_) { data = text; }
        if (res.status === 401) {
          authToken = '';
          sessionStorage.removeItem('fp_pma_token');
          showLogin();
          throw new Error((data && data.error) || 'Phiên đăng nhập đã hết hạn');
        }
        if (!res.ok) {
          throw new Error((data && (data.error || data.message)) || ('HTTP ' + res.status));
        }
        return data;
      }

      function showLogin() {
        document.getElementById('loginView').classList.remove('hidden');
        document.getElementById('appView').classList.add('hidden');
        document.getElementById('btnRefresh').classList.add('hidden');
        document.getElementById('btnLogout').classList.add('hidden');
        document.getElementById('userLabel').textContent = '';
      }

      function showApp(username) {
        document.getElementById('loginView').classList.add('hidden');
        document.getElementById('appView').classList.remove('hidden');
        document.getElementById('btnRefresh').classList.remove('hidden');
        document.getElementById('btnLogout').classList.remove('hidden');
        document.getElementById('userLabel').textContent = 'Admin: ' + username;
      }

      function getActiveDb() {
        return databases.find((d) => d.id === activeDbId || d.name === activeDbId) || databases[0] || null;
      }

      function renderSidebar() {
        const container = document.getElementById('dbList');
        if (!databases.length) {
          container.innerHTML = '<p class="muted">Chưa có cơ sở dữ liệu nào. Nhấn "+ Tạo DB" để tạo SQLite 3 (WAL thuần Rust), MariaDB hoặc PostgreSQL.</p>';
          return;
        }
        const currentDb = getActiveDb();
        if (currentDb) activeDbId = currentDb.id;
        container.innerHTML = databases.map((db) => {
          const isActive = db.id === activeDbId;
          const tablesHtml = isActive && Array.isArray(db.tables) && db.tables.length
            ? '<div class="tbl-list">' + db.tables.map((t) =>
                '<div class="tbl-item ' + (t.tableName === activeTable ? 'active' : '') + '" data-select-table="' + esc(t.tableName) + '">' +
                '<span>' + esc(t.tableName) + '</span><span>' + esc(t.rowCount) + '</span></div>'
              ).join('') + '</div>'
            : '';
          return '<div class="db-item ' + (isActive ? 'active' : '') + '" data-select-db="' + esc(db.id) + '">' +
            '<div style="font-weight:700;font-size:13px;">' + esc(db.name) + '</div>' +
            '<div class="db-meta">' + esc(db.engine) + ' · ' + esc(db.status) + ' · ' + esc(db.sizeMb) + ' MB</div>' +
            tablesHtml + '</div>';
        }).join('');

        container.querySelectorAll('[data-select-db]').forEach((el) => {
          el.addEventListener('click', (e) => {
            if (e.target.closest('[data-select-table]')) return;
            activeDbId = el.getAttribute('data-select-db');
            const db = getActiveDb();
            activeTable = db && db.tables && db.tables[0] ? db.tables[0].tableName : '';
            browseOffset = 0;
            if (activeTab === 'databases') activeTab = 'structure';
            syncTabs();
            renderSidebar();
            renderTab();
          });
        });
        container.querySelectorAll('[data-select-table]').forEach((el) => {
          el.addEventListener('click', () => {
            activeTable = el.getAttribute('data-select-table');
            browseOffset = 0;
            activeTab = 'browse';
            syncTabs();
            renderSidebar();
            loadBrowseRows();
          });
        });
      }

      function syncTabs() {
        document.querySelectorAll('#mainTabs .tab').forEach((btn) => {
          btn.classList.toggle('active', btn.getAttribute('data-tab') === activeTab);
        });
      }

      function renderResultTable(res, editableOpts) {
        if (!res) return '';
        if (!res.success) {
          return '<div class="notice err">' + esc(res.error || 'SQL execution failed') + '</div>';
        }
        const meta = '<div class="notice ok">OK · ' + esc(res.commandTag) + ' · ' + esc(res.affectedRows) + ' rows · ' + esc(res.executionTimeMs) + ' ms</div>';
        if (!res.columns || !res.columns.length) return meta;
        const canEdit = editableOpts && editableOpts.pkCol && editableOpts.tableName;
        const ths = res.columns.map((c) => '<th>' + esc(c) + (canEdit && c === editableOpts.pkCol ? ' <span class="badge">PK</span>' : '') + '</th>').join('') + (canEdit ? '<th>Thao tác dòng</th>' : '');
        const trs = (res.rows || []).map((r, rIdx) => {
          const tds = res.columns.map((c) => {
            const val = r[c] === null || r[c] === undefined ? 'NULL' : r[c];
            return '<td data-col="' + esc(c) + '">' + esc(val) + '</td>';
          }).join('');
          const actTd = canEdit
            ? '<td class="row"><button type="button" data-edit-row="' + rIdx + '">Sửa nhanh</button><button type="button" class="danger" data-del-row="' + rIdx + '">Xóa dòng</button></td>'
            : '';
          return '<tr data-row-idx="' + rIdx + '">' + tds + actTd + '</tr>';
        }).join('');
        return meta + '<div class="table-wrap"><table><thead><tr>' + ths + '</tr></thead><tbody>' + (trs || '<tr><td colspan="' + (res.columns.length + (canEdit ? 1 : 0)) + '" class="muted">(0 dòng)</td></tr>') + '</tbody></table></div>';
      }

      async function loadBrowseRows() {
        const db = getActiveDb();
        if (!db || !activeTable) { renderTab(); return; }
        try {
          const data = await api('/databases/' + encodeURIComponent(db.id) + '/tables/' + encodeURIComponent(activeTable) + '/rows?limit=' + browseLimit + '&offset=' + browseOffset);
          lastQueryResult = data;
          renderTab();
        } catch (err) {
          showBanner(err.message, true);
        }
      }

      function renderTab() {
        const el = document.getElementById('tabContent');
        const db = getActiveDb();

        if (activeTab === 'databases') {
          el.innerHTML =
            '<div class="card">' +
            '<h3 style="margin:0 0 10px;font-size:14px;">Tạo Cơ sở dữ liệu mới (Pure-Rust SQLite 3 WAL / MariaDB / PostgreSQL)</h3>' +
            '<form id="createDbForm" class="grid-2">' +
            '<div><label>Tên Database</label><input id="newDbName" placeholder="app_store" required /></div>' +
            '<div><label>Engine</label><select id="newDbEngine">' +
            '<option value="MariaDB 11.4">MariaDB 11.4 / MySQL</option>' +
            '<option value="PostgreSQL 17">PostgreSQL 17</option>' +
            '<option value="SQLite 3 (WAL)">SQLite 3 (WAL) — 100% Pure Rust (Không cần cài đặt)</option>' +
            '</select></div>' +
            '<div><label>Username</label><input id="newDbUser" placeholder="app_store_usr" /></div>' +
            '<div><label>Password (bắt buộc với MariaDB/PostgreSQL)</label><input id="newDbPass" type="password" placeholder="Nhập mật khẩu DB user" /></div>' +
            '<div style="grid-column:1/-1;"><button type="submit" class="primary">Khởi tạo Database</button></div>' +
            '</form></div>' +
            '<div class="card"><h3 style="margin:0 0 10px;font-size:14px;">Danh sách Cơ sở dữ liệu (' + databases.length + ')</h3>' +
            '<div class="table-wrap"><table><thead><tr><th>Tên</th><th>Engine</th><th>User / Host</th><th>Trạng thái</th><th>Số bảng</th><th>Dung lượng</th><th>Thao tác</th></tr></thead><tbody>' +
            (databases.map((d) =>
              '<tr><td><strong>' + esc(d.name) + '</strong></td><td>' + esc(d.engine) + '</td><td>' + esc(d.username) + '@' + esc(d.host) + '</td><td>' + esc(d.status) + '</td><td>' + (d.tables ? d.tables.length : 0) + '</td><td>' + esc(d.sizeMb) + ' MB</td>' +
              '<td class="row"><button type="button" class="primary" data-open-db="' + esc(d.id) + '">Quản lý Bảng</button> <button type="button" data-pw-db="' + esc(d.id) + '" data-pw-name="' + esc(d.name) + '">Đổi MK</button> <button type="button" class="danger" data-drop-db="' + esc(d.id) + '">Xóa</button></td></tr>'
            ).join('') || '<tr><td colspan="7" class="muted">Chưa có database nào.</td></tr>') +
            '</tbody></table></div></div>';

          document.getElementById('createDbForm').addEventListener('submit', async (ev) => {
            ev.preventDefault();
            showBanner('', false);
            const name = document.getElementById('newDbName').value.trim();
            const engine = document.getElementById('newDbEngine').value;
            const username = document.getElementById('newDbUser').value.trim() || (name + '_usr');
            const password = document.getElementById('newDbPass').value;
            try {
              const list = await api('/databases', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ name, engine, username, password: password || undefined, collation: '', linkedAppId: '' })
              });
              databases = Array.isArray(list) ? list : [];
              if (databases[0]) activeDbId = databases[0].id;
              showBanner('Đã tạo database "' + name + '" (' + engine + ') thành công.', false);
              renderSidebar();
              renderTab();
            } catch (err) {
              showBanner(err.message, true);
            }
          });

          el.querySelectorAll('[data-open-db]').forEach((b) => {
            b.addEventListener('click', () => {
              activeDbId = b.getAttribute('data-open-db');
              const picked = getActiveDb();
              activeTable = picked && picked.tables && picked.tables[0] ? picked.tables[0].tableName : '';
              activeTab = 'structure';
              syncTabs();
              renderSidebar();
              renderTab();
            });
          });
          el.querySelectorAll('[data-pw-db]').forEach((b) => {
            b.addEventListener('click', async () => {
              const id = b.getAttribute('data-pw-db');
              const nm = b.getAttribute('data-pw-name');
              const npw = await askDialog({
                title: 'Đổi mật khẩu Database User',
                message: 'Nhập mật khẩu mới cho user của database "' + nm + '":',
                prompt: true,
                inputType: 'password',
                placeholder: 'Tối thiểu 8 ký tự',
                confirmText: 'Lưu mật khẩu'
              });
              if (!npw) return;
              try {
                databases = await api('/databases/' + encodeURIComponent(id) + '/password', {
                  method: 'PUT',
                  headers: { 'Content-Type': 'application/json' },
                  body: JSON.stringify({ newPassword: npw })
                });
                showBanner('Đã cập nhật mật khẩu cho database "' + nm + '".', false);
              } catch (err) {
                showBanner(err.message, true);
              }
            });
          });
          el.querySelectorAll('[data-drop-db]').forEach((b) => {
            b.addEventListener('click', async () => {
              const id = b.getAttribute('data-drop-db');
              const ok = await askDialog({
                title: 'Xóa vĩnh viễn Database',
                message: 'Xác nhận xóa vĩnh viễn database này? Dữ liệu sẽ không thể khôi phục.',
                danger: true,
                confirmText: 'Xóa Database'
              });
              if (!ok) return;
              try {
                const list = await api('/databases/' + encodeURIComponent(id), { method: 'DELETE' });
                databases = Array.isArray(list) ? list : [];
                if (activeDbId === id) activeDbId = databases[0] ? databases[0].id : '';
                showBanner('Đã xóa database.', false);
                renderSidebar();
                renderTab();
              } catch (err) {
                showBanner(err.message, true);
              }
            });
          });
          return;
        }

        if (activeTab === 'server_cfg') {
          if (!mysqlConfig) {
            el.innerHTML = '<div class="card"><p class="muted">Đang nạp cấu hình MySQL / MariaDB &amp; phpMyAdmin...</p></div>';
            api('/databases/mysql-config').then((cfg) => {
              mysqlConfig = cfg;
              renderTab();
            }).catch((err) => showBanner(err.message, true));
            return;
          }
          el.innerHTML =
            '<div class="card">' +
            '<h3 style="margin:0 0 10px;font-size:14px;">Cấu hình MariaDB / MySQL (50-server.cnf) &amp; FerrousAdmin (/phpadmin/)</h3>' +
            '<div class="grid-2" style="margin-bottom:12px;">' +
            '<div><label>Phiên bản Engine</label><input value="' + esc(mysqlConfig.mysqlVersion || '') + '" readonly /></div>' +
            '<div><label>Unix Socket</label><input value="' + esc(mysqlConfig.socketPath || '/run/mysqld/mysqld.sock') + '" readonly /></div>' +
            '<div><label>Bind Address : Port</label><input value="' + esc((mysqlConfig.bindAddress || '127.0.0.1') + ':' + (mysqlConfig.port || 3306)) + '" readonly /></div>' +
            '<div><label>Max Connections / Buffer Pool</label><input value="' + esc((mysqlConfig.maxConnections || 151) + ' conn / ' + (mysqlConfig.innodbBufferPoolMb || 128) + ' MB') + '" readonly /></div>' +
            '</div>' +
            '<label>Nội dung cấu hình trực tiếp (/etc/mysql/mariadb.conf.d/50-server.cnf)</label>' +
            '<textarea id="myCnfEditor" style="min-height:220px;" placeholder="[mysqld]\nbind-address = 127.0.0.1\nmax_connections = 200\ninnodb_buffer_pool_size = 256M">' + esc(mysqlConfig.myCnfRaw || '') + '</textarea>' +
            '<div class="row" style="margin-top:12px;justify-content:flex-end;">' +
            '<button type="button" class="primary" id="btnSaveMyCnf">Lưu cấu hình &amp; Reload MariaDB</button>' +
            '</div></div>';
          document.getElementById('btnSaveMyCnf').addEventListener('click', async () => {
            const myCnfRaw = document.getElementById('myCnfEditor').value;
            try {
              mysqlConfig = await api('/databases/mysql-config', {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ myCnfRaw })
              });
              showBanner('Đã lưu cấu hình MariaDB/MySQL và reload dịch vụ.', false);
              renderTab();
            } catch (err) {
              showBanner(err.message, true);
            }
          });
          return;
        }

        if (!db) {
          el.innerHTML = '<div class="card"><p class="muted">Hãy chọn hoặc tạo một database ở tab Databases trước.</p></div>';
          return;
        }

        const isMysql = db.engine.toLowerCase().includes('maria') || db.engine.toLowerCase().includes('mysql');

        if (activeTab === 'structure') {
          const rows = (db.tables || []).map((t) =>
            '<tr><td><strong>' + esc(t.tableName) + '</strong></td><td>' + esc(t.rowCount) + '</td><td>' + esc(t.sizeKb) + ' KB</td><td>' +
            (t.columns || []).map((c) => esc(c.name) + ':' + esc(c.dataType) + (c.isPrimaryKey ? ' [PK]' : '')).join(', ') +
            '</td><td class="row"><button type="button" class="primary" data-browse="' + esc(t.tableName) + '">Browse &amp; Sửa</button>' +
            '<button type="button" data-insert-tbl="' + esc(t.tableName) + '">Chèn dòng</button>' +
            '<button type="button" data-addcol-tbl="' + esc(t.tableName) + '">+ Thêm cột</button>' +
            '<button type="button" data-trunc="' + esc(t.tableName) + '">Truncate</button>' +
            '<button type="button" class="danger" data-drop-tbl="' + esc(t.tableName) + '">Drop</button></td></tr>'
          ).join('');
          el.innerHTML = '<div class="card"><div class="row" style="justify-content:space-between;margin-bottom:10px;">' +
            '<h3 style="margin:0;font-size:14px;">Cấu trúc bảng — ' + esc(db.name) + ' (' + esc(db.engine) + ')</h3>' +
            '<div class="row"><button type="button" class="primary" id="btnGoNewTable">+ Tạo bảng mới</button></div></div>' +
            '<div class="table-wrap"><table><thead><tr><th>Bảng</th><th>Dòng</th><th>Kích thước</th><th>Cột (Schema)</th><th>Thao tác chuyên sâu</th></tr></thead><tbody>' +
            (rows || '<tr><td colspan="5" class="muted">Chưa có bảng nào trong database này.</td></tr>') +
            '</tbody></table></div></div>';
          document.getElementById('btnGoNewTable').addEventListener('click', () => { activeTab = 'new_table'; syncTabs(); renderTab(); });
          el.querySelectorAll('[data-browse]').forEach((b) => b.addEventListener('click', () => {
            activeTable = b.getAttribute('data-browse'); browseOffset = 0; activeTab = 'browse'; syncTabs(); renderSidebar(); loadBrowseRows();
          }));
          el.querySelectorAll('[data-insert-tbl]').forEach((b) => b.addEventListener('click', () => {
            activeTable = b.getAttribute('data-insert-tbl'); activeTab = 'insert'; syncTabs(); renderSidebar(); renderTab();
          }));
          el.querySelectorAll('[data-addcol-tbl]').forEach((b) => b.addEventListener('click', async () => {
            const tbl = b.getAttribute('data-addcol-tbl');
            const colSpec = await askDialog({
              title: 'Thêm cột mới (ALTER TABLE)',
              message: 'Nhập định nghĩa cột mới cho bảng "' + tbl + '" (VD: status VARCHAR(64) DEFAULT \'active\'):',
              prompt: true,
              defaultValue: 'extra_note TEXT',
              confirmText: 'Thêm cột'
            });
            if (!colSpec) return;
            const sql = 'ALTER TABLE ' + sqlQuoteId(tbl, isMysql) + ' ADD COLUMN ' + colSpec + ';';
            try {
              const res = await api('/databases/' + encodeURIComponent(db.id) + '/query', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ query: sql })
              });
              if (!res.success) { showBanner(res.error || 'ALTER TABLE lỗi', true); return; }
              databases = await api('/databases');
              showBanner('Đã thêm cột mới vào bảng ' + tbl, false);
              renderSidebar(); renderTab();
            } catch (err) { showBanner(err.message, true); }
          }));
          el.querySelectorAll('[data-trunc]').forEach((b) => b.addEventListener('click', async () => {
            const tbl = b.getAttribute('data-trunc');
            const ok = await askDialog({
              title: 'Làm rỗng bảng (TRUNCATE)',
              message: 'Xác nhận làm rỗng (TRUNCATE) toàn bộ dữ liệu trong bảng "' + tbl + '"?',
              danger: true,
              confirmText: 'Truncate'
            });
            if (!ok) return;
            try {
              databases = await api('/databases/' + encodeURIComponent(db.id) + '/tables/' + encodeURIComponent(tbl) + '/truncate', { method: 'POST' });
              showBanner('Đã làm rỗng bảng ' + tbl, false);
              renderSidebar(); renderTab();
            } catch (err) { showBanner(err.message, true); }
          }));
          el.querySelectorAll('[data-drop-tbl]').forEach((b) => b.addEventListener('click', async () => {
            const tbl = b.getAttribute('data-drop-tbl');
            const ok = await askDialog({
              title: 'Xóa bảng (DROP TABLE)',
              message: 'Xác nhận XÓA BẢNG (DROP TABLE) "' + tbl + '"?',
              danger: true,
              confirmText: 'Drop Table'
            });
            if (!ok) return;
            try {
              databases = await api('/databases/' + encodeURIComponent(db.id) + '/tables/' + encodeURIComponent(tbl), { method: 'DELETE' });
              if (activeTable === tbl) activeTable = '';
              showBanner('Đã xóa bảng ' + tbl, false);
              renderSidebar(); renderTab();
            } catch (err) { showBanner(err.message, true); }
          }));
          return;
        }

        if (activeTab === 'browse') {
          const tblOpts = (db.tables || []).map((t) => '<option value="' + esc(t.tableName) + '" ' + (t.tableName === activeTable ? 'selected' : '') + '>' + esc(t.tableName) + ' (' + esc(t.rowCount) + ')</option>').join('');
          const selectedObj = (db.tables || []).find((t) => t.tableName === activeTable);
          const pkColObj = selectedObj && selectedObj.columns ? (selectedObj.columns.find((c) => c.isPrimaryKey) || selectedObj.columns[0]) : null;
          const pkCol = pkColObj ? pkColObj.name : '';
          const fallbackRes = selectedObj ? {
            success: true,
            commandTag: 'SELECT',
            executionTimeMs: 0.1,
            affectedRows: (selectedObj.rows || []).length,
            columns: (selectedObj.columns || []).map((c) => c.name),
            rows: selectedObj.rows || []
          } : null;
          const effectiveRes = lastQueryResult || fallbackRes;
          el.innerHTML = '<div class="card"><div class="row" style="justify-content:space-between;margin-bottom:10px;">' +
            '<div class="row"><strong style="font-size:14px;">Bảng:</strong>' +
            '<select id="browseTblSelect" style="width:auto;">' + (tblOpts || '<option value="">(Không có bảng)</option>') + '</select>' +
            '<span class="muted">Khóa chính: <strong>' + esc(pkCol || '—') + '</strong> · Offset: ' + browseOffset + '</span></div>' +
            '<div class="row">' +
            '<button type="button" id="btnPrevPage" ' + (browseOffset === 0 ? 'disabled' : '') + '>&laquo; Trang trước</button>' +
            '<button type="button" id="btnNextPage">Trang sau &raquo;</button>' +
            '<button type="button" id="btnGoInsert">+ Chèn dòng mới</button>' +
            '<button type="button" class="primary" id="btnReloadRows">Làm mới</button>' +
            '</div></div>' +
            '<div id="inlineEditBox" class="hidden card" style="background:#020617;margin-bottom:12px;"></div>' +
            renderResultTable(effectiveRes, { pkCol, tableName: activeTable }) + '</div>';

          document.getElementById('browseTblSelect').addEventListener('change', (ev) => {
            activeTable = ev.target.value;
            browseOffset = 0;
            loadBrowseRows();
          });
          document.getElementById('btnPrevPage').addEventListener('click', () => {
            browseOffset = Math.max(0, browseOffset - browseLimit);
            loadBrowseRows();
          });
          document.getElementById('btnNextPage').addEventListener('click', () => {
            browseOffset += browseLimit;
            loadBrowseRows();
          });
          document.getElementById('btnGoInsert').addEventListener('click', () => {
            activeTab = 'insert'; syncTabs(); renderTab();
          });
          document.getElementById('btnReloadRows').addEventListener('click', loadBrowseRows);

          el.querySelectorAll('[data-edit-row]').forEach((btn) => {
            btn.addEventListener('click', () => {
              const rIdx = Number(btn.getAttribute('data-edit-row'));
              const rowObj = effectiveRes && effectiveRes.rows ? effectiveRes.rows[rIdx] : null;
              if (!rowObj || !pkCol) return;
              const pkVal = rowObj[pkCol];
              const editBox = document.getElementById('inlineEditBox');
              editBox.classList.remove('hidden');
              editBox.innerHTML =
                '<h4 style="margin:0 0 10px;font-size:13px;">Chỉnh sửa dòng (' + esc(pkCol) + ' = ' + esc(pkVal) + ') trong bảng ' + esc(activeTable) + '</h4>' +
                '<form id="inlineRowForm" class="grid-2">' +
                (effectiveRes.columns || []).map((c) =>
                  '<div><label>' + esc(c) + (c === pkCol ? ' (PK)' : '') + '</label>' +
                  '<input name="' + esc(c) + '" value="' + esc(rowObj[c] === null || rowObj[c] === undefined ? '' : rowObj[c]) + '" /></div>'
                ).join('') +
                '<div class="row" style="grid-column:1/-1;justify-content:flex-end;margin-top:8px;">' +
                '<button type="button" id="btnCancelInline">Hủy</button>' +
                '<button type="submit" class="primary">Lưu UPDATE SQL</button>' +
                '</div></form>';
              document.getElementById('btnCancelInline').addEventListener('click', () => editBox.classList.add('hidden'));
              document.getElementById('inlineRowForm').addEventListener('submit', async (ev) => {
                ev.preventDefault();
                const fd = new FormData(ev.target);
                const sets = [];
                (effectiveRes.columns || []).forEach((c) => {
                  const newVal = fd.get(c);
                  if (String(newVal) !== String(rowObj[c] === null || rowObj[c] === undefined ? '' : rowObj[c])) {
                    sets.push(sqlQuoteId(c, isMysql) + ' = ' + sqlQuoteVal(newVal, isMysql));
                  }
                });
                if (!sets.length) {
                  editBox.classList.add('hidden');
                  return;
                }
                const sql = 'UPDATE ' + sqlQuoteId(activeTable, isMysql) + ' SET ' + sets.join(', ') +
                  ' WHERE ' + sqlQuoteId(pkCol, isMysql) + ' = ' + sqlQuoteVal(pkVal, isMysql) + ';';
                try {
                  const res = await api('/databases/' + encodeURIComponent(db.id) + '/query', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ query: sql })
                  });
                  if (!res.success) { showBanner(res.error || 'UPDATE lỗi', true); return; }
                  showBanner('Đã cập nhật dòng (' + pkCol + '=' + pkVal + ') thành công.', false);
                  await loadBrowseRows();
                } catch (err) {
                  showBanner(err.message, true);
                }
              });
            });
          });

          el.querySelectorAll('[data-del-row]').forEach((btn) => {
            btn.addEventListener('click', async () => {
              const rIdx = Number(btn.getAttribute('data-del-row'));
              const rowObj = effectiveRes && effectiveRes.rows ? effectiveRes.rows[rIdx] : null;
              if (!rowObj || !pkCol) return;
              const pkVal = rowObj[pkCol];
              const ok = await askDialog({
                title: 'Xóa dòng (DELETE)',
                message: 'Xác nhận xóa dòng có ' + pkCol + ' = ' + pkVal + ' khỏi bảng ' + activeTable + '?',
                danger: true,
                confirmText: 'Xóa dòng'
              });
              if (!ok) return;
              const sql = 'DELETE FROM ' + sqlQuoteId(activeTable, isMysql) + ' WHERE ' + sqlQuoteId(pkCol, isMysql) + ' = ' + sqlQuoteVal(pkVal, isMysql) + ';';
              try {
                const res = await api('/databases/' + encodeURIComponent(db.id) + '/query', {
                  method: 'POST',
                  headers: { 'Content-Type': 'application/json' },
                  body: JSON.stringify({ query: sql })
                });
                if (!res.success) { showBanner(res.error || 'DELETE lỗi', true); return; }
                databases = await api('/databases');
                showBanner('Đã xóa dòng (' + pkCol + '=' + pkVal + ').', false);
                renderSidebar();
                await loadBrowseRows();
              } catch (err) {
                showBanner(err.message, true);
              }
            });
          });
          return;
        }

        if (activeTab === 'insert') {
          const selectedObj = (db.tables || []).find((t) => t.tableName === activeTable) || (db.tables || [])[0];
          if (!selectedObj) {
            el.innerHTML = '<div class="card"><p class="muted">Hãy tạo bảng trước khi chèn dữ liệu.</p></div>';
            return;
          }
          activeTable = selectedObj.tableName;
          const cols = selectedObj.columns || [];
          el.innerHTML = '<div class="card"><h3 style="margin:0 0 10px;font-size:14px;">Chèn dòng mới (INSERT INTO ' + esc(activeTable) + ')</h3>' +
            '<form id="insertRowForm" class="grid-2">' +
            cols.map((c) =>
              '<div><label>' + esc(c.name) + ' <span class="muted">(' + esc(c.dataType) + (c.isPrimaryKey ? ' · PK' : '') + ')</span></label>' +
              '<input name="' + esc(c.name) + '" placeholder="' + (c.isPrimaryKey ? 'Để trống nếu AUTO_INCREMENT' : (c.nullable ? 'NULL' : 'Nhập giá trị')) + '" /></div>'
            ).join('') +
            '<div style="grid-column:1/-1;margin-top:8px;"><button type="submit" class="primary">Thực thi INSERT</button></div>' +
            '</form></div>';
          document.getElementById('insertRowForm').addEventListener('submit', async (ev) => {
            ev.preventDefault();
            const fd = new FormData(ev.target);
            const colNames = [];
            const colVals = [];
            cols.forEach((c) => {
              const raw = String(fd.get(c.name) || '');
              if (raw === '' && c.isPrimaryKey) return;
              colNames.push(sqlQuoteId(c.name, isMysql));
              colVals.push(raw === '' && c.nullable ? 'NULL' : sqlQuoteVal(raw, isMysql));
            });
            if (!colNames.length) {
              showBanner('Hãy nhập ít nhất 1 cột.', true);
              return;
            }
            const sql = 'INSERT INTO ' + sqlQuoteId(activeTable, isMysql) + ' (' + colNames.join(', ') + ') VALUES (' + colVals.join(', ') + ');';
            try {
              const res = await api('/databases/' + encodeURIComponent(db.id) + '/query', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ query: sql })
              });
              if (!res.success) { showBanner(res.error || 'INSERT lỗi', true); return; }
              databases = await api('/databases');
              showBanner('Đã chèn dòng mới vào bảng "' + activeTable + '".', false);
              activeTab = 'browse';
              syncTabs();
              renderSidebar();
              await loadBrowseRows();
            } catch (err) {
              showBanner(err.message, true);
            }
          });
          return;
        }

        if (activeTab === 'sql') {
          const defaultSql = activeTable
            ? 'SELECT * FROM ' + sqlQuoteId(activeTable, isMysql) + ' LIMIT 50;'
            : (db.engine.toLowerCase().includes('sqlite')
              ? "SELECT name, type FROM sqlite_master WHERE type = 'table';"
              : 'SHOW TABLES;');
          el.innerHTML = '<div class="card"><div class="row" style="justify-content:space-between;margin-bottom:10px;">' +
            '<h3 style="margin:0;font-size:14px;">SQL Console — ' + esc(db.name) + ' (' + esc(db.engine) + ')</h3>' +
            '<div class="row">' +
            '<button type="button" data-tpl-sql="SHOW TABLES;">SHOW TABLES</button>' +
            (isMysql ? '<button type="button" data-tpl-sql="SHOW PROCESSLIST;">PROCESSLIST</button><button type="button" data-tpl-sql="SHOW VARIABLES LIKE \'%buffer%\';">VARIABLES</button>' : '') +
            (activeTable ? '<button type="button" data-tpl-sql="SELECT * FROM ' + esc(sqlQuoteId(activeTable, isMysql)) + ' LIMIT 100;">SELECT * (' + esc(activeTable) + ')</button>' : '') +
            '<button type="button" data-tpl-sql="SELECT option_name, option_value FROM wp_options WHERE option_name IN (\'siteurl\', \'home\', \'blogname\', \'admin_email\');">WP Options</button>' +
            '</div></div>' +
            '<textarea id="sqlEditor">' + esc(defaultSql) + '</textarea>' +
            '<div class="row" style="margin-top:10px;"><button type="button" class="primary" id="btnRunSql">Thực thi SQL</button></div></div>' +
            '<div id="sqlOut">' + renderResultTable(lastQueryResult) + '</div>';
          el.querySelectorAll('[data-tpl-sql]').forEach((b) => {
            b.addEventListener('click', () => {
              document.getElementById('sqlEditor').value = b.getAttribute('data-tpl-sql');
            });
          });
          document.getElementById('btnRunSql').addEventListener('click', async () => {
            const query = document.getElementById('sqlEditor').value;
            showBanner('', false);
            try {
              lastQueryResult = await api('/databases/' + encodeURIComponent(db.id) + '/query', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ query })
              });
              databases = await api('/databases');
              renderSidebar();
              document.getElementById('sqlOut').innerHTML = renderResultTable(lastQueryResult);
            } catch (err) {
              showBanner(err.message, true);
            }
          });
          return;
        }

        if (activeTab === 'new_table') {
          el.innerHTML = '<div class="card"><h3 style="margin:0 0 10px;font-size:14px;">Tạo bảng mới trong ' + esc(db.name) + '</h3>' +
            '<form id="newTableForm" class="grid-2">' +
            '<div><label>Tên bảng</label><input id="ntName" placeholder="users" required /></div>' +
            '<div><label>Cột 1 (Primary Key AUTO_INCREMENT)</label><input id="ntCol1" value="id" required /></div>' +
            '<div><label>Cột 2 (TEXT / VARCHAR)</label><input id="ntCol2" value="title" required /></div>' +
            '<div><label>Cột 3 (TEXT / TIMESTAMP)</label><input id="ntCol3" value="created_at" required /></div>' +
            '<div style="grid-column:1/-1;"><button type="submit" class="primary">Tạo bảng</button></div>' +
            '</form></div>';
          document.getElementById('newTableForm').addEventListener('submit', async (ev) => {
            ev.preventDefault();
            const t = document.getElementById('ntName').value.trim();
            const c1 = document.getElementById('ntCol1').value.trim();
            const c2 = document.getElementById('ntCol2').value.trim();
            const c3 = document.getElementById('ntCol3').value.trim();
            const isSqlite = db.engine.toLowerCase().includes('sqlite');
            const isPg = db.engine.toLowerCase().includes('postgres');
            const qt = sqlQuoteId(t, isMysql);
            const qc1 = sqlQuoteId(c1, isMysql);
            const qc2 = sqlQuoteId(c2, isMysql);
            const qc3 = sqlQuoteId(c3, isMysql);
            const ddl = isSqlite
              ? 'CREATE TABLE ' + qt + ' (' + qc1 + ' INTEGER PRIMARY KEY AUTOINCREMENT, ' + qc2 + ' TEXT NOT NULL, ' + qc3 + " TEXT DEFAULT (datetime('now')));"
              : isPg
              ? 'CREATE TABLE ' + qt + ' (' + qc1 + ' BIGSERIAL PRIMARY KEY, ' + qc2 + ' VARCHAR(255) NOT NULL, ' + qc3 + ' TIMESTAMPTZ DEFAULT NOW());'
              : 'CREATE TABLE ' + qt + ' (' + qc1 + ' BIGINT AUTO_INCREMENT PRIMARY KEY, ' + qc2 + ' VARCHAR(255) NOT NULL, ' + qc3 + ' TIMESTAMP DEFAULT CURRENT_TIMESTAMP);';
            try {
              const res = await api('/databases/' + encodeURIComponent(db.id) + '/query', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ query: ddl })
              });
              if (!res.success) { showBanner(res.error || 'Tạo bảng thất bại', true); return; }
              databases = await api('/databases');
              activeTable = t;
              activeTab = 'structure';
              syncTabs();
              showBanner('Đã tạo bảng "' + t + '" thành công.', false);
              renderSidebar(); renderTab();
            } catch (err) { showBanner(err.message, true); }
          });
          return;
        }

        if (activeTab === 'export') {
          el.innerHTML = '<div class="card"><div class="row" style="justify-content:space-between;margin-bottom:10px;">' +
            '<h3 style="margin:0;font-size:14px;">Xuất bản sao lưu .SQL — ' + esc(db.name) + '</h3>' +
            '<div class="row"><button type="button" class="primary" id="btnGenExport">Tạo bản Dump .SQL</button>' +
            '<button type="button" id="btnDlExport">Tải file .sql</button></div></div>' +
            '<textarea id="exportOut" readonly placeholder="Nhấn Tạo bản Dump .SQL để xuất toàn bộ schema + dữ liệu...">' + esc(lastExportSql) + '</textarea></div>';
          document.getElementById('btnGenExport').addEventListener('click', async () => {
            try {
              const res = await api('/databases/' + encodeURIComponent(db.id) + '/export');
              lastExportSql = (res && res.sql) || '';
              document.getElementById('exportOut').value = lastExportSql;
              showBanner('Đã xuất dump SQL thành công (' + lastExportSql.length + ' bytes).', false);
            } catch (err) { showBanner(err.message, true); }
          });
          document.getElementById('btnDlExport').addEventListener('click', () => {
            if (!lastExportSql) return;
            const blob = new Blob([lastExportSql], { type: 'application/sql' });
            const a = document.createElement('a');
            a.href = URL.createObjectURL(blob);
            a.download = db.name + '.sql';
            a.click();
          });
          return;
        }

        if (activeTab === 'import') {
          el.innerHTML = '<div class="card"><h3 style="margin:0 0 10px;font-size:14px;">Nhập script .SQL — ' + esc(db.name) + '</h3>' +
            '<input type="file" id="sqlFileInput" accept=".sql,.txt" style="margin-bottom:10px;" />' +
            '<textarea id="importSqlText" placeholder="Dán lệnh SQL hoặc chọn file .sql ở trên..."></textarea>' +
            '<div class="row" style="margin-top:10px;"><button type="button" class="primary" id="btnRunImport">Chạy Import .SQL</button></div></div>';
          document.getElementById('sqlFileInput').addEventListener('change', (ev) => {
            const f = ev.target.files && ev.target.files[0];
            if (!f) return;
            const reader = new FileReader();
            reader.onload = () => { document.getElementById('importSqlText').value = String(reader.result || ''); };
            reader.readAsText(f);
          });
          document.getElementById('btnRunImport').addEventListener('click', async () => {
            const sql = document.getElementById('importSqlText').value;
            try {
              const res = await api('/databases/' + encodeURIComponent(db.id) + '/import', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ sql })
              });
              if (!res.success) { showBanner(res.error || 'Import lỗi', true); return; }
              databases = await api('/databases');
              showBanner('Import SQL thành công (' + res.executionTimeMs + ' ms).', false);
              renderSidebar();
            } catch (err) { showBanner(err.message, true); }
          });
        }
      }

      async function loadAll() {
        const sess = await api('/auth/session');
        if (!sess || !sess.authenticated) { showLogin(); return; }
        const uname = (sess.user && sess.user.username) || sess.username || 'admin';
        showApp(uname);
        databases = await api('/databases');
        if (activeDbId) {
          const matched = databases.find((d) => d.id === activeDbId || d.name === activeDbId);
          activeDbId = matched ? matched.id : (databases[0] ? databases[0].id : '');
        } else if (databases[0]) {
          activeDbId = databases[0].id;
        }
        const db = getActiveDb();
        if (!activeTable && db && db.tables && db.tables[0]) activeTable = db.tables[0].tableName;
        syncTabs();
        renderSidebar();
        if (activeTab === 'browse' && activeTable) {
          await loadBrowseRows();
        } else {
          renderTab();
        }
      }

      document.getElementById('loginForm').addEventListener('submit', async (ev) => {
        ev.preventDefault();
        const errEl = document.getElementById('loginError');
        errEl.classList.add('hidden');
        try {
          const username = document.getElementById('loginUser').value.trim();
          const password = document.getElementById('loginPass').value;
          const res = await api('/auth/login', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ username, password })
          });
          if (res && res.token) {
            authToken = res.token;
            sessionStorage.setItem('fp_pma_token', authToken);
          }
          await loadAll();
        } catch (err) {
          errEl.textContent = err.message;
          errEl.classList.remove('hidden');
        }
      });

      document.getElementById('btnRefresh').addEventListener('click', () => loadAll().catch((e) => showBanner(e.message, true)));
      document.getElementById('btnLogout').addEventListener('click', async () => {
        try { await api('/auth/logout', { method: 'POST' }); } catch (_) {}
        authToken = '';
        sessionStorage.removeItem('fp_pma_token');
        showLogin();
      });
      document.getElementById('btnShowCreateDb').addEventListener('click', () => {
        activeTab = 'databases'; syncTabs(); renderTab();
      });
      document.querySelectorAll('#mainTabs .tab').forEach((btn) => {
        btn.addEventListener('click', () => {
          activeTab = btn.getAttribute('data-tab');
          syncTabs();
          if (activeTab === 'browse' && activeTable) { loadBrowseRows(); } else { renderTab(); }
        });
      });

      loadAll().catch(() => showLogin());
    })();
  </script>
</body>
</html>"##;

/// Serves the standalone Pure-Rust FerrousAdmin SQL Studio (Panel custom DB UI) at `/phpadmin` and `/phpadmin/`.
pub async fn serve_ferrous_admin(_req: HttpRequest) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .insert_header(("X-Frame-Options", "SAMEORIGIN"))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .insert_header(("Cache-Control", "no-store"))
        .body(FERROUS_ADMIN_HTML)
}

pub const OFFICIAL_PMA_DIR: &str = "/www/server/phpmyadmin";
pub const OFFICIAL_PMA_ROUTER: &str = "/www/server/ferrous-pma-router.php";
pub const OFFICIAL_PMA_TMP_DIR: &str = "/var/lib/phpmyadmin/tmp";
pub const OFFICIAL_PMA_UNIT_NAME: &str = "ferrous-phpmyadmin.service";
pub const OFFICIAL_PMA_UNIT_PATH: &str = "/etc/systemd/system/ferrous-phpmyadmin.service";
pub const OFFICIAL_PMA_PORT: u16 = 889;
pub const OFFICIAL_PMA_VERSION: &str = "5.2.2";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialPhpMyAdminStatus {
    pub installed: bool,
    pub running: bool,
    pub version: String,
    pub install_dir: String,
    pub official_url: String,
    pub custom_panel_url: String,
}

fn detect_installed_pma_version() -> String {
    let pkg_json = Path::new(OFFICIAL_PMA_DIR).join("package.json");
    if let Ok(raw) = std::fs::read_to_string(&pkg_json) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(v) = val.get("version").and_then(|x| x.as_str()) {
                if !v.trim().is_empty() {
                    return v.trim().to_string();
                }
            }
        }
    }
    OFFICIAL_PMA_VERSION.to_string()
}

fn find_php_cli_binary_path() -> Option<&'static str> {
    [
        "/usr/bin/php8.3",
        "/usr/bin/php8.2",
        "/usr/bin/php8.4",
        "/usr/bin/php8.1",
        "/usr/bin/php",
        "/usr/bin/php7.4",
    ]
    .into_iter()
    .find(|candidate| Path::new(candidate).is_file())
}

async fn is_official_pma_port_open() -> bool {
    tokio::time::timeout(
        Duration::from_millis(250),
        TcpStream::connect(("127.0.0.1", OFFICIAL_PMA_PORT)),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
}

pub async fn get_official_phpmyadmin_status() -> OfficialPhpMyAdminStatus {
    let installed = Path::new(OFFICIAL_PMA_DIR).join("index.php").is_file();
    let running = if installed {
        is_official_pma_port_open().await
    } else {
        false
    };
    OfficialPhpMyAdminStatus {
        installed,
        running,
        version: if installed {
            detect_installed_pma_version()
        } else {
            OFFICIAL_PMA_VERSION.to_string()
        },
        install_dir: OFFICIAL_PMA_DIR.to_string(),
        official_url: "/phpmyadmin/".to_string(),
        custom_panel_url: "/phpadmin/".to_string(),
    }
}

fn generate_blowfish_secret() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#%^*()-_=+";
    let mut bytes = [0u8; 32];
    if OsRng.try_fill_bytes(&mut bytes).is_err() {
        return "FerrousPanelOfficialPmaSecretKey!".to_string();
    }
    bytes
        .iter()
        .map(|&b| ALPHABET[(b as usize) % ALPHABET.len()] as char)
        .collect()
}

fn render_official_pma_config_inc(secret: &str) -> String {
    format!(
        "<?php\n\
declare(strict_types=1);\n\
$cfg['blowfish_secret'] = '{secret}';\n\
$i = 0;\n\
$i++;\n\
$cfg['Servers'][$i]['auth_type'] = 'cookie';\n\
$cfg['Servers'][$i]['host'] = 'localhost';\n\
$cfg['Servers'][$i]['compress'] = false;\n\
$cfg['Servers'][$i]['AllowNoPassword'] = false;\n\
if (file_exists('/run/mysqld/mysqld.sock')) {{\n\
    $cfg['Servers'][$i]['socket'] = '/run/mysqld/mysqld.sock';\n\
}}\n\
$i++;\n\
$cfg['Servers'][$i]['verbose'] = '127.0.0.1 (TCP)';\n\
$cfg['Servers'][$i]['auth_type'] = 'cookie';\n\
$cfg['Servers'][$i]['host'] = '127.0.0.1';\n\
$cfg['Servers'][$i]['port'] = '3306';\n\
$cfg['Servers'][$i]['compress'] = false;\n\
$cfg['Servers'][$i]['AllowNoPassword'] = false;\n\
$cfg['UploadDir'] = '';\n\
$cfg['SaveDir'] = '';\n\
$cfg['TempDir'] = '{OFFICIAL_PMA_TMP_DIR}';\n\
$cfg['ExecTimeLimit'] = 300;\n\
$cfg['SendErrorReports'] = 'never';\n"
    )
}

fn render_official_pma_router_php() -> &'static str {
    r#"<?php
if (!empty($_SERVER['HTTP_X_FORWARDED_PROTO']) && strtolower($_SERVER['HTTP_X_FORWARDED_PROTO']) === 'https') {
    $_SERVER['HTTPS'] = 'on';
    $_SERVER['SERVER_PORT'] = '443';
}
if (!empty($_SERVER['HTTP_X_FORWARDED_HOST'])) {
    $_SERVER['HTTP_HOST'] = $_SERVER['HTTP_X_FORWARDED_HOST'];
}
$uri = parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH) ?: '/';
if ($uri === '/phpmyadmin') {
    header('Location: /phpmyadmin/', true, 302);
    exit;
}
if (strpos($uri, '/phpmyadmin/') !== 0) {
    http_response_code(403);
    echo 'Forbidden';
    exit;
}
$rel = substr($uri, strlen('/phpmyadmin'));
if ($rel === '' || $rel === '/') {
    $rel = '/index.php';
}
$base = realpath('/www/server/phpmyadmin');
if ($base === false) {
    http_response_code(503);
    echo 'Official phpMyAdmin directory missing';
    exit;
}
$candidate = $base . $rel;
$real = realpath($candidate);
if ($real === false || strpos($real, $base) !== 0) {
    http_response_code(404);
    echo 'Not Found';
    exit;
}
if (is_dir($real)) {
    $idx = $real . '/index.php';
    if (is_file($idx)) {
        $real = $idx;
        $rel = rtrim($rel, '/') . '/index.php';
    } else {
        http_response_code(403);
        echo 'Forbidden';
        exit;
    }
}
$ext = strtolower(pathinfo($real, PATHINFO_EXTENSION));
if ($ext !== 'php') {
    return false;
}
chdir(dirname($real));
$_SERVER['SCRIPT_FILENAME'] = $real;
$_SERVER['SCRIPT_NAME'] = '/phpmyadmin' . $rel;
$_SERVER['PHP_SELF'] = '/phpmyadmin' . $rel;
require $real;
"#
}

fn render_official_pma_systemd_unit(php_bin: &str) -> String {
    format!(
        "[Unit]\n\
Description=FerrousPanel Official phpMyAdmin Worker (127.0.0.1:{OFFICIAL_PMA_PORT})\n\
After=network.target mariadb.service mysql.service\n\
\n\
[Service]\n\
Type=simple\n\
User=www-data\n\
Group=www-data\n\
WorkingDirectory={OFFICIAL_PMA_DIR}\n\
Environment=PHP_CLI_SERVER_WORKERS=4\n\
ExecStart={php_bin} -S 127.0.0.1:{OFFICIAL_PMA_PORT} -d upload_max_filesize=128M -d post_max_size=128M -d memory_limit=256M -d max_execution_time=300 -t /www/server {OFFICIAL_PMA_ROUTER}\n\
Restart=always\n\
RestartSec=2\n\
NoNewPrivileges=true\n\
PrivateTmp=true\n\
\n\
[Install]\n\
WantedBy=multi-user.target\n"
    )
}

/// Downloads & configures Official phpMyAdmin from the official release archive (`phpmyadmin.net` / GitHub `phpmyadmin/phpmyadmin`)
/// into `/www/server/phpmyadmin` and starts `ferrous-phpmyadmin.service` on `127.0.0.1:889`, while keeping
/// FerrousAdmin (`/phpadmin/`) active in parallel.
pub async fn install_or_repair_official_phpmyadmin(
    executor: &dyn CommandExecutor,
) -> Result<OfficialPhpMyAdminStatus, PanelError> {
    let apt_timeout = Duration::from_secs(240);
    let cmd_timeout = Duration::from_secs(90);

    // 1. Ensure PHP CLI + required phpMyAdmin extensions (mysqli, mbstring, xml, zip, curl) are installed.
    let needs_php_pkgs = find_php_cli_binary_path().is_none()
        || !Path::new("/usr/share/php").exists()
        || {
            if let Ok(out) = executor
                .run(AllowedBinary::Php83, &["-m"], Duration::from_secs(5))
                .await
            {
                let lower = out.stdout.to_ascii_lowercase();
                !lower.contains("mysqli") || !lower.contains("mbstring")
            } else {
                true
            }
        };

    if needs_php_pkgs && executor.is_binary_available(AllowedBinary::AptGet) {
        let _ = executor
            .run(AllowedBinary::AptGet, &["update", "-qq"], apt_timeout)
            .await;
        let _ = executor
            .run(
                AllowedBinary::AptGet,
                &[
                    "install",
                    "-y",
                    "-qq",
                    "php-cli",
                    "php-mysql",
                    "php-mbstring",
                    "php-xml",
                    "php-zip",
                    "php-curl",
                ],
                apt_timeout,
            )
            .await;
    }

    let Some(php_bin) = find_php_cli_binary_path() else {
        return Err(PanelError::Conflict(
            "Chưa tìm thấy PHP CLI trên máy chủ — hãy cài PHP (ví dụ: sudo apt install -y php-cli php-mysql php-mbstring php-xml) trước khi kích hoạt phpMyAdmin chính thức."
                .to_string(),
        ));
    };

    // 2. Download Official phpMyAdmin archive if not yet present in /www/server/phpmyadmin/index.php.
    let pma_dir = Path::new(OFFICIAL_PMA_DIR);
    if !pma_dir.join("index.php").is_file() {
        std::fs::create_dir_all(pma_dir)?;
        let tmp_tar = std::env::temp_dir().join(format!(
            "ferrous-official-phpmyadmin-{}.tar.gz",
            std::process::id()
        ));
        let tmp_tar_str = tmp_tar.display().to_string();
        let urls = [
            "https://files.phpmyadmin.net/phpMyAdmin/5.2.2/phpMyAdmin-5.2.2-all-languages.tar.gz",
            "https://github.com/phpmyadmin/phpmyadmin/releases/download/RELEASE_5_2_2/phpMyAdmin-5.2.2-all-languages.tar.gz",
            "https://files.phpmyadmin.net/phpMyAdmin/5.2.1/phpMyAdmin-5.2.1-all-languages.tar.gz",
        ];
        let mut downloaded = false;
        for url in urls {
            if let Ok(out) = executor
                .run(
                    AllowedBinary::Curl,
                    &["-fsSL", "--connect-timeout", "15", "-o", &tmp_tar_str, url],
                    cmd_timeout,
                )
                .await
            {
                if out.success && tmp_tar.is_file() {
                    downloaded = true;
                    break;
                }
            }
        }
        if !downloaded {
            let _ = std::fs::remove_file(&tmp_tar);
            return Err(PanelError::BadGateway(
                "Không thể tải gói phpMyAdmin chính thức từ files.phpmyadmin.net / GitHub. Vui lòng kiểm tra kết nối mạng của VPS."
                    .to_string(),
            ));
        }

        let extract = executor
            .run(
                AllowedBinary::Tar,
                &[
                    "-xzf",
                    &tmp_tar_str,
                    "--strip-components=1",
                    "-C",
                    OFFICIAL_PMA_DIR,
                ],
                cmd_timeout,
            )
            .await;
        let _ = std::fs::remove_file(&tmp_tar);
        let extract_out = extract?;
        if !extract_out.success || !pma_dir.join("index.php").is_file() {
            return Err(PanelError::CommandFailed {
                binary: AllowedBinary::Tar.label(),
                exit_code: extract_out.exit_code,
                stderr: extract_out.stderr,
            });
        }
    }

    // 3. Prepare tmp dir, config.inc.php, router, permissions, and systemd unit.
    std::fs::create_dir_all(OFFICIAL_PMA_TMP_DIR)?;
    let cfg_inc = pma_dir.join("config.inc.php");
    if !cfg_inc.is_file() {
        let secret = generate_blowfish_secret();
        atomic_write_file(&cfg_inc, render_official_pma_config_inc(&secret).as_bytes(), 0o640)?;
    }
    atomic_write_file(
        Path::new(OFFICIAL_PMA_ROUTER),
        render_official_pma_router_php().as_bytes(),
        0o644,
    )?;

    if executor.is_binary_available(AllowedBinary::Chown) {
        let _ = executor
            .run(
                AllowedBinary::Chown,
                &["-R", "www-data:www-data", OFFICIAL_PMA_DIR, OFFICIAL_PMA_TMP_DIR],
                Duration::from_secs(20),
            )
            .await;
    }

    atomic_write_file(
        Path::new(OFFICIAL_PMA_UNIT_PATH),
        render_official_pma_systemd_unit(php_bin).as_bytes(),
        0o644,
    )?;

    if executor.is_binary_available(AllowedBinary::Systemctl) {
        let _ = executor
            .run(AllowedBinary::Systemctl, &["daemon-reload"], Duration::from_secs(15))
            .await;
        let _ = executor
            .run(
                AllowedBinary::Systemctl,
                &["enable", OFFICIAL_PMA_UNIT_NAME],
                Duration::from_secs(15),
            )
            .await;
        let _ = executor
            .run(
                AllowedBinary::Systemctl,
                &["restart", OFFICIAL_PMA_UNIT_NAME],
                Duration::from_secs(15),
            )
            .await;
    }

    for _ in 0..15 {
        if is_official_pma_port_open().await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(180)).await;
    }

    Ok(get_official_phpmyadmin_status().await)
}

pub async fn redirect_phpmyadmin_slash(req: HttpRequest) -> HttpResponse {
    let qs = req.query_string();
    let loc = if qs.is_empty() {
        "/phpmyadmin/".to_string()
    } else {
        format!("/phpmyadmin/?{qs}")
    };
    HttpResponse::Found()
        .insert_header(("Location", loc))
        .insert_header(("Cache-Control", "no-store"))
        .finish()
}

fn decode_chunked_body(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut pos = 0usize;
    while pos < raw.len() {
        let Some(rel_end) = raw[pos..].windows(2).position(|w| w == b"\r\n") else {
            break;
        };
        let size_line = String::from_utf8_lossy(&raw[pos..pos + rel_end]);
        let hex_part = size_line.split(';').next().unwrap_or("").trim();
        let Ok(chunk_size) = usize::from_str_radix(hex_part, 16) else {
            break;
        };
        pos += rel_end + 2;
        if chunk_size == 0 {
            break;
        }
        let end = pos.saturating_add(chunk_size).min(raw.len());
        out.extend_from_slice(&raw[pos..end]);
        pos = (end + 2).min(raw.len());
    }
    out
}

fn render_official_pma_launcher_html() -> &'static str {
    r##"<!DOCTYPE html>
<html lang="vi" class="dark">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>phpMyAdmin Chính thức &amp; FerrousAdmin — FerrousPanel</title>
  <style>
    body { margin:0; font-family:system-ui,-apple-system,sans-serif; background:#090d16; color:#e2e8f0; display:flex; align-items:center; justify-content:center; min-height:100vh; padding:20px; }
    .card { max-width:580px; width:100%; background:#111827; border:1px solid #1e293b; border-radius:14px; padding:26px; box-shadow:0 24px 48px rgba(0,0,0,.45); }
    h1 { margin:0 0 8px; font-size:20px; }
    p { margin:0 0 16px; color:#94a3b8; font-size:14px; line-height:1.55; }
    .actions { display:flex; gap:10px; flex-wrap:wrap; margin-top:18px; }
    .btn { display:inline-flex; align-items:center; justify-content:center; padding:10px 16px; border-radius:9px; font-weight:600; font-size:13px; text-decoration:none; cursor:pointer; border:1px solid #334155; background:#1e293b; color:#e2e8f0; }
    .btn-primary { background:#f97316; border-color:#ea580c; color:#fff; }
    .btn-blue { background:rgba(59,130,246,.18); border-color:rgba(59,130,246,.45); color:#93c5fd; }
    .code { font-family:ui-monospace,monospace; background:#020617; border:1px solid #1e293b; border-radius:8px; padding:10px 12px; font-size:12px; color:#cbd5e1; overflow-x:auto; }
    #msg { margin-top:14px; font-size:13px; display:none; padding:10px 12px; border-radius:8px; }
  </style>
</head>
<body>
  <div class="card">
    <h1>Quản trị Cơ sở dữ liệu Kép (Dual-Mode)</h1>
    <p>FerrousPanel hỗ trợ song song cả hai trình quản trị CSDL trên mọi tên miền và IP:</p>
    <ul style="color:#cbd5e1;font-size:13.5px;line-height:1.7;padding-left:18px;margin:0 0 16px;">
      <li><strong>phpMyAdmin Chính thức</strong> tại <code>/phpmyadmin/</code> (mã nguồn chính thức từ <code>phpmyadmin.net</code>)</li>
      <li><strong>FerrousAdmin (Trang tuỳ chỉnh của Panel)</strong> tại <code>/phpadmin/</code> (100% Pure Rust, siêu tốc)</li>
    </ul>
    <p style="margin-bottom:8px;">Hiện tại dịch vụ <code>phpMyAdmin Chính thức</code> chưa được kích hoạt trên máy chủ này. Bạn có thể bấm nút cài tự động bên dưới (nếu đang đăng nhập Panel) hoặc chạy lệnh cập nhật:</p>
    <div class="code">sudo fp update</div>
    <div id="msg"></div>
    <div class="actions">
      <button type="button" class="btn btn-primary" id="btnInstallOfficial">Tải &amp; Kích hoạt phpMyAdmin Chính thức ngay</button>
      <a href="/phpadmin/" class="btn btn-blue">Mở Trang CSDL Tuỳ chỉnh của Panel (/phpadmin/) →</a>
    </div>
  </div>
  <script>
    document.getElementById('btnInstallOfficial').addEventListener('click', async function () {
      const btn = this;
      const msg = document.getElementById('msg');
      btn.disabled = true;
      btn.textContent = 'Đang tải từ phpmyadmin.net & khởi chạy…';
      msg.style.display = 'block';
      msg.style.background = 'rgba(59,130,246,.14)';
      msg.style.color = '#93c5fd';
      msg.textContent = 'Đang cài đặt phpMyAdmin chính thức (khoảng 10–30 giây)…';
      try {
        const tok = sessionStorage.getItem('fp_token') || localStorage.getItem('fp_token') || '';
        const headers = { 'Content-Type': 'application/json', 'X-Requested-With': 'FerrousPanel-UI' };
        if (tok) headers['Authorization'] = 'Bearer ' + tok;
        const resp = await fetch('/api/databases/phpmyadmin/install', { method: 'POST', credentials: 'same-origin', headers, body: '{}' });
        const data = await resp.json().catch(() => ({}));
        if (!resp.ok) throw new Error(data.error || ('HTTP ' + resp.status + ' — Hãy đăng nhập Panel rồi vào mục Cơ sở dữ liệu để bấm Kích hoạt phpMyAdmin.'));
        msg.style.background = 'rgba(16,185,129,.16)';
        msg.style.color = '#6ee7b7';
        msg.textContent = 'Đã cài đặt & khởi chạy phpMyAdmin chính thức! Đang tải trang…';
        setTimeout(() => window.location.reload(), 600);
      } catch (e) {
        btn.disabled = false;
        btn.textContent = 'Thử kích hoạt lại';
        msg.style.background = 'rgba(239,68,68,.16)';
        msg.style.color = '#fca5a5';
        msg.textContent = e.message;
      }
    });
  </script>
</body>
</html>"##
}

/// Reverse-proxies `/phpmyadmin/*` (all HTTP methods, cookies, sessions, SQL uploads/exports)
/// to the Official phpMyAdmin worker on `127.0.0.1:889`, auto-starting the systemd unit if
/// installed on disk, or serving the Dual-Mode 1-Click Launcher if not yet installed.
pub async fn serve_official_phpmyadmin(
    req: HttpRequest,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> HttpResponse {
    let mut stream_opt = TcpStream::connect(("127.0.0.1", OFFICIAL_PMA_PORT)).await.ok();

    if stream_opt.is_none()
        && state.config.is_production()
        && Path::new(OFFICIAL_PMA_DIR).join("index.php").is_file()
        && state
            .webserver
            .executor()
            .is_binary_available(AllowedBinary::Systemctl)
    {
        let _ = state
            .webserver
            .executor()
            .run(
                AllowedBinary::Systemctl,
                &["start", OFFICIAL_PMA_UNIT_NAME],
                Duration::from_secs(8),
            )
            .await;
        for _ in 0..8 {
            tokio::time::sleep(Duration::from_millis(150)).await;
            if let Ok(s) = TcpStream::connect(("127.0.0.1", OFFICIAL_PMA_PORT)).await {
                stream_opt = Some(s);
                break;
            }
        }
    }

    let Some(mut stream) = stream_opt else {
        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .insert_header(("Cache-Control", "no-store"))
            .body(render_official_pma_launcher_html());
    };

    let method = req.method().as_str();
    let path = req.uri().path();
    let target_uri = match req.uri().query() {
        Some(q) if !q.is_empty() => format!("{path}?{q}"),
        _ => path.to_string(),
    };
    let (host, scheme) = {
        let conn_info = req.connection_info();
        (
            conn_info.host().to_string(),
            conn_info.scheme().to_string(),
        )
    };

    let mut raw_req = Vec::with_capacity(512 + body.len());
    raw_req.extend_from_slice(format!("{method} {target_uri} HTTP/1.1\r\n").as_bytes());
    raw_req.extend_from_slice(format!("Host: {host}\r\n").as_bytes());
    raw_req.extend_from_slice(b"Connection: close\r\n");
    raw_req.extend_from_slice(format!("X-Forwarded-Host: {host}\r\n").as_bytes());
    raw_req.extend_from_slice(format!("X-Forwarded-Proto: {scheme}\r\n").as_bytes());

    for (hname, hval) in req.headers() {
        let lower = hname.as_str().to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "host"
                | "connection"
                | "transfer-encoding"
                | "content-length"
                | "accept-encoding"
                | "x-forwarded-host"
                | "x-forwarded-proto"
        ) {
            continue;
        }
        if let Ok(v) = hval.to_str() {
            if !v.contains('\r') && !v.contains('\n') {
                raw_req.extend_from_slice(format!("{}: {v}\r\n", hname.as_str()).as_bytes());
            }
        }
    }
    raw_req.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
    raw_req.extend_from_slice(&body);

    if tokio::time::timeout(Duration::from_secs(60), stream.write_all(&raw_req))
        .await
        .map_err(|_| ())
        .and_then(|r| r.map_err(|_| ()))
        .is_err()
    {
        return HttpResponse::BadGateway()
            .content_type("text/plain; charset=utf-8")
            .body("Failed to forward request to Official phpMyAdmin worker");
    }

    let mut resp_bytes = Vec::with_capacity(16 * 1024);
    if tokio::time::timeout(Duration::from_secs(120), stream.read_to_end(&mut resp_bytes))
        .await
        .map_err(|_| ())
        .and_then(|r| r.map_err(|_| ()))
        .is_err()
    {
        return HttpResponse::GatewayTimeout()
            .content_type("text/plain; charset=utf-8")
            .body("Official phpMyAdmin worker timed out");
    }

    let Some(hdr_end) = resp_bytes.windows(4).position(|w| w == b"\r\n\r\n") else {
        return HttpResponse::BadGateway()
            .content_type("text/plain; charset=utf-8")
            .body("Invalid upstream response from Official phpMyAdmin worker");
    };

    let header_text = String::from_utf8_lossy(&resp_bytes[..hdr_end]);
    let raw_body = &resp_bytes[hdr_end + 4..];
    let mut lines = header_text.split("\r\n");
    let status_line = lines.next().unwrap_or("HTTP/1.1 200 OK");
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .and_then(|c| StatusCode::from_u16(c).ok())
        .unwrap_or(StatusCode::OK);

    let mut builder = HttpResponse::build(status_code);
    let mut is_chunked = false;
    let mut is_html = false;

    for line in lines {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let key_trim = k.trim();
        let val_trim = v.trim();
        let lower = key_trim.to_ascii_lowercase();
        if lower == "transfer-encoding" && val_trim.to_ascii_lowercase().contains("chunked") {
            is_chunked = true;
            continue;
        }
        if matches!(
            lower.as_str(),
            "connection" | "content-length" | "transfer-encoding" | "keep-alive"
        ) {
            continue;
        }
        if lower == "content-type" && val_trim.to_ascii_lowercase().contains("text/html") {
            is_html = true;
        }
        builder.append_header((key_trim.to_string(), val_trim.to_string()));
    }

    let mut decoded_body = if is_chunked {
        decode_chunked_body(raw_body)
    } else {
        raw_body.to_vec()
    };

    // Inject a subtle quick-switch pill in HTML pages so users can switch to Panel's custom DB UI (/phpadmin/) anytime.
    if is_html && method != "HEAD" {
        let badge = b"<a href=\"/phpadmin/\" style=\"position:fixed;bottom:12px;right:12px;z-index:99999;background:#0f172a;color:#fb923c;border:1px solid #f97316;padding:6px 12px;border-radius:999px;font:600 12px system-ui,sans-serif;text-decoration:none;box-shadow:0 4px 14px rgba(0,0,0,.35)\">\xE2\x87\x84 Trang CSDL Panel (/phpadmin/)</a></body>";
        if let Some(pos) = decoded_body
            .windows(7)
            .rposition(|w| w.eq_ignore_ascii_case(b"</body>"))
        {
            decoded_body.splice(pos..pos + 7, badge.iter().copied());
        }
    }

    builder.body(decoded_body)
}

