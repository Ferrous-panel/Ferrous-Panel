use actix_web::{HttpRequest, HttpResponse};

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

      function sqlQuoteVal(val) {
        if (val === null || val === undefined || val === 'NULL') return 'NULL';
        return "'" + String(val).replace(/\\/g, '\\\\').replace(/'/g, "''") + "'";
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
              const npw = window.prompt('Nhập mật khẩu mới cho user của database "' + nm + '":');
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
              if (!window.confirm('Xác nhận xóa vĩnh viễn database này?')) return;
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
            const colSpec = window.prompt('Nhập định nghĩa cột mới cho bảng "' + tbl + '" (VD: status VARCHAR(64) DEFAULT \'active\'):', 'extra_note TEXT');
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
            if (!window.confirm('Xác nhận làm rỗng (TRUNCATE) toàn bộ dữ liệu trong bảng "' + tbl + '"?')) return;
            try {
              databases = await api('/databases/' + encodeURIComponent(db.id) + '/tables/' + encodeURIComponent(tbl) + '/truncate', { method: 'POST' });
              showBanner('Đã làm rỗng bảng ' + tbl, false);
              renderSidebar(); renderTab();
            } catch (err) { showBanner(err.message, true); }
          }));
          el.querySelectorAll('[data-drop-tbl]').forEach((b) => b.addEventListener('click', async () => {
            const tbl = b.getAttribute('data-drop-tbl');
            if (!window.confirm('Xác nhận XÓA BẢNG (DROP TABLE) "' + tbl + '"?')) return;
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
                    sets.push(sqlQuoteId(c, isMysql) + ' = ' + sqlQuoteVal(newVal));
                  }
                });
                if (!sets.length) {
                  editBox.classList.add('hidden');
                  return;
                }
                const sql = 'UPDATE ' + sqlQuoteId(activeTable, isMysql) + ' SET ' + sets.join(', ') +
                  ' WHERE ' + sqlQuoteId(pkCol, isMysql) + ' = ' + sqlQuoteVal(pkVal) + ';';
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
              if (!window.confirm('Xác nhận xóa dòng có ' + pkCol + ' = ' + pkVal + ' khỏi bảng ' + activeTable + '?')) return;
              const sql = 'DELETE FROM ' + sqlQuoteId(activeTable, isMysql) + ' WHERE ' + sqlQuoteId(pkCol, isMysql) + ' = ' + sqlQuoteVal(pkVal) + ';';
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
              colVals.push(raw === '' && c.nullable ? 'NULL' : sqlQuoteVal(raw));
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
            const ddl = isSqlite
              ? 'CREATE TABLE ' + t + ' (' + c1 + ' INTEGER PRIMARY KEY AUTOINCREMENT, ' + c2 + ' TEXT NOT NULL, ' + c3 + " TEXT DEFAULT (datetime('now')));"
              : isPg
              ? 'CREATE TABLE ' + t + ' (' + c1 + ' BIGSERIAL PRIMARY KEY, ' + c2 + ' VARCHAR(255) NOT NULL, ' + c3 + ' TIMESTAMPTZ DEFAULT NOW());'
              : 'CREATE TABLE `' + t + '` (`' + c1 + '` BIGINT AUTO_INCREMENT PRIMARY KEY, `' + c2 + '` VARCHAR(255) NOT NULL, `' + c3 + '` TIMESTAMP DEFAULT CURRENT_TIMESTAMP);';
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

/// Serves the standalone Pure-Rust FerrousAdmin SQL Studio (Zero PHP replacement for phpMyAdmin)
/// at `/phpadmin`, `/phpadmin/`, `/phpmyadmin`, and `/phpmyadmin/`.
pub async fn serve_ferrous_admin(_req: HttpRequest) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .insert_header(("X-Frame-Options", "SAMEORIGIN"))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .insert_header(("Cache-Control", "no-store"))
        .body(FERROUS_ADMIN_HTML)
}
