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
  <meta name="generator" content="FPSql &amp; Database (FerrousAdmin Pure-Rust)" />
  <title>FPSql &amp; Database — Pure-Rust SQL Studio (/phpadmin/)</title>
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
      <span>FPSql &amp; Database</span>
      <span class="badge">100% Pure Rust · Zero PHP</span>
      <span class="badge badge-ok" id="engineStatus">Actix-Web + SQLite WAL + MariaDB + PostgreSQL</span>
    </div>
    <div class="row" id="headerActions">
      <a href="/phpmyadmin/" style="font-size:12px;font-weight:600;padding:7px 12px;border-radius:7px;border:1px solid rgba(59,130,246,0.45);background:rgba(59,130,246,0.16);color:#93c5fd;text-decoration:none;">⇄ __PMA_DISPLAY_NAME__ (/phpmyadmin/) ↗</a>
      <a href="/pgadmin/" style="font-size:12px;font-weight:600;padding:7px 12px;border-radius:7px;border:1px solid rgba(56,189,248,0.45);background:rgba(56,189,248,0.14);color:#7dd3fc;text-decoration:none;">🐘 pgAdmin 4 (/pgadmin/) ↗</a>
      <button type="button" id="btnPmaLang" style="font-family:var(--mono);font-weight:700;">EN</button>
      <span class="muted" id="userLabel"></span>
      <button type="button" id="btnRefresh" class="hidden">Làm mới</button>
      <button type="button" id="btnLogout" class="danger hidden">Đăng xuất</button>
    </div>
  </header>

  <section id="loginView" class="login-box">
    <h2 style="margin:0 0 6px;font-size:18px;" data-en="Sign in to FPSql &amp; Database">Đăng nhập FPSql &amp; Database</h2>
    <p class="muted" style="margin:0 0 16px;" data-en="Pure-Rust SQL Studio for FerrousPanel. Sign in with your FerrousPanel admin credentials (Argon2id).">Quản trị SQL thuần Rust của FerrousPanel. Sử dụng tài khoản quản trị FerrousPanel (Argon2id).</p>
    <div id="loginError" class="notice err hidden"></div>
    <form id="loginForm" style="display:flex;flex-direction:column;gap:12px;">
      <div>
        <label for="loginUser" data-en="Username">Tài khoản</label>
        <input id="loginUser" type="text" value="admin_ferrous" required autocomplete="username" />
      </div>
      <div>
        <label for="loginPass" data-en="Password">Mật khẩu</label>
        <input id="loginPass" type="password" required autocomplete="current-password" />
      </div>
      <button type="submit" class="primary" data-en="Sign In to FPSql &amp; Database">Đăng nhập SQL Studio</button>
    </form>
  </section>

  <div id="appView" class="layout hidden">
    <aside>
      <div class="row" style="justify-content:space-between;margin-bottom:10px;">
        <strong style="font-size:13px;" data-en="Databases">Cơ sở dữ liệu</strong>
        <button type="button" class="primary" id="btnShowCreateDb" data-en="+ New DB">+ Tạo DB</button>
      </div>
      <div id="dbList"></div>
    </aside>
    <main>
      <div id="banner" class="notice hidden"></div>
      <div class="tabs" id="mainTabs">
        <button type="button" class="tab active" data-tab="databases">Databases</button>
        <button type="button" class="tab" data-tab="structure" data-en="Structure (Tables &amp; Columns)">Structure (Bảng &amp; Cột)</button>
        <button type="button" class="tab" data-tab="browse" data-en="Browse &amp; Edit Rows">Browse &amp; Sửa Dòng</button>
        <button type="button" class="tab" data-tab="insert" data-en="Insert Row">Chèn Dòng (Insert)</button>
        <button type="button" class="tab" data-tab="sql">SQL Console</button>
        <button type="button" class="tab" data-tab="new_table" data-en="Create Table">Tạo Bảng</button>
        <button type="button" class="tab" data-tab="export">Export .SQL</button>
        <button type="button" class="tab" data-tab="import">Import .SQL</button>
        <button type="button" class="tab" data-tab="server_cfg" data-en="MySQL / PMA Config">Cấu hình MySQL / PMA</button>
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
      const ssoParamToken = urlParams.get('fp_sso') || '';
      if (ssoParamToken) {
        sessionStorage.setItem('fp_pma_token', ssoParamToken);
        sessionStorage.setItem('fp_session_token', ssoParamToken);
      }
      let authToken = ssoParamToken
        || sessionStorage.getItem('fp_session_token')
        || localStorage.getItem('fp_session_token')
        || sessionStorage.getItem('fp_pma_token')
        || localStorage.getItem('fp_token')
        || '';

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

      const pmaLangBtn = document.getElementById('btnPmaLang');
      function applyPmaLang() {
        const en = (localStorage.getItem('fp_lang') || 'vi') === 'en';
        document.documentElement.lang = en ? 'en' : 'vi';
        if (pmaLangBtn) pmaLangBtn.textContent = en ? 'VI' : 'EN';
        document.getElementById('btnRefresh').textContent = en ? 'Refresh' : 'Làm mới';
        document.getElementById('btnLogout').textContent = en ? 'Sign Out' : 'Đăng xuất';
        document.getElementById('pmaDialogCancel').textContent = en ? 'Cancel' : 'Hủy';
        document.querySelectorAll('[data-en]').forEach((el) => {
          if (!el.hasAttribute('data-vi')) el.setAttribute('data-vi', el.textContent);
          el.textContent = en ? el.getAttribute('data-en') : el.getAttribute('data-vi');
        });
      }
      if (pmaLangBtn) {
        pmaLangBtn.addEventListener('click', () => {
          const next = (localStorage.getItem('fp_lang') || 'vi') === 'en' ? 'vi' : 'en';
          localStorage.setItem('fp_lang', next);
          applyPmaLang();
          renderSidebar();
          if (activeTab === 'browse' && activeTable) { loadBrowseRows(); } else { renderTab(); }
        });
      }
      applyPmaLang();

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
    let pma_label = format!("Myphpadmin {}", detect_installed_pma_version());
    let body = FERROUS_ADMIN_HTML.replace("__PMA_DISPLAY_NAME__", &pma_label);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .insert_header(("X-Frame-Options", "SAMEORIGIN"))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .insert_header(("Cache-Control", "no-store"))
        .body(body)
}

pub const OFFICIAL_PMA_DIR: &str = "/www/server/phpmyadmin";
pub const OFFICIAL_PMA_ROUTER: &str = "/www/server/ferrous-pma-router.php";
pub const OFFICIAL_PMA_TMP_DIR: &str = "/var/lib/phpmyadmin/tmp";
pub const OFFICIAL_PMA_UNIT_NAME: &str = "ferrous-phpmyadmin.service";
pub const OFFICIAL_PMA_UNIT_PATH: &str = "/etc/systemd/system/ferrous-phpmyadmin.service";
/// Unprivileged loopback port (>= 1024) so `User=www-data` binds cleanly under `NoNewPrivileges=true`.
pub const OFFICIAL_PMA_PORT: u16 = 18889;
pub const OFFICIAL_PMA_VERSION: &str = "5.2.3";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialPhpMyAdminStatus {
    pub installed: bool,
    pub running: bool,
    pub version: String,
    pub display_name: String,
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

fn find_php_cli_binary() -> Option<(AllowedBinary, &'static str, Option<&'static str>)> {
    [
        (AllowedBinary::Php83, "/usr/bin/php8.3", Some("8.3")),
        (AllowedBinary::Php82, "/usr/bin/php8.2", Some("8.2")),
        (AllowedBinary::Php81, "/usr/bin/php8.1", Some("8.1")),
        (AllowedBinary::Php84, "/usr/bin/php8.4", Some("8.4")),
        (AllowedBinary::Php83, "/usr/bin/php", None),
        (AllowedBinary::Php74, "/usr/bin/php7.4", Some("7.4")),
    ]
    .into_iter()
    .find(|(_, candidate, _)| Path::new(candidate).is_file())
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
    let version = if installed {
        detect_installed_pma_version()
    } else {
        OFFICIAL_PMA_VERSION.to_string()
    };
    OfficialPhpMyAdminStatus {
        installed,
        running,
        display_name: format!("Myphpadmin {version}"),
        version,
        install_dir: OFFICIAL_PMA_DIR.to_string(),
        official_url: "/phpmyadmin/".to_string(),
        custom_panel_url: "/phpadmin/".to_string(),
    }
}

fn generate_blowfish_secret() -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#%^*()-_=+";
    let mut bytes = [0u8; 32];
    if OsRng.try_fill_bytes(&mut bytes).is_err() {
        return "FerrousPanelOfficialPmaSecretKey".to_string();
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
$ferrousSsoFile = '{sso_file}';\n\
$ferrousSsoLegacyFile = '{sso_legacy_file}';\n\
$ferrousSsoActive = !empty($_SERVER['HTTP_X_FERROUS_PMA_AUTH'])\n\
    && $_SERVER['HTTP_X_FERROUS_PMA_AUTH'] === '1';\n\
$ferrousSsoPass = '';\n\
if ($ferrousSsoActive) {{\n\
    if (!empty($_SERVER['HTTP_X_FERROUS_PMA_PASS'])) {{\n\
        $ferrousSsoPass = trim((string) $_SERVER['HTTP_X_FERROUS_PMA_PASS']);\n\
    }} elseif (is_readable($ferrousSsoFile)) {{\n\
        $ferrousSsoPass = trim((string) @file_get_contents($ferrousSsoFile));\n\
    }} elseif (is_readable($ferrousSsoLegacyFile)) {{\n\
        $ferrousSsoPass = trim((string) @file_get_contents($ferrousSsoLegacyFile));\n\
    }}\n\
}}\n\
for ($i = 1; $i <= 2; $i++) {{\n\
    $cfg['Servers'][$i]['verbose'] = $i === 1\n\
        ? 'FPSql & Database — MariaDB / MySQL (Local Socket)'\n\
        : 'FPSql & Database — MariaDB / MySQL (127.0.0.1:3306)';\n\
    if ($ferrousSsoPass !== '') {{\n\
        $cfg['Servers'][$i]['auth_type'] = 'config';\n\
        $cfg['Servers'][$i]['user'] = 'fp_myphpadmin';\n\
        $cfg['Servers'][$i]['password'] = $ferrousSsoPass;\n\
        $cfg['Servers'][$i]['controluser'] = 'fp_myphpadmin';\n\
        $cfg['Servers'][$i]['controlpass'] = $ferrousSsoPass;\n\
    }} else {{\n\
        $cfg['Servers'][$i]['auth_type'] = 'cookie';\n\
    }}\n\
    $cfg['Servers'][$i]['host'] = $i === 1 ? 'localhost' : '127.0.0.1';\n\
    $cfg['Servers'][$i]['controlhost'] = $cfg['Servers'][$i]['host'];\n\
    if ($i === 2) {{\n\
        $cfg['Servers'][$i]['port'] = '3306';\n\
    }} elseif (file_exists('/run/mysqld/mysqld.sock')) {{\n\
        $cfg['Servers'][$i]['socket'] = '/run/mysqld/mysqld.sock';\n\
    }}\n\
    $cfg['Servers'][$i]['compress'] = false;\n\
    $cfg['Servers'][$i]['AllowNoPassword'] = false;\n\
    $cfg['Servers'][$i]['pmadb'] = 'phpmyadmin';\n\
    $cfg['Servers'][$i]['bookmarktable'] = 'pma__bookmark';\n\
    $cfg['Servers'][$i]['relation'] = 'pma__relation';\n\
    $cfg['Servers'][$i]['table_info'] = 'pma__table_info';\n\
    $cfg['Servers'][$i]['table_coords'] = 'pma__table_coords';\n\
    $cfg['Servers'][$i]['pdf_pages'] = 'pma__pdf_pages';\n\
    $cfg['Servers'][$i]['column_info'] = 'pma__column_info';\n\
    $cfg['Servers'][$i]['history'] = 'pma__history';\n\
    $cfg['Servers'][$i]['table_uiprefs'] = 'pma__table_uiprefs';\n\
    $cfg['Servers'][$i]['tracking'] = 'pma__tracking';\n\
    $cfg['Servers'][$i]['userconfig'] = 'pma__userconfig';\n\
    $cfg['Servers'][$i]['recent'] = 'pma__recent';\n\
    $cfg['Servers'][$i]['favorite'] = 'pma__favorite';\n\
    $cfg['Servers'][$i]['users'] = 'pma__users';\n\
    $cfg['Servers'][$i]['usergroups'] = 'pma__usergroups';\n\
    $cfg['Servers'][$i]['navigationhiding'] = 'pma__navigationhiding';\n\
    $cfg['Servers'][$i]['savedsearches'] = 'pma__savedsearches';\n\
    $cfg['Servers'][$i]['central_columns'] = 'pma__central_columns';\n\
    $cfg['Servers'][$i]['designer_settings'] = 'pma__designer_settings';\n\
    $cfg['Servers'][$i]['export_templates'] = 'pma__export_templates';\n\
}}\n\
$cfg['PmaNoRelation_DisableWarning'] = true;\n\
$cfg['ZeroConf'] = true;\n\
$cfg['VersionCheck'] = false;\n\
$cfg['LoginCookieValidity'] = 86400;\n\
$cfg['UploadDir'] = '';\n\
$cfg['SaveDir'] = '';\n\
$cfg['TempDir'] = '{OFFICIAL_PMA_TMP_DIR}';\n\
$cfg['AllowThirdPartyFraming'] = 'sameorigin';\n\
$cfg['ExecTimeLimit'] = 300;\n\
$cfg['SendErrorReports'] = 'never';\n",
        sso_file = crate::modules::databases::MYPHPADMIN_SSO_PASS_FILE,
        sso_legacy_file = crate::modules::databases::MYPHPADMIN_SSO_PASS_FILE_LEGACY
    )
}

fn render_official_pma_router_php() -> &'static str {
    r#"<?php
$isHttps = !empty($_SERVER['HTTP_X_FORWARDED_PROTO']) && strtolower($_SERVER['HTTP_X_FORWARDED_PROTO']) === 'https';
if ($isHttps) {
    $_SERVER['HTTPS'] = 'on';
}
if (!empty($_SERVER['HTTP_X_FORWARDED_HOST'])) {
    $_SERVER['HTTP_HOST'] = $_SERVER['HTTP_X_FORWARDED_HOST'];
}
$hostHeader = $_SERVER['HTTP_HOST'] ?? 'localhost';
if (strpos($hostHeader, ':') !== false && substr($hostHeader, -1) !== ']') {
    $parts = explode(':', $hostHeader);
    $portPart = end($parts);
    if (ctype_digit($portPart)) {
        $_SERVER['SERVER_PORT'] = $portPart;
        array_pop($parts);
        $_SERVER['SERVER_NAME'] = implode(':', $parts);
    }
} else {
    $_SERVER['SERVER_PORT'] = $isHttps ? '443' : '80';
    $_SERVER['SERVER_NAME'] = $hostHeader;
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
$blockedPrefixes = ['/setup', '/libraries', '/templates', '/vendor', '/sql', '/locale', '/.git', '/test'];
$blockedFiles = ['/config.inc.php', '/config.sample.inc.php', '/composer.json', '/composer.lock', '/package.json', '/yarn.lock'];
$relLower = strtolower($rel);
foreach ($blockedPrefixes as $pfx) {
    if ($relLower === $pfx || strpos($relLower, $pfx . '/') === 0) {
        http_response_code(403);
        echo 'Forbidden';
        exit;
    }
}
if (in_array($relLower, $blockedFiles, true)) {
    http_response_code(403);
    echo 'Forbidden';
    exit;
}
$base = realpath('/www/server/phpmyadmin');
if ($base === false) {
    http_response_code(503);
    echo 'Myphpadmin directory missing';
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
Description=FerrousPanel Myphpadmin Worker (127.0.0.1:{OFFICIAL_PMA_PORT})\n\
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

fn set_dir_mode(path: &str, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let p = Path::new(path);
    if p.exists() {
        let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode));
    }
}

fn ensure_config_allows_sameorigin_frame(cfg_inc: &Path) {
    if let Ok(content) = std::fs::read_to_string(cfg_inc) {
        if !content.contains("HTTP_X_FERROUS_PMA_PASS")
            || !content.contains("PmaNoRelation_DisableWarning")
            || !content.contains("pma__bookmark")
            || !content.contains("FPSql & Database")
        {
            let secret = generate_blowfish_secret();
            let _ = atomic_write_file(
                cfg_inc,
                render_official_pma_config_inc(&secret).as_bytes(),
                0o644,
            );
        }
    }
}

/// Downloads & configures Myphpadmin from `files.phpmyadmin.net` into `/www/server/phpmyadmin`
/// and starts `ferrous-phpmyadmin.service` on unprivileged loopback port `127.0.0.1:18889`,
/// keeping FerrousAdmin (`/phpadmin/`) active in parallel.
pub async fn install_or_repair_official_phpmyadmin(
    executor: &dyn CommandExecutor,
) -> Result<OfficialPhpMyAdminStatus, PanelError> {
    let apt_timeout = Duration::from_secs(240);
    let cmd_timeout = Duration::from_secs(120);
    let noninteractive_env = [("DEBIAN_FRONTEND", "noninteractive")];

    // 1. Ensure PHP CLI + required extensions (mysqli, mbstring, xml, zip, curl) are installed
    // for the EXACT active PHP version (important on Ubuntu with Ondrej PPA where `php-mysql`
    // may target a different default PHP version).
    let detected_php = find_php_cli_binary();
    let needs_php_pkgs = match detected_php {
        None => true,
        Some((bin, _, _)) => {
            if let Ok(out) = executor.run(bin, &["-m"], Duration::from_secs(5)).await {
                let lower = out.stdout.to_ascii_lowercase();
                !lower.contains("mysqli")
                    || !lower.contains("mbstring")
                    || !lower.contains("xml")
            } else {
                true
            }
        }
    };

    if needs_php_pkgs && executor.is_binary_available(AllowedBinary::AptGet) {
        let _ = executor
            .run_with_env(
                AllowedBinary::AptGet,
                &["update", "-qq"],
                &noninteractive_env,
                None,
                apt_timeout,
            )
            .await;

        if let Some((_, _, Some(ver))) = detected_php {
            let pkgs = [
                format!("php{ver}-cli"),
                format!("php{ver}-mysql"),
                format!("php{ver}-mbstring"),
                format!("php{ver}-xml"),
                format!("php{ver}-zip"),
                format!("php{ver}-curl"),
                format!("php{ver}-gd"),
                format!("php{ver}-intl"),
                format!("php{ver}-bz2"),
            ];
            let mut args: Vec<&str> = vec![
                "install",
                "-y",
                "-qq",
                "-o",
                "Dpkg::Options::=--force-confdef",
                "-o",
                "Dpkg::Options::=--force-confold",
            ];
            for p in &pkgs {
                args.push(p.as_str());
            }
            let _ = executor
                .run_with_env(
                    AllowedBinary::AptGet,
                    &args,
                    &noninteractive_env,
                    None,
                    apt_timeout,
                )
                .await;
        }

        if find_php_cli_binary().is_none() || detected_php.and_then(|(_, _, v)| v).is_none() {
            let _ = executor
                .run_with_env(
                    AllowedBinary::AptGet,
                    &[
                        "install",
                        "-y",
                        "-qq",
                        "-o",
                        "Dpkg::Options::=--force-confdef",
                        "-o",
                        "Dpkg::Options::=--force-confold",
                        "php-cli",
                        "php-mysql",
                        "php-mbstring",
                        "php-xml",
                        "php-zip",
                        "php-curl",
                        "php-gd",
                        "php-intl",
                        "php-bz2",
                    ],
                    &noninteractive_env,
                    None,
                    apt_timeout,
                )
                .await;
        }
    }

    let Some((_, php_bin, _)) = find_php_cli_binary() else {
        return Err(PanelError::Conflict(format!(
            "Chưa tìm thấy PHP CLI trên máy chủ — hãy cài PHP (ví dụ: sudo apt install -y php-cli php-mysql php-mbstring php-xml) trước khi kích hoạt Myphpadmin {OFFICIAL_PMA_VERSION}."
        )));
    };

    // 2. Download official phpMyAdmin release archive if not yet present in /www/server/phpmyadmin/index.php.
    let pma_dir = Path::new(OFFICIAL_PMA_DIR);
    std::fs::create_dir_all("/www/server")?;
    set_dir_mode("/www", 0o755);
    set_dir_mode("/www/server", 0o755);

    if !pma_dir.join("index.php").is_file() {
        std::fs::create_dir_all(pma_dir)?;
        let tmp_tar = std::env::temp_dir().join(format!(
            "ferrous-myphpadmin-{}.tar.gz",
            std::process::id()
        ));
        let tmp_tar_str = tmp_tar.display().to_string();
        let urls = [
            "https://files.phpmyadmin.net/phpMyAdmin/5.2.3/phpMyAdmin-5.2.3-all-languages.tar.gz",
            "https://files.phpmyadmin.net/phpMyAdmin/5.2.2/phpMyAdmin-5.2.2-all-languages.tar.gz",
            "https://files.phpmyadmin.net/phpMyAdmin/5.2.1/phpMyAdmin-5.2.1-all-languages.tar.gz",
        ];
        let mut downloaded = false;
        let mut last_curl_err = String::new();
        for url in urls {
            match executor
                .run(
                    AllowedBinary::Curl,
                    &[
                        "-fsSL",
                        "--retry",
                        "2",
                        "--connect-timeout",
                        "15",
                        "-o",
                        &tmp_tar_str,
                        url,
                    ],
                    cmd_timeout,
                )
                .await
            {
                Ok(out) if out.success && tmp_tar.is_file() => {
                    downloaded = true;
                    break;
                }
                Ok(out) => {
                    last_curl_err = out.stderr;
                }
                Err(err) => {
                    last_curl_err = err.to_string();
                }
            }
        }
        if !downloaded {
            let _ = std::fs::remove_file(&tmp_tar);
            return Err(PanelError::BadGateway(format!(
                "Không thể tải gói Myphpadmin {OFFICIAL_PMA_VERSION} từ files.phpmyadmin.net ({last_curl_err}). Vui lòng kiểm tra kết nối mạng của VPS."
            )));
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

    // 3. Prepare tmp dir, config.inc.php, router, directory permissions, SSO account, and systemd unit on 127.0.0.1:18889.
    std::fs::create_dir_all(OFFICIAL_PMA_TMP_DIR)?;
    set_dir_mode("/var/lib/phpmyadmin", 0o755);
    set_dir_mode(OFFICIAL_PMA_TMP_DIR, 0o770);
    set_dir_mode(OFFICIAL_PMA_DIR, 0o755);

    let _ = crate::modules::databases::ensure_myphpadmin_sso_mysql_account(executor).await;
    if executor.is_binary_available(AllowedBinary::Chown)
        && Path::new(crate::modules::databases::MYPHPADMIN_SSO_PASS_FILE).is_file()
    {
        let _ = executor
            .run(
                AllowedBinary::Chown,
                &[
                    "root:www-data",
                    crate::modules::databases::MYPHPADMIN_SSO_PASS_FILE,
                ],
                Duration::from_secs(5),
            )
            .await;
        set_dir_mode(crate::modules::databases::MYPHPADMIN_SSO_PASS_FILE, 0o640);
    }

    let cfg_inc = pma_dir.join("config.inc.php");
    if !cfg_inc.is_file() {
        let secret = generate_blowfish_secret();
        atomic_write_file(&cfg_inc, render_official_pma_config_inc(&secret).as_bytes(), 0o640)?;
    } else {
        ensure_config_allows_sameorigin_frame(&cfg_inc);
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
                &[
                    "-R",
                    "www-data:www-data",
                    OFFICIAL_PMA_DIR,
                    OFFICIAL_PMA_TMP_DIR,
                ],
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
            .run(
                AllowedBinary::Systemctl,
                &["daemon-reload"],
                Duration::from_secs(15),
            )
            .await;
        let _ = executor
            .run(
                AllowedBinary::Systemctl,
                &["enable", OFFICIAL_PMA_UNIT_NAME],
                Duration::from_secs(15),
            )
            .await;
        let restart_out = executor
            .run(
                AllowedBinary::Systemctl,
                &["restart", OFFICIAL_PMA_UNIT_NAME],
                Duration::from_secs(20),
            )
            .await?;
        if !restart_out.success {
            return Err(PanelError::BadGateway(format!(
                "Không thể khởi chạy {OFFICIAL_PMA_UNIT_NAME}: {}",
                restart_out.stderr.trim()
            )));
        }
    }

    for _ in 0..25 {
        if is_official_pma_port_open().await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    let status = get_official_phpmyadmin_status().await;
    if !status.running && executor.is_binary_available(AllowedBinary::Journalctl) {
        let detail = executor
            .run(
                AllowedBinary::Journalctl,
                &["-u", OFFICIAL_PMA_UNIT_NAME, "-n", "12", "--no-pager"],
                Duration::from_secs(5),
            )
            .await
            .map(|o| o.stdout)
            .unwrap_or_default();
        return Err(PanelError::BadGateway(format!(
            "Dịch vụ {} ({}) đã ghi cấu hình nhưng chưa mở cổng 127.0.0.1:{OFFICIAL_PMA_PORT}. Nhật ký: {}",
            status.display_name,
            OFFICIAL_PMA_UNIT_NAME,
            detail.trim()
        )));
    }

    Ok(status)
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

fn render_official_pma_launcher_html(version: &str) -> String {
    let display = format!("Myphpadmin {version}");
    let template = r##"<!DOCTYPE html>
<html lang="vi" class="dark">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>__PMA_NAME__ &amp; FPSql &amp; Database — FerrousPanel</title>
  <style>
    * { box-sizing: border-box; }
    body { margin:0; font-family:system-ui,-apple-system,sans-serif; background:#090d16; color:#e2e8f0; display:flex; align-items:center; justify-content:center; min-height:100vh; padding:20px; }
    .card { max-width:580px; width:100%; background:#111827; border:1px solid #1e293b; border-radius:14px; padding:26px; box-shadow:0 24px 48px rgba(0,0,0,.45); }
    h1 { margin:0 0 8px; font-size:20px; }
    p { margin:0 0 16px; color:#94a3b8; font-size:14px; line-height:1.55; }
    .actions { display:flex; gap:10px; flex-wrap:wrap; margin-top:18px; }
    .btn { display:inline-flex; align-items:center; justify-content:center; padding:10px 16px; border-radius:9px; font-weight:600; font-size:13px; text-decoration:none; cursor:pointer; border:1px solid #334155; background:#1e293b; color:#e2e8f0; }
    .btn-primary { background:#f97316; border-color:#ea580c; color:#fff; }
    .btn-blue { background:rgba(59,130,246,.18); border-color:rgba(59,130,246,.45); color:#93c5fd; }
    .code { font-family:ui-monospace,monospace; background:#020617; border:1px solid #1e293b; border-radius:8px; padding:10px 12px; font-size:12px; color:#cbd5e1; overflow-x:auto; }
    .auth-box { margin-top:14px; padding:14px; border-radius:10px; background:#0b1220; border:1px solid #1e293b; display:none; }
    .auth-box input { width:100%; margin:6px 0 10px; padding:9px 11px; border-radius:8px; border:1px solid #334155; background:#020617; color:#e2e8f0; font-size:13px; }
    .auth-box label { font-size:12px; color:#94a3b8; display:block; }
    #msg { margin-top:14px; font-size:13px; display:none; padding:10px 12px; border-radius:8px; white-space:pre-wrap; word-break:break-word; }
  </style>
</head>
<body>
  <div class="card">
    <h1>FPSql &amp; Database (Dual-Mode SQL)</h1>
    <p>FerrousPanel hỗ trợ song song cả hai trình quản trị CSDL trên mọi tên miền và IP:</p>
    <ul style="color:#cbd5e1;font-size:13.5px;line-height:1.7;padding-left:18px;margin:0 0 16px;">
      <li><strong>__PMA_NAME__</strong> tại <code>/phpmyadmin/</code> (tải trực tiếp từ <code>phpmyadmin.net</code>)</li>
      <li><strong>FPSql &amp; Database (Pure-Rust SQL Studio)</strong> tại <code>/phpadmin/</code> (100% Pure Rust, siêu tốc)</li>
    </ul>
    <p style="margin-bottom:8px;">Hiện tại dịch vụ <code>__PMA_NAME__</code> chưa được kích hoạt trên máy chủ này. Bạn có thể bấm nút cài tự động bên dưới hoặc chạy lệnh cập nhật trên VPS:</p>
    <div class="code">sudo fp update</div>
    <div id="msg"></div>
    <form id="inlineAuthBox" class="auth-box">
      <div style="font-weight:600;font-size:13px;margin-bottom:8px;color:#fb923c;">Đăng nhập Quản trị Panel để kích hoạt __PMA_NAME__</div>
      <label for="fpUser">Tài khoản Panel</label>
      <input id="fpUser" type="text" value="admin_ferrous" required autocomplete="username" />
      <label for="fpPass">Mật khẩu Panel</label>
      <input id="fpPass" type="password" required autocomplete="current-password" />
      <button type="submit" class="btn btn-primary" id="btnAuthAndInstall">Đăng nhập &amp; Cài __PMA_NAME__ ngay</button>
    </form>
    <div class="actions">
      <button type="button" class="btn btn-primary" id="btnInstallOfficial">Tải &amp; Kích hoạt __PMA_NAME__ ngay</button>
      <a href="/phpadmin/" class="btn btn-blue">Mở FPSql &amp; Database (/phpadmin/) →</a>
    </div>
  </div>
  <script>
    (function () {
      const apiBase = window.location.pathname.startsWith('/phpmyadmin')
        ? '/phpmyadmin/api'
        : '/api';
      const btn = document.getElementById('btnInstallOfficial');
      const msg = document.getElementById('msg');
      const authBox = document.getElementById('inlineAuthBox');

      function showMsg(text, kind) {
        msg.style.display = 'block';
        if (kind === 'ok') {
          msg.style.background = 'rgba(16,185,129,.16)';
          msg.style.color = '#6ee7b7';
        } else if (kind === 'err') {
          msg.style.background = 'rgba(239,68,68,.16)';
          msg.style.color = '#fca5a5';
        } else {
          msg.style.background = 'rgba(59,130,246,.14)';
          msg.style.color = '#93c5fd';
        }
        msg.textContent = text;
      }

      async function runInstall(explicitToken) {
        btn.disabled = true;
        btn.textContent = 'Đang tải & khởi chạy __PMA_NAME__…';
        showMsg('Đang cài đặt __PMA_NAME__ từ files.phpmyadmin.net (khoảng 10–30 giây)…', 'info');
        try {
          const tok = explicitToken || sessionStorage.getItem('fp_pma_token') || sessionStorage.getItem('fp_token') || localStorage.getItem('fp_token') || '';
          const headers = { 'Content-Type': 'application/json', 'X-Requested-With': 'FerrousPanel-UI' };
          if (tok) headers['Authorization'] = 'Bearer ' + tok;
          const resp = await fetch(apiBase + '/databases/phpmyadmin/install', {
            method: 'POST',
            credentials: 'same-origin',
            headers,
            body: '{}'
          });
          const data = await resp.json().catch(() => ({}));
          if (resp.status === 401 || resp.status === 403) {
            btn.disabled = false;
            btn.textContent = 'Tải & Kích hoạt __PMA_NAME__ ngay';
            authBox.style.display = 'block';
            showMsg('Vui lòng đăng nhập tài khoản quản trị Panel ở khung bên dưới để cho phép cài đặt __PMA_NAME__.', 'info');
            return;
          }
          if (!resp.ok) {
            throw new Error(data.error || ('HTTP ' + resp.status + ' — Cài đặt thất bại.'));
          }
          showMsg('Đã cài đặt & khởi chạy ' + (data.displayName || '__PMA_NAME__') + '! Đang mở trang…', 'ok');
          setTimeout(() => window.location.reload(), 600);
        } catch (e) {
          btn.disabled = false;
          btn.textContent = 'Thử kích hoạt lại';
          showMsg(e.message || 'Lỗi không cài được __PMA_NAME__.', 'err');
        }
      }

      btn.addEventListener('click', () => runInstall(''));

      authBox.addEventListener('submit', async function (ev) {
        ev.preventDefault();
        const sub = document.getElementById('btnAuthAndInstall');
        sub.disabled = true;
        try {
          const r = await fetch(apiBase + '/auth/login', {
            method: 'POST',
            credentials: 'same-origin',
            headers: { 'Content-Type': 'application/json', 'X-Requested-With': 'FerrousPanel-UI' },
            body: JSON.stringify({
              username: document.getElementById('fpUser').value.trim(),
              password: document.getElementById('fpPass').value,
              rememberMe: false
            })
          });
          const d = await r.json().catch(() => ({}));
          if (!r.ok || !d.token) {
            throw new Error(d.error || 'Sai tài khoản hoặc mật khẩu quản trị Panel.');
          }
          sessionStorage.setItem('fp_pma_token', d.token);
          authBox.style.display = 'none';
          await runInstall(d.token);
        } catch (e) {
          showMsg(e.message || 'Đăng nhập thất bại.', 'err');
        } finally {
          sub.disabled = false;
        }
      });
    })();
  </script>
</body>
</html>"##;
    template.replace("__PMA_NAME__", &display)
}

fn extract_query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=')?;
        if k == key && !v.trim().is_empty() {
            return Some(v.trim().to_string());
        }
    }
    None
}

fn strip_fp_sso_from_query(query: &str) -> String {
    query
        .split('&')
        .filter(|pair| !pair.is_empty() && !pair.starts_with("fp_sso="))
        .collect::<Vec<_>>()
        .join("&")
}

fn verify_panel_session_for_pma(req: &HttpRequest, state: &AppState) -> (bool, Option<String>) {
    let query_tok = req
        .uri()
        .query()
        .and_then(|q| extract_query_param(q, "fp_sso"));
    let candidates = [
        req.cookie(crate::auth::middleware::SESSION_COOKIE_NAME)
            .map(|c| c.value().trim().to_string()),
        req.cookie("fp_pma_sso")
            .map(|c| c.value().trim().to_string()),
        query_tok.clone(),
        req.headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().strip_prefix("Bearer "))
            .map(|s| s.trim().to_string()),
        req.headers()
            .get("x-fp-session")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_string()),
    ];

    for cand in candidates.into_iter().flatten() {
        if !cand.is_empty()
            && state
                .auth_store
                .verify_session(&cand)
                .ok()
                .flatten()
                .is_some()
        {
            return (true, Some(cand));
        }
    }

    let client_ip = crate::auth::rate_limit::extract_client_ip(req, &state.config);
    if state.auth_store.has_active_session_for_ip(&client_ip) {
        return (true, None);
    }
    (false, None)
}

async fn resolve_myphpadmin_sso_password(state: &AppState) -> Option<String> {
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    static CACHE: OnceLock<Mutex<Option<(Instant, String)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some((ts, ref pw)) = *guard {
            if ts.elapsed() < Duration::from_secs(45) && !pw.is_empty() {
                return Some(pw.clone());
            }
        }
    }

    let ensured = crate::modules::databases::ensure_myphpadmin_sso_mysql_account(
        state.webserver.executor().as_ref(),
    )
    .await;
    if let Some((_, pw)) = ensured {
        if state
            .webserver
            .executor()
            .is_binary_available(AllowedBinary::Chown)
            && Path::new(crate::modules::databases::MYPHPADMIN_SSO_PASS_FILE).is_file()
        {
            let _ = state
                .webserver
                .executor()
                .run(
                    AllowedBinary::Chown,
                    &[
                        "root:www-data",
                        crate::modules::databases::MYPHPADMIN_SSO_PASS_FILE,
                    ],
                    Duration::from_secs(5),
                )
                .await;
            set_dir_mode(crate::modules::databases::MYPHPADMIN_SSO_PASS_FILE, 0o640);
        }
        if let Ok(mut guard) = cache.lock() {
            *guard = Some((Instant::now(), pw.clone()));
        }
        return Some(pw);
    }

    std::fs::read_to_string(crate::modules::databases::MYPHPADMIN_SSO_PASS_FILE)
        .ok()
        .or_else(|| {
            std::fs::read_to_string(crate::modules::databases::MYPHPADMIN_SSO_PASS_FILE_LEGACY).ok()
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Reverse-proxies `/phpmyadmin/*` (all HTTP methods, cookies, sessions, SQL uploads/exports)
/// to the Myphpadmin worker on `127.0.0.1:18889`, auto-healing & auto-starting the systemd unit
/// if installed on disk, or serving the Dual-Mode 1-Click Launcher if not yet installed.
pub async fn serve_official_phpmyadmin(
    req: HttpRequest,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> HttpResponse {
    if state.config.is_production() {
        let cfg_inc = Path::new(OFFICIAL_PMA_DIR).join("config.inc.php");
        if cfg_inc.is_file() {
            ensure_config_allows_sameorigin_frame(&cfg_inc);
        }
    }

    let mut stream_opt = TcpStream::connect(("127.0.0.1", OFFICIAL_PMA_PORT)).await.ok();

    if stream_opt.is_none()
        && state.config.is_production()
        && Path::new(OFFICIAL_PMA_DIR).join("index.php").is_file()
        && state
            .webserver
            .executor()
            .is_binary_available(AllowedBinary::Systemctl)
    {
        // Auto-migrate stale unit files that used privileged port 889 (which fails under User=www-data).
        let unit_needs_rewrite = match std::fs::read_to_string(OFFICIAL_PMA_UNIT_PATH) {
            Ok(txt) => !txt.contains(&format!("127.0.0.1:{OFFICIAL_PMA_PORT}")),
            Err(_) => true,
        };
        if unit_needs_rewrite {
            if let Some((_, php_bin, _)) = find_php_cli_binary() {
                set_dir_mode("/www", 0o755);
                set_dir_mode("/www/server", 0o755);
                let _ = std::fs::create_dir_all(OFFICIAL_PMA_TMP_DIR);
                set_dir_mode("/var/lib/phpmyadmin", 0o755);
                set_dir_mode(OFFICIAL_PMA_TMP_DIR, 0o770);
                ensure_config_allows_sameorigin_frame(&Path::new(OFFICIAL_PMA_DIR).join("config.inc.php"));
                let _ = atomic_write_file(
                    Path::new(OFFICIAL_PMA_ROUTER),
                    render_official_pma_router_php().as_bytes(),
                    0o644,
                );
                let _ = atomic_write_file(
                    Path::new(OFFICIAL_PMA_UNIT_PATH),
                    render_official_pma_systemd_unit(php_bin).as_bytes(),
                    0o644,
                );
                let _ = state
                    .webserver
                    .executor()
                    .run(AllowedBinary::Systemctl, &["daemon-reload"], Duration::from_secs(8))
                    .await;
            }
        }
        let _ = state
            .webserver
            .executor()
            .run(
                AllowedBinary::Systemctl,
                &["restart", OFFICIAL_PMA_UNIT_NAME],
                Duration::from_secs(10),
            )
            .await;
        for _ in 0..12 {
            tokio::time::sleep(Duration::from_millis(160)).await;
            if let Ok(s) = TcpStream::connect(("127.0.0.1", OFFICIAL_PMA_PORT)).await {
                stream_opt = Some(s);
                break;
            }
        }
    }

    let Some(mut stream) = stream_opt else {
        let ver = detect_installed_pma_version();
        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .insert_header(("Cache-Control", "no-store"))
            .body(render_official_pma_launcher_html(&ver));
    };

    let method = req.method().as_str();
    let path = req.uri().path();
    let (sso_authorized, verified_token) = verify_panel_session_for_pma(&req, &state);
    let mut sso_password_hdr: Option<String> = None;

    if sso_authorized && state.config.is_production() {
        sso_password_hdr = resolve_myphpadmin_sso_password(&state).await;
        if path.ends_with('/') || path.ends_with(".php") {
            crate::modules::databases::reconcile_live_mariadb_schemas_into_store(
                &state.modules,
                state.webserver.executor().as_ref(),
            )
            .await;
        }
    }

    let cleaned_query = req
        .uri()
        .query()
        .map(strip_fp_sso_from_query)
        .unwrap_or_default();
    let target_uri = if !cleaned_query.is_empty() {
        format!("{path}?{cleaned_query}")
    } else {
        path.to_string()
    };
    let (host, scheme) = {
        let conn_info = req.connection_info();
        let fwd_proto = req
            .headers()
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        (
            conn_info.host().to_string(),
            fwd_proto.unwrap_or_else(|| conn_info.scheme().to_string()),
        )
    };

    let mut raw_req = Vec::with_capacity(512 + body.len());
    raw_req.extend_from_slice(format!("{method} {target_uri} HTTP/1.1\r\n").as_bytes());
    raw_req.extend_from_slice(format!("Host: {host}\r\n").as_bytes());
    raw_req.extend_from_slice(b"Connection: close\r\n");
    raw_req.extend_from_slice(format!("X-Forwarded-Host: {host}\r\n").as_bytes());
    raw_req.extend_from_slice(format!("X-Forwarded-Proto: {scheme}\r\n").as_bytes());
    if sso_authorized {
        raw_req.extend_from_slice(b"X-Ferrous-PMA-Auth: 1\r\n");
        if let Some(ref pw) = sso_password_hdr {
            if !pw.contains('\r') && !pw.contains('\n') {
                raw_req.extend_from_slice(format!("X-Ferrous-PMA-Pass: {pw}\r\n").as_bytes());
            }
        }
    }

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
                | "x-ferrous-pma-auth"
                | "x-ferrous-pma-pass"
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
            .body("Failed to forward request to Myphpadmin worker");
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
            .body("Myphpadmin worker timed out");
    }

    if sso_authorized && state.config.is_production() && method != "GET" && method != "HEAD" {
        crate::modules::databases::reconcile_live_mariadb_schemas_into_store(
            &state.modules,
            state.webserver.executor().as_ref(),
        )
        .await;
    }

    let Some(hdr_end) = resp_bytes.windows(4).position(|w| w == b"\r\n\r\n") else {
        return HttpResponse::BadGateway()
            .content_type("text/plain; charset=utf-8")
            .body("Invalid upstream response from Myphpadmin worker");
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
    if let Some(tok) = verified_token {
        if !tok.contains(';') {
            if req.cookie(crate::auth::middleware::SESSION_COOKIE_NAME).is_none() {
                builder.append_header((
                    "Set-Cookie",
                    format!(
                        "{}={tok}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400",
                        crate::auth::middleware::SESSION_COOKIE_NAME
                    ),
                ));
            }
            if req.cookie("fp_pma_sso").is_none() {
                builder.append_header((
                    "Set-Cookie",
                    format!("fp_pma_sso={tok}; Path=/phpmyadmin; HttpOnly; SameSite=Lax; Max-Age=86400"),
                ));
            }
        }
    }
    let mut is_chunked = false;
    let mut is_html = false;
    let internal_origin = format!("http://127.0.0.1:{OFFICIAL_PMA_PORT}");

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
        if lower == "x-frame-options" {
            builder.insert_header(("X-Frame-Options", "SAMEORIGIN"));
            continue;
        }
        if lower == "content-security-policy" {
            let rewritten = val_trim.replace("frame-ancestors 'none'", "frame-ancestors 'self'");
            builder.append_header((key_trim.to_string(), rewritten));
            continue;
        }
        if lower == "location" {
            let stripped = val_trim
                .strip_prefix(&internal_origin)
                .unwrap_or(val_trim);
            let rewritten = if (stripped.starts_with("http://") || stripped.starts_with("https://"))
                && stripped.contains("/phpmyadmin")
            {
                stripped
                    .find("/phpmyadmin")
                    .map(|idx| &stripped[idx..])
                    .unwrap_or(stripped)
            } else {
                stripped
            };
            builder.append_header((key_trim.to_string(), rewritten.to_string()));
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

    // Inject a subtle quick-switch pill in HTML pages so users can switch to Panel's custom FPSql & Database UI (/phpadmin/) anytime.
    if is_html && method != "HEAD" {
        let badge = b"<a href=\"/phpadmin/\" style=\"position:fixed;bottom:12px;right:12px;z-index:99999;background:#0f172a;color:#fb923c;border:1px solid #f97316;padding:6px 12px;border-radius:999px;font:600 12px system-ui,sans-serif;text-decoration:none;box-shadow:0 4px 14px rgba(0,0,0,.35)\">\xE2\x87\x84 FPSql &amp; Database (/phpadmin/)</a></body>";
        if let Some(pos) = decoded_body
            .windows(7)
            .rposition(|w| w.eq_ignore_ascii_case(b"</body>"))
        {
            decoded_body.splice(pos..pos + 7, badge.iter().copied());
        }
    }

    builder.body(decoded_body)
}

// ============================================================================
// Official pgAdmin 4 Clone, Auto-Sync & Reverse Proxy (/pgadmin/*)
// ============================================================================

pub const OFFICIAL_PGADMIN_REPO_DIR: &str = "/www/server/pgadmin";
pub const OFFICIAL_PGADMIN_STATE_DIR: &str = "/var/lib/pgadmin";
pub const OFFICIAL_PGADMIN_VENV_DIR: &str = "/var/lib/pgadmin/venv";
pub const OFFICIAL_PGADMIN_UNIT_NAME: &str = "ferrous-pgadmin.service";
pub const OFFICIAL_PGADMIN_UNIT_PATH: &str = "/etc/systemd/system/ferrous-pgadmin.service";
pub const OFFICIAL_PGADMIN_PORT: u16 = 18890;
/// Verified prebuilt wheel release containing all 49 production Webpack bundles (`static/js/generated/*`).
/// Note: upstream `pgadmin4==9.19` on PyPI omitted `static/js/generated/*`, causing `#pg-spinner` to hang.
pub const OFFICIAL_PGADMIN_DEFAULT_VERSION: &str = "9.18";
pub const OFFICIAL_PGADMIN_WHEEL_SPEC: &str = "pgadmin4==9.18";
pub const OFFICIAL_PGADMIN_CONFIG_MARKER: &str = "FerrousPanel-PgAdmin-Config-v3";

static PGADMIN_REPAIR_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> =
    std::sync::OnceLock::new();
static LAST_PGADMIN_ACCOUNT_SYNC_SECS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
static LAST_PGADMIN_DB_RECONCILE_SECS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

fn pgadmin_repair_lock() -> &'static tokio::sync::Mutex<()> {
    PGADMIN_REPAIR_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

fn current_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialPgAdminStatus {
    pub installed: bool,
    pub running: bool,
    pub repo_cloned: bool,
    pub version: String,
    pub display_name: String,
    pub install_dir: String,
    pub pgadmin_url: String,
    pub custom_panel_url: String,
}

/// Verifies that `/var/lib/pgadmin/venv` contains the compiled frontend Webpack bundles
/// (`app.bundle.js` and `vendor.main.js`) required to dismiss `#pg-spinner` (`Loading pgAdmin 4...`).
pub fn has_valid_pgadmin_frontend_bundles() -> bool {
    let lib_dir = Path::new(OFFICIAL_PGADMIN_VENV_DIR).join("lib");
    let Ok(entries) = std::fs::read_dir(&lib_dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("python") {
            continue;
        }
        let gen_dir = entry
            .path()
            .join("site-packages/pgadmin4/pgadmin/static/js/generated");
        if gen_dir.join("app.bundle.js").is_file() && gen_dir.join("vendor.main.js").is_file() {
            return true;
        }
    }
    false
}

/// Returns `true` when `/var/lib/pgadmin` has a healthy virtualenv with frontend bundles AND
/// up-to-date `v3` `config_local.py`, `wsgi_pgadmin.py`, and `ferrous-pgadmin.service` files.
pub fn is_pgadmin_installation_healthy() -> bool {
    if !Path::new("/var/lib/pgadmin/venv/bin/gunicorn").is_file()
        || !has_valid_pgadmin_frontend_bundles()
    {
        return false;
    }
    for path in [
        "/var/lib/pgadmin/config_local.py",
        "/var/lib/pgadmin/wsgi_pgadmin.py",
        OFFICIAL_PGADMIN_UNIT_PATH,
    ] {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return false;
        };
        if !raw.contains(OFFICIAL_PGADMIN_CONFIG_MARKER) {
            return false;
        }
    }
    true
}

fn detect_installed_pgadmin_version() -> String {
    let ver_file = Path::new(OFFICIAL_PGADMIN_STATE_DIR).join("version.txt");
    if let Ok(raw) = std::fs::read_to_string(&ver_file) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    OFFICIAL_PGADMIN_DEFAULT_VERSION.to_string()
}

async fn is_official_pgadmin_port_open() -> bool {
    tokio::time::timeout(
        Duration::from_millis(250),
        TcpStream::connect(("127.0.0.1", OFFICIAL_PGADMIN_PORT)),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
}

pub async fn get_official_pgadmin_status() -> OfficialPgAdminStatus {
    let venv_ready = Path::new("/var/lib/pgadmin/venv/bin/gunicorn").is_file()
        && has_valid_pgadmin_frontend_bundles();
    let repo_cloned = Path::new(OFFICIAL_PGADMIN_REPO_DIR).join(".git").is_dir()
        || Path::new(OFFICIAL_PGADMIN_REPO_DIR).join("README.md").is_file();
    let installed = venv_ready;
    let running = if installed {
        is_official_pgadmin_port_open().await
    } else {
        false
    };
    let version = detect_installed_pgadmin_version();
    OfficialPgAdminStatus {
        installed,
        running,
        repo_cloned,
        display_name: format!("pgAdmin 4 ({version})"),
        version,
        install_dir: OFFICIAL_PGADMIN_REPO_DIR.to_string(),
        pgadmin_url: "/pgadmin/".to_string(),
        custom_panel_url: "/phpadmin/".to_string(),
    }
}

fn render_pgadmin_config_local() -> &'static str {
    r#"# FerrousPanel-PgAdmin-Config-v3 — Single-User SSO Mode behind Panel Session Guard
SERVER_MODE = False
MASTER_PASSWORD_REQUIRED = False
UPGRADE_CHECK_ENABLED = False
DEFAULT_SERVER = '127.0.0.1'
DEFAULT_SERVER_PORT = 18890
DATA_DIR = '/var/lib/pgadmin/data'
LOG_FILE = '/var/lib/pgadmin/data/pgadmin4.log'
SQLITE_PATH = '/var/lib/pgadmin/data/pgadmin4.db'
SESSION_DB_PATH = '/var/lib/pgadmin/data/sessions'
STORAGE_DIR = '/var/lib/pgadmin/data/storage'
COOKIE_DEFAULT_PATH = '/pgadmin'
SESSION_COOKIE_PATH = '/pgadmin'
ENHANCED_COOKIE_PROTECTION = False
WTF_CSRF_SSL_STRICT = False
PROXY_X_FOR_COUNT = 1
PROXY_X_PROTO_COUNT = 1
PROXY_X_HOST_COUNT = 1
PROXY_X_PORT_COUNT = 0
PROXY_X_PREFIX_COUNT = 0
COMPRESS_MIMETYPES = []
X_FRAME_OPTIONS = 'SAMEORIGIN'
CROSS_ORIGIN_OPENER_POLICY = 'same-origin-allow-popups'
CONTENT_SECURITY_POLICY = (
    "default-src 'self' https: http: wss: ws: data: blob:;"
    " script-src 'self' 'unsafe-inline' 'unsafe-eval' https: http: blob:;"
    " style-src 'self' 'unsafe-inline' https: http:;"
    " img-src 'self' data: blob: https: http:;"
    " font-src 'self' data: https: http:;"
    " connect-src 'self' https: http: wss: ws:;"
    " frame-ancestors 'self';"
    " object-src 'none';"
    " base-uri 'self';"
)
"#
}

fn render_pgadmin_servers_json() -> String {
    format!(
        r#"{{
  "Servers": {{
    "1": {{
      "Name": "FPSql & Database — PostgreSQL 17 (Local)",
      "Group": "Servers",
      "Host": "127.0.0.1",
      "Port": 5432,
      "MaintenanceDB": "postgres",
      "Username": "{user}",
      "PassFile": "{pgpass}",
      "SSLMode": "prefer"
    }}
  }}
}}
"#,
        user = crate::modules::databases::PGADMIN_SSO_USER,
        pgpass = crate::modules::databases::PGADMIN_PGPASS_FILE,
    )
}

fn render_pgadmin_wsgi_py() -> &'static str {
    r#"# FerrousPanel-PgAdmin-Config-v3
import os
import sys

os.environ['PGADMIN_SERVER_MODE'] = 'OFF'
os.environ['SCRIPT_NAME'] = '/pgadmin'
sys.path.insert(0, '/var/lib/pgadmin')

from pgadmin4.pgAdmin4 import app as _pgadmin_app

def application(environ, start_response):
    environ['SCRIPT_NAME'] = '/pgadmin'
    path_info = environ.get('PATH_INFO', '') or '/'
    if path_info == '/pgadmin':
        environ['PATH_INFO'] = '/'
    elif path_info.startswith('/pgadmin/'):
        stripped = path_info[len('/pgadmin'):]
        environ['PATH_INFO'] = stripped if stripped else '/'
    elif not path_info.startswith('/'):
        environ['PATH_INFO'] = '/' + path_info
    return _pgadmin_app(environ, start_response)
"#
}

fn render_pgadmin_bootstrap_py() -> &'static str {
    r#"# FerrousPanel-PgAdmin-Config-v3
import importlib.util
import os
import shutil
import sqlite3
import subprocess
import sys

os.environ['PGADMIN_SERVER_MODE'] = 'OFF'
os.environ['SCRIPT_NAME'] = '/pgadmin'
sys.path.insert(0, '/var/lib/pgadmin')

spec = importlib.util.find_spec('pgadmin4.pgAdmin4')
if spec is None or not spec.origin:
    raise RuntimeError('pgadmin4.pgAdmin4 module not found in /var/lib/pgadmin/venv')
pkg_dir = os.path.dirname(os.path.abspath(spec.origin))

src_cfg = '/var/lib/pgadmin/config_local.py'
dst_cfg = os.path.join(pkg_dir, 'config_local.py')
if os.path.isfile(src_cfg):
    shutil.copyfile(src_cfg, dst_cfg)

sqlite_db = '/var/lib/pgadmin/data/pgadmin4.db'
sessions_dir = '/var/lib/pgadmin/data/sessions'
if os.path.isfile(sqlite_db):
    try:
        if pkg_dir not in sys.path:
            sys.path.insert(0, pkg_dir)
        from pgadmin.model import SCHEMA_VERSION
        conn = sqlite3.connect(sqlite_db)
        cur = conn.execute("SELECT value FROM version WHERE name = 'ConfigDB'")
        row = cur.fetchone()
        conn.close()
        if row and int(row[0]) > int(SCHEMA_VERSION):
            os.remove(sqlite_db)
            shutil.rmtree(sessions_dir, ignore_errors=True)
            os.makedirs(sessions_dir, exist_ok=True)
    except Exception:
        try:
            os.remove(sqlite_db)
        except OSError:
            pass
        shutil.rmtree(sessions_dir, ignore_errors=True)
        os.makedirs(sessions_dir, exist_ok=True)

import pgadmin4.pgAdmin4 as _pga_entry  # noqa: F401 — initializes SQLite schema when missing

ver = ''
try:
    from version import APP_VERSION
    ver = str(APP_VERSION).strip()
except Exception:
    pass
if not ver:
    try:
        from importlib.metadata import version as meta_ver
        ver = meta_ver('pgadmin4')
    except Exception:
        ver = '9.18'
with open('/var/lib/pgadmin/version.txt', 'w', encoding='utf-8') as vf:
    vf.write(str(ver).strip() + '\n')

has_local_server = False
if os.path.isfile(sqlite_db):
    try:
        conn = sqlite3.connect(sqlite_db)
        cur = conn.execute("SELECT COUNT(*) FROM server WHERE host = '127.0.0.1' AND port = 5432")
        row = cur.fetchone()
        if row and int(row[0]) > 0:
            has_local_server = True
            conn.execute(
                "UPDATE server SET username = ?, passfile = ? WHERE host = '127.0.0.1' AND port = 5432",
                ('fp_pgadmin', '/var/lib/pgadmin/pgpass'),
            )
            conn.commit()
        conn.close()
    except Exception:
        has_local_server = False

setup_py = os.path.join(pkg_dir, 'setup.py')
servers_json = '/var/lib/pgadmin/servers.json'
if not has_local_server and os.path.isfile(setup_py) and os.path.isfile(servers_json):
    env = os.environ.copy()
    env['PGADMIN_SERVER_MODE'] = 'OFF'
    env['SCRIPT_NAME'] = '/pgadmin'
    env['PYTHONPATH'] = '/var/lib/pgadmin'
    subprocess.run(
        [sys.executable, setup_py, 'load-servers', servers_json],
        env=env,
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
"#
}

fn render_pgadmin_systemd_unit() -> String {
    format!(
        "# {OFFICIAL_PGADMIN_CONFIG_MARKER}\n\
[Unit]\n\
Description=FerrousPanel pgAdmin 4 Worker (127.0.0.1:{OFFICIAL_PGADMIN_PORT})\n\
After=network.target postgresql.service\n\
\n\
[Service]\n\
Type=simple\n\
User=www-data\n\
Group=www-data\n\
WorkingDirectory={OFFICIAL_PGADMIN_STATE_DIR}\n\
Environment=HOME={OFFICIAL_PGADMIN_STATE_DIR}\n\
Environment=PYTHONPATH={OFFICIAL_PGADMIN_STATE_DIR}\n\
Environment=PGADMIN_SERVER_MODE=OFF\n\
Environment=SCRIPT_NAME=/pgadmin\n\
Environment=PGPASSFILE={pgpass}\n\
ExecStart=/var/lib/pgadmin/venv/bin/gunicorn --bind 127.0.0.1:{OFFICIAL_PGADMIN_PORT} --workers 1 --threads 16 --timeout 180 --chdir {OFFICIAL_PGADMIN_STATE_DIR} wsgi_pgadmin:application\n\
Restart=always\n\
RestartSec=3\n\
NoNewPrivileges=true\n\
PrivateTmp=true\n\
\n\
[Install]\n\
WantedBy=multi-user.target\n",
        pgpass = crate::modules::databases::PGADMIN_PGPASS_FILE,
    )
}

/// Clones `https://github.com/pgadmin-org/pgadmin4.git` into `/www/server/pgadmin`, installs the
/// verified `pgadmin4==9.18` runtime (with complete precompiled frontend bundles) into `/var/lib/pgadmin/venv`,
/// auto-registers the local PostgreSQL cluster (`FPSql & Database — PostgreSQL 17`), and starts
/// `ferrous-pgadmin.service` on `127.0.0.1:18890`.
pub async fn install_or_repair_official_pgadmin(
    executor: &dyn CommandExecutor,
) -> Result<OfficialPgAdminStatus, PanelError> {
    let _repair_guard = pgadmin_repair_lock().lock().await;
    let apt_timeout = Duration::from_secs(300);
    let pip_timeout = Duration::from_secs(600);
    let noninteractive_env = [("DEBIAN_FRONTEND", "noninteractive")];

    if executor.is_binary_available(AllowedBinary::AptGet) {
        let need_apt = !executor.is_binary_available(AllowedBinary::Psql)
            || !executor.is_binary_available(AllowedBinary::Python3)
            || !Path::new("/usr/lib/python3/dist-packages/venv").exists()
            || !Path::new("/var/lib/pgadmin/venv/bin/python3").is_file();
        if need_apt {
            let _ = executor
                .run_with_env(
                    AllowedBinary::AptGet,
                    &["update", "-qq"],
                    &noninteractive_env,
                    None,
                    apt_timeout,
                )
                .await;
            let _ = executor
                .run_with_env(
                    AllowedBinary::AptGet,
                    &[
                        "install",
                        "-y",
                        "-qq",
                        "-o",
                        "Dpkg::Options::=--force-confdef",
                        "-o",
                        "Dpkg::Options::=--force-confold",
                        "postgresql",
                        "postgresql-client",
                        "python3",
                        "python3-venv",
                        "python3-pip",
                        "libpq-dev",
                        "git",
                    ],
                    &noninteractive_env,
                    None,
                    apt_timeout,
                )
                .await;
        }
    }

    if executor.is_binary_available(AllowedBinary::Systemctl) {
        let _ = executor
            .run(
                AllowedBinary::Systemctl,
                &["enable", "--now", "postgresql"],
                Duration::from_secs(20),
            )
            .await;
    }

    if !executor.is_binary_available(AllowedBinary::Python3) {
        return Err(PanelError::Conflict(
            "Chưa tìm thấy python3 trên máy chủ — hãy chạy: sudo apt install -y python3 python3-venv postgresql".to_string(),
        ));
    }

    // 1. Clone upstream pgAdmin 4 repository into /www/server/pgadmin (shallow clone).
    std::fs::create_dir_all("/www/server")?;
    set_dir_mode("/www", 0o755);
    set_dir_mode("/www/server", 0o755);
    let repo_dir = Path::new(OFFICIAL_PGADMIN_REPO_DIR);
    if !repo_dir.join(".git").is_dir() && executor.is_binary_available(AllowedBinary::Git) {
        let _ = std::fs::remove_dir_all(repo_dir);
        let _ = executor
            .run(
                AllowedBinary::Git,
                &[
                    "clone",
                    "--depth",
                    "1",
                    "https://github.com/pgadmin-org/pgadmin4.git",
                    OFFICIAL_PGADMIN_REPO_DIR,
                ],
                Duration::from_secs(180),
            )
            .await;
    }

    // 2. Create isolated virtualenv /var/lib/pgadmin/venv and install verified pgadmin4==9.18 + gunicorn wheels.
    std::fs::create_dir_all("/var/lib/pgadmin/data/sessions")?;
    std::fs::create_dir_all("/var/lib/pgadmin/data/storage")?;
    set_dir_mode(OFFICIAL_PGADMIN_STATE_DIR, 0o755);

    if !Path::new("/var/lib/pgadmin/venv/bin/python3").is_file() {
        let venv_out = executor
            .run(
                AllowedBinary::Python3,
                &["-m", "venv", OFFICIAL_PGADMIN_VENV_DIR],
                Duration::from_secs(90),
            )
            .await?;
        if !venv_out.success {
            return Err(PanelError::CommandFailed {
                binary: AllowedBinary::Python3.label(),
                exit_code: venv_out.exit_code,
                stderr: venv_out.stderr,
            });
        }
    }

    let need_pip_install = !Path::new("/var/lib/pgadmin/venv/bin/gunicorn").is_file()
        || !has_valid_pgadmin_frontend_bundles()
        || detect_installed_pgadmin_version() != OFFICIAL_PGADMIN_DEFAULT_VERSION;

    if need_pip_install {
        let pip_out = executor
            .run(
                AllowedBinary::PgAdminPython,
                &[
                    "-m",
                    "pip",
                    "install",
                    "--quiet",
                    "--upgrade",
                    "pip",
                    "wheel",
                    "gunicorn",
                    OFFICIAL_PGADMIN_WHEEL_SPEC,
                ],
                pip_timeout,
            )
            .await?;
        if !pip_out.success {
            return Err(PanelError::CommandFailed {
                binary: AllowedBinary::PgAdminPython.label(),
                exit_code: pip_out.exit_code,
                stderr: pip_out.stderr,
            });
        }
    }

    // 3. Provision PostgreSQL SSO account + pgpass + pgAdmin config_local.py + servers.json.
    let _ = crate::modules::databases::ensure_pgadmin_sso_postgres_account(executor).await;
    LAST_PGADMIN_ACCOUNT_SYNC_SECS.store(
        current_epoch_secs(),
        std::sync::atomic::Ordering::Relaxed,
    );

    atomic_write_file(
        &Path::new(OFFICIAL_PGADMIN_STATE_DIR).join("config_local.py"),
        render_pgadmin_config_local().as_bytes(),
        0o644,
    )?;
    atomic_write_file(
        &Path::new(OFFICIAL_PGADMIN_STATE_DIR).join("servers.json"),
        render_pgadmin_servers_json().as_bytes(),
        0o640,
    )?;
    atomic_write_file(
        &Path::new(OFFICIAL_PGADMIN_STATE_DIR).join("wsgi_pgadmin.py"),
        render_pgadmin_wsgi_py().as_bytes(),
        0o644,
    )?;
    let bootstrap_path = Path::new(OFFICIAL_PGADMIN_STATE_DIR).join("bootstrap_pgadmin.py");
    atomic_write_file(&bootstrap_path, render_pgadmin_bootstrap_py().as_bytes(), 0o644)?;

    let boot_out = executor
        .run(
            AllowedBinary::PgAdminPython,
            &["/var/lib/pgadmin/bootstrap_pgadmin.py"],
            Duration::from_secs(90),
        )
        .await?;
    if !boot_out.success {
        tracing::warn!(
            exit_code = ?boot_out.exit_code,
            stderr = %boot_out.stderr.trim(),
            "pgadmin bootstrap script reported non-zero exit"
        );
    }

    if executor.is_binary_available(AllowedBinary::Chown) {
        let _ = executor
            .run(
                AllowedBinary::Chown,
                &["-R", "www-data:www-data", OFFICIAL_PGADMIN_STATE_DIR],
                Duration::from_secs(20),
            )
            .await;
    }
    set_dir_mode(crate::modules::databases::PGADMIN_PGPASS_FILE, 0o600);
    set_dir_mode(crate::modules::databases::PGADMIN_SSO_PASS_FILE, 0o600);

    atomic_write_file(
        Path::new(OFFICIAL_PGADMIN_UNIT_PATH),
        render_pgadmin_systemd_unit().as_bytes(),
        0o644,
    )?;

    if executor.is_binary_available(AllowedBinary::Systemctl) {
        let _ = executor
            .run(
                AllowedBinary::Systemctl,
                &["daemon-reload"],
                Duration::from_secs(15),
            )
            .await;
        let _ = executor
            .run(
                AllowedBinary::Systemctl,
                &["enable", OFFICIAL_PGADMIN_UNIT_NAME],
                Duration::from_secs(15),
            )
            .await;
        let restart_out = executor
            .run(
                AllowedBinary::Systemctl,
                &["restart", OFFICIAL_PGADMIN_UNIT_NAME],
                Duration::from_secs(25),
            )
            .await?;
        if !restart_out.success {
            return Err(PanelError::BadGateway(format!(
                "Không thể khởi chạy {OFFICIAL_PGADMIN_UNIT_NAME}: {}",
                restart_out.stderr.trim()
            )));
        }
    }

    for _ in 0..48 {
        if is_official_pgadmin_port_open().await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let status = get_official_pgadmin_status().await;
    if !status.running && executor.is_binary_available(AllowedBinary::Journalctl) {
        let detail = executor
            .run(
                AllowedBinary::Journalctl,
                &["-u", OFFICIAL_PGADMIN_UNIT_NAME, "-n", "15", "--no-pager"],
                Duration::from_secs(5),
            )
            .await
            .map(|o| o.stdout)
            .unwrap_or_default();
        return Err(PanelError::BadGateway(format!(
            "Dịch vụ {} ({}) đã cài đặt nhưng chưa mở cổng 127.0.0.1:{OFFICIAL_PGADMIN_PORT}. Nhật ký: {}",
            status.display_name,
            OFFICIAL_PGADMIN_UNIT_NAME,
            detail.trim()
        )));
    }

    Ok(status)
}

pub async fn redirect_pgadmin_slash(req: HttpRequest) -> HttpResponse {
    let qs = req.query_string();
    let loc = if qs.is_empty() {
        "/pgadmin/".to_string()
    } else {
        format!("/pgadmin/?{qs}")
    };
    HttpResponse::Found()
        .insert_header(("Location", loc))
        .insert_header(("Cache-Control", "no-store"))
        .finish()
}

fn render_official_pgadmin_launcher_html(version: &str, require_login_only: bool) -> String {
    let display = format!("pgAdmin 4 ({version})");
    let template = r##"<!DOCTYPE html>
<html lang="vi" class="dark">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>__PGA_NAME__ &amp; FPSql &amp; Database — FerrousPanel</title>
  <style>
    * { box-sizing: border-box; }
    body { margin:0; font-family:system-ui,-apple-system,sans-serif; background:#090d16; color:#e2e8f0; display:flex; align-items:center; justify-content:center; min-height:100vh; padding:20px; }
    .card { max-width:600px; width:100%; background:#111827; border:1px solid #1e293b; border-radius:14px; padding:26px; box-shadow:0 24px 48px rgba(0,0,0,.45); }
    h1 { margin:0 0 8px; font-size:20px; }
    p { margin:0 0 16px; color:#94a3b8; font-size:14px; line-height:1.55; }
    .actions { display:flex; gap:10px; flex-wrap:wrap; margin-top:18px; }
    .btn { display:inline-flex; align-items:center; justify-content:center; padding:10px 16px; border-radius:9px; font-weight:600; font-size:13px; text-decoration:none; cursor:pointer; border:1px solid #334155; background:#1e293b; color:#e2e8f0; }
    .btn-primary { background:#f97316; border-color:#ea580c; color:#fff; }
    .btn-blue { background:rgba(59,130,246,.18); border-color:rgba(59,130,246,.45); color:#93c5fd; }
    .auth-box { margin-top:14px; padding:14px; border-radius:10px; background:#0b1220; border:1px solid #1e293b; }
    .auth-box input { width:100%; margin:6px 0 10px; padding:9px 11px; border-radius:8px; border:1px solid #334155; background:#020617; color:#e2e8f0; font-size:13px; }
    .auth-box label { font-size:12px; color:#94a3b8; display:block; }
    #msg { margin-top:14px; font-size:13px; display:none; padding:10px 12px; border-radius:8px; white-space:pre-wrap; word-break:break-word; }
  </style>
</head>
<body>
  <div class="card">
    <h1>PostgreSQL Studio — __PGA_NAME__ &amp; FPSql</h1>
    <p>Dành cho Website &amp; Ứng dụng sử dụng <strong>PostgreSQL</strong>: tự động clone <code>pgadmin4</code> về máy chủ, tự động kết nối SSO tới PostgreSQL cục bộ và đồng bộ dữ liệu 2 chiều với <strong>FPSql &amp; Database</strong>.</p>
    <div id="msg"></div>
    <form id="inlineAuthBox" class="auth-box" style="display:__AUTH_DISPLAY__;">
      <div style="font-weight:600;font-size:13px;margin-bottom:8px;color:#fb923c;">Đăng nhập Quản trị FerrousPanel để mở __PGA_NAME__</div>
      <label for="fpUser">Tài khoản Panel / Username</label>
      <input id="fpUser" type="text" value="admin_ferrous" required autocomplete="username" />
      <label for="fpPass">Mật khẩu / Password</label>
      <input id="fpPass" type="password" required autocomplete="current-password" />
      <button type="submit" class="btn btn-primary" id="btnAuthAndOpen">Đăng nhập &amp; Mở __PGA_NAME__</button>
    </form>
    <div class="actions">
      <button type="button" class="btn btn-primary" id="btnInstallPgAdmin" style="display:__INSTALL_DISPLAY__;">Clone &amp; Kích hoạt __PGA_NAME__ (1-Click)</button>
      <a href="/phpadmin/" class="btn btn-blue">Mở FPSql &amp; Database (/phpadmin/) →</a>
    </div>
  </div>
  <script>
    (function () {
      const requireLoginOnly = __LOGIN_ONLY__;
      const apiBase = window.location.pathname.startsWith('/pgadmin') ? '/pgadmin/api' : '/api';
      const btn = document.getElementById('btnInstallPgAdmin');
      const msg = document.getElementById('msg');
      const authBox = document.getElementById('inlineAuthBox');

      function showMsg(text, kind) {
        msg.style.display = 'block';
        msg.style.background = kind === 'ok' ? 'rgba(16,185,129,.16)' : (kind === 'err' ? 'rgba(239,68,68,.16)' : 'rgba(59,130,246,.14)');
        msg.style.color = kind === 'ok' ? '#6ee7b7' : (kind === 'err' ? '#fca5a5' : '#93c5fd');
        msg.textContent = text;
      }

      async function runInstall(explicitToken) {
        if (btn) {
          btn.disabled = true;
          btn.textContent = 'Đang clone & khởi chạy __PGA_NAME__…';
        }
        showMsg('Đang clone pgAdmin 4 & cấu hình đồng bộ PostgreSQL (khoảng 30–90 giây lần đầu)…', 'info');
        try {
          const tok = explicitToken || sessionStorage.getItem('fp_pma_token') || sessionStorage.getItem('fp_session_token') || localStorage.getItem('fp_session_token') || '';
          const headers = { 'Content-Type': 'application/json', 'X-Requested-With': 'FerrousPanel-UI' };
          if (tok) headers['Authorization'] = 'Bearer ' + tok;
          const resp = await fetch(apiBase + '/databases/pgadmin/install', {
            method: 'POST',
            credentials: 'same-origin',
            headers,
            body: '{}'
          });
          const data = await resp.json().catch(() => ({}));
          if (resp.status === 401 || resp.status === 403) {
            if (btn) { btn.disabled = false; btn.textContent = 'Clone & Kích hoạt __PGA_NAME__ (1-Click)'; }
            authBox.style.display = 'block';
            showMsg('Vui lòng đăng nhập tài khoản quản trị Panel để kích hoạt __PGA_NAME__.', 'info');
            return;
          }
          if (!resp.ok) throw new Error(data.error || ('HTTP ' + resp.status + ' — Cài đặt thất bại.'));
          showMsg('Đã kích hoạt ' + (data.displayName || '__PGA_NAME__') + '! Đang mở…', 'ok');
          const nextUrl = tok ? ('/pgadmin/?fp_sso=' + encodeURIComponent(tok)) : '/pgadmin/';
          setTimeout(() => { window.location.href = nextUrl; }, 500);
        } catch (e) {
          if (btn) { btn.disabled = false; btn.textContent = 'Thử kích hoạt lại'; }
          showMsg(e.message || 'Không thể cài đặt __PGA_NAME__.', 'err');
        }
      }

      if (btn) btn.addEventListener('click', () => runInstall(''));

      authBox.addEventListener('submit', async function (ev) {
        ev.preventDefault();
        const sub = document.getElementById('btnAuthAndOpen');
        sub.disabled = true;
        try {
          const r = await fetch(apiBase + '/auth/login', {
            method: 'POST',
            credentials: 'same-origin',
            headers: { 'Content-Type': 'application/json', 'X-Requested-With': 'FerrousPanel-UI' },
            body: JSON.stringify({
              username: document.getElementById('fpUser').value.trim(),
              password: document.getElementById('fpPass').value,
              rememberMe: false
            })
          });
          const d = await r.json().catch(() => ({}));
          if (!r.ok || !d.token) throw new Error(d.error || 'Sai tài khoản hoặc mật khẩu.');
          sessionStorage.setItem('fp_pma_token', d.token);
          if (requireLoginOnly) {
            window.location.href = '/pgadmin/?fp_sso=' + encodeURIComponent(d.token);
            return;
          }
          authBox.style.display = 'none';
          await runInstall(d.token);
        } catch (e) {
          showMsg(e.message || 'Đăng nhập thất bại.', 'err');
        } finally {
          sub.disabled = false;
        }
      });
    })();
  </script>
</body>
</html>"##;
    template
        .replace("__PGA_NAME__", &display)
        .replace(
            "__AUTH_DISPLAY__",
            if require_login_only { "block" } else { "none" },
        )
        .replace(
            "__INSTALL_DISPLAY__",
            if require_login_only { "none" } else { "inline-flex" },
        )
        .replace(
            "__LOGIN_ONLY__",
            if require_login_only { "true" } else { "false" },
        )
}

fn verify_panel_session_for_pgadmin(req: &HttpRequest, state: &AppState) -> (bool, Option<String>) {
    // Check explicit `?fp_sso=` / header / main cookie first so fresh tokens override stale cookies.
    let (pma_ok, pma_tok) = verify_panel_session_for_pma(req, state);
    if pma_ok {
        return (true, pma_tok);
    }
    if let Some(c) = req.cookie("fp_pgadmin_sso") {
        let tok = c.value().trim().to_string();
        if !tok.is_empty()
            && state
                .auth_store
                .verify_session(&tok)
                .ok()
                .flatten()
                .is_some()
        {
            return (true, Some(tok));
        }
    }
    (false, None)
}

/// Forwards an authenticated `/pgadmin/*` request to an open Gunicorn TCP stream (`127.0.0.1:18890`
/// in production, or an ephemeral test listener in integration tests) and normalizes cookies,
/// `Location`, `X-Frame-Options`, `Content-Security-Policy`, and the `FPSql & Database` switcher badge.
pub async fn forward_pgadmin_http_stream(
    mut stream: TcpStream,
    upstream_port: u16,
    req: &HttpRequest,
    body: &[u8],
    verified_token: Option<&str>,
) -> HttpResponse {
    let method = req.method().as_str();
    let path = req.uri().path();
    let cleaned_query = req
        .uri()
        .query()
        .map(strip_fp_sso_from_query)
        .unwrap_or_default();
    let target_uri = if !cleaned_query.is_empty() {
        format!("{path}?{cleaned_query}")
    } else {
        path.to_string()
    };

    let (host, scheme) = {
        let conn_info = req.connection_info();
        let fwd_proto = req
            .headers()
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        (
            conn_info.host().to_string(),
            fwd_proto.unwrap_or_else(|| conn_info.scheme().to_string()),
        )
    };

    let mut raw_req = Vec::with_capacity(512 + body.len());
    raw_req.extend_from_slice(format!("{method} {target_uri} HTTP/1.1\r\n").as_bytes());
    raw_req.extend_from_slice(format!("Host: {host}\r\n").as_bytes());
    raw_req.extend_from_slice(b"Connection: close\r\n");
    raw_req.extend_from_slice(b"X-Script-Name: /pgadmin\r\n");
    raw_req.extend_from_slice(b"X-Forwarded-Prefix: /pgadmin\r\n");
    raw_req.extend_from_slice(format!("X-Scheme: {scheme}\r\n").as_bytes());
    raw_req.extend_from_slice(format!("X-Forwarded-Proto: {scheme}\r\n").as_bytes());
    raw_req.extend_from_slice(format!("X-Forwarded-Host: {host}\r\n").as_bytes());

    for (hname, hval) in req.headers() {
        let lower = hname.as_str().to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "host"
                | "connection"
                | "transfer-encoding"
                | "content-length"
                | "accept-encoding"
                | "x-script-name"
                | "x-forwarded-prefix"
                | "x-scheme"
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
    raw_req.extend_from_slice(body);

    if tokio::time::timeout(Duration::from_secs(60), stream.write_all(&raw_req))
        .await
        .map_err(|_| ())
        .and_then(|r| r.map_err(|_| ()))
        .is_err()
    {
        return HttpResponse::BadGateway()
            .content_type("text/plain; charset=utf-8")
            .body("Failed to forward request to pgAdmin worker");
    }

    let mut resp_bytes = Vec::with_capacity(32 * 1024);
    if tokio::time::timeout(Duration::from_secs(120), stream.read_to_end(&mut resp_bytes))
        .await
        .map_err(|_| ())
        .and_then(|r| r.map_err(|_| ()))
        .is_err()
    {
        return HttpResponse::GatewayTimeout()
            .content_type("text/plain; charset=utf-8")
            .body("pgAdmin worker timed out");
    }

    let Some(hdr_end) = resp_bytes.windows(4).position(|w| w == b"\r\n\r\n") else {
        return HttpResponse::BadGateway()
            .content_type("text/plain; charset=utf-8")
            .body("Invalid upstream response from pgAdmin worker");
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
    if let Some(tok) = verified_token {
        if !tok.contains(';') {
            if req
                .cookie(crate::auth::middleware::SESSION_COOKIE_NAME)
                .map(|c| c.value().to_string())
                .as_deref()
                != Some(tok)
            {
                builder.append_header((
                    "Set-Cookie",
                    format!(
                        "{}={tok}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400",
                        crate::auth::middleware::SESSION_COOKIE_NAME
                    ),
                ));
            }
            if req
                .cookie("fp_pgadmin_sso")
                .map(|c| c.value().to_string())
                .as_deref()
                != Some(tok)
            {
                builder.append_header((
                    "Set-Cookie",
                    format!("fp_pgadmin_sso={tok}; Path=/pgadmin; HttpOnly; SameSite=Lax; Max-Age=86400"),
                ));
            }
        }
    }

    let mut is_chunked = false;
    let mut is_html = false;
    let internal_origin = format!("http://127.0.0.1:{upstream_port}");

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
        if lower == "x-frame-options" {
            builder.insert_header(("X-Frame-Options", "SAMEORIGIN"));
            continue;
        }
        if lower == "content-security-policy" {
            let mut rewritten = val_trim.replace("frame-ancestors 'none'", "frame-ancestors 'self'");
            if !rewritten.contains("frame-ancestors") {
                rewritten.push_str(" frame-ancestors 'self';");
            }
            builder.append_header((key_trim.to_string(), rewritten));
            continue;
        }
        if lower == "location" {
            let stripped = val_trim
                .strip_prefix(&internal_origin)
                .unwrap_or(val_trim);
            let rewritten = if (stripped.starts_with("http://") || stripped.starts_with("https://"))
                && stripped.contains("/pgadmin")
            {
                stripped
                    .find("/pgadmin")
                    .map(|idx| &stripped[idx..])
                    .unwrap_or(stripped)
            } else if stripped.starts_with('/') && !stripped.starts_with("/pgadmin") {
                return_prefixed_pgadmin_location(stripped)
            } else {
                stripped
            };
            builder.append_header((key_trim.to_string(), rewritten.to_string()));
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

    if is_html && method != "HEAD" {
        let badge = b"<a href=\"/phpadmin/\" style=\"position:fixed;bottom:12px;right:12px;z-index:99999;background:#0f172a;color:#fb923c;border:1px solid #f97316;padding:6px 12px;border-radius:999px;font:600 12px system-ui,sans-serif;text-decoration:none;box-shadow:0 4px 14px rgba(0,0,0,.35)\">\xE2\x87\x84 FPSql &amp; Database (/phpadmin/)</a></body>";
        if let Some(pos) = decoded_body
            .windows(7)
            .rposition(|w| w.eq_ignore_ascii_case(b"</body>"))
        {
            decoded_body.splice(pos..pos + 7, badge.iter().copied());
        }
    }

    builder.body(decoded_body)
}

fn return_prefixed_pgadmin_location(stripped: &str) -> &str {
    // Static / root-relative paths emitted without SCRIPT_NAME (defensive fallback)
    if stripped == "/browser/" || stripped == "/login" {
        if stripped == "/browser/" {
            "/pgadmin/browser/"
        } else {
            "/pgadmin/login"
        }
    } else {
        stripped
    }
}

/// Reverse-proxies `/pgadmin/*` to the local `ferrous-pgadmin.service` worker (`127.0.0.1:18890`),
/// enforcing FerrousPanel session SSO, automatically repairing broken/outdated `pgadmin4` venvs,
/// and keeping PostgreSQL databases synchronized 2-way with `FPSql & Database` without blocking assets.
pub async fn serve_official_pgadmin(
    req: HttpRequest,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> HttpResponse {
    // Auto-heal existing installations (e.g. upgrading from broken PyPI pgadmin4 9.19 missing
    // static/js/generated/* bundles or older v1/v2 config_local.py) automatically!
    if state.config.is_production()
        && Path::new("/var/lib/pgadmin/venv/bin/python3").is_file()
        && !is_pgadmin_installation_healthy()
    {
        let _ = install_or_repair_official_pgadmin(state.webserver.executor().as_ref()).await;
    }

    let mut stream_opt = TcpStream::connect(("127.0.0.1", OFFICIAL_PGADMIN_PORT))
        .await
        .ok();

    if stream_opt.is_none()
        && state.config.is_production()
        && Path::new("/var/lib/pgadmin/venv/bin/gunicorn").is_file()
    {
        if !is_pgadmin_installation_healthy() {
            let _ = install_or_repair_official_pgadmin(state.webserver.executor().as_ref()).await;
        } else if state
            .webserver
            .executor()
            .is_binary_available(AllowedBinary::Systemctl)
        {
            let _ = state
                .webserver
                .executor()
                .run(
                    AllowedBinary::Systemctl,
                    &["restart", OFFICIAL_PGADMIN_UNIT_NAME],
                    Duration::from_secs(15),
                )
                .await;
        }
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if let Ok(s) = TcpStream::connect(("127.0.0.1", OFFICIAL_PGADMIN_PORT)).await {
                stream_opt = Some(s);
                break;
            }
        }
    }

    let ver = detect_installed_pgadmin_version();
    let Some(stream) = stream_opt else {
        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .insert_header(("Cache-Control", "no-store"))
            .body(render_official_pgadmin_launcher_html(&ver, false));
    };

    let (sso_authorized, verified_token) = verify_panel_session_for_pgadmin(&req, &state);
    if !sso_authorized {
        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .insert_header(("Cache-Control", "no-store"))
            .body(render_official_pgadmin_launcher_html(&ver, true));
    }

    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();
    if state.config.is_production() && (path == "/pgadmin/" || path == "/pgadmin/browser/") {
        let now = current_epoch_secs();
        let prev_acct =
            LAST_PGADMIN_ACCOUNT_SYNC_SECS.load(std::sync::atomic::Ordering::Relaxed);
        if now.saturating_sub(prev_acct) >= 60 {
            LAST_PGADMIN_ACCOUNT_SYNC_SECS
                .store(now, std::sync::atomic::Ordering::Relaxed);
            let _ = crate::modules::databases::ensure_pgadmin_sso_postgres_account(
                state.webserver.executor().as_ref(),
            )
            .await;
        }
        let prev_rec =
            LAST_PGADMIN_DB_RECONCILE_SECS.load(std::sync::atomic::Ordering::Relaxed);
        if now.saturating_sub(prev_rec) >= 15 {
            LAST_PGADMIN_DB_RECONCILE_SECS
                .store(now, std::sync::atomic::Ordering::Relaxed);
            let bg_state = state.clone();
            actix_web::rt::spawn(async move {
                crate::modules::databases::reconcile_live_postgres_databases_into_store(
                    &bg_state.modules,
                    bg_state.webserver.executor().as_ref(),
                )
                .await;
            });
        }
    }

    let resp = forward_pgadmin_http_stream(
        stream,
        OFFICIAL_PGADMIN_PORT,
        &req,
        &body,
        verified_token.as_deref(),
    )
    .await;

    if state.config.is_production()
        && method != "GET"
        && method != "HEAD"
        && !path.contains("/heart_beat")
    {
        let now = current_epoch_secs();
        let prev_rec =
            LAST_PGADMIN_DB_RECONCILE_SECS.load(std::sync::atomic::Ordering::Relaxed);
        if now.saturating_sub(prev_rec) >= 8 {
            LAST_PGADMIN_DB_RECONCILE_SECS
                .store(now, std::sync::atomic::Ordering::Relaxed);
            let bg_state = state.clone();
            actix_web::rt::spawn(async move {
                crate::modules::databases::reconcile_live_postgres_databases_into_store(
                    &bg_state.modules,
                    bg_state.webserver.executor().as_ref(),
                )
                .await;
            });
        }
    }

    resp
}

