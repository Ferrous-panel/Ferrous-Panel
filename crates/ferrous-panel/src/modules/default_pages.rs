use crate::webserver::atomic_write_file;
use std::path::Path;

pub const DEFAULT_INDEX_MARKER: &str = "FerrousPanel-Managed-Default-Index";
pub const SITE_PLACEHOLDER_MARKER: &str = "FerrousPanel-Managed-Site-Placeholder";

const UNCONFIGURED_HOST_HTML: &str = r##"<!DOCTYPE html>
<!-- FerrousPanel-Managed-Default-Index -->
<html lang="vi">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Ferrous Edge Node — Ready</title>
  <style>
    :root {
      --bg: #060913;
      --card: rgba(15, 23, 42, 0.78);
      --border: rgba(148, 163, 184, 0.16);
      --text: #f8fafc;
      --muted: #94a3b8;
      --accent: #f97316;
      --accent-2: #38bdf8;
      --ok: #22c55e;
      --mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
      --sans: -apple-system, BlinkMacSystemFont, "Plus Jakarta Sans", "Segoe UI", system-ui, sans-serif;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      min-height: 100vh;
      font-family: var(--sans);
      color: var(--text);
      background:
        radial-gradient(circle at 15% 15%, rgba(249, 115, 22, 0.16), transparent 42%),
        radial-gradient(circle at 85% 80%, rgba(56, 189, 248, 0.14), transparent 45%),
        var(--bg);
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      padding: 24px 16px;
    }
    .shell {
      width: 100%;
      max-width: 720px;
      background: var(--card);
      backdrop-filter: blur(18px);
      -webkit-backdrop-filter: blur(18px);
      border: 1px solid var(--border);
      border-radius: 20px;
      padding: 34px 32px;
      box-shadow: 0 28px 64px rgba(0, 0, 0, 0.55);
      position: relative;
      overflow: hidden;
    }
    .shell::before {
      content: "";
      position: absolute;
      inset: 0 0 auto 0;
      height: 3px;
      background: linear-gradient(90deg, var(--accent), #fb923c, var(--accent-2));
    }
    .top {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      flex-wrap: wrap;
      margin-bottom: 24px;
    }
    .pill {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      font-family: var(--mono);
      font-size: 12px;
      color: #cbd5e1;
      background: rgba(2, 6, 23, 0.65);
      border: 1px solid var(--border);
      border-radius: 999px;
      padding: 6px 13px;
    }
    .dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--ok);
      box-shadow: 0 0 10px var(--ok);
    }
    .lang-btn {
      font-family: var(--mono);
      font-size: 12px;
      font-weight: 600;
      color: var(--text);
      background: rgba(30, 41, 59, 0.75);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 6px 12px;
      cursor: pointer;
    }
    .lang-btn:hover { border-color: var(--accent); color: #fdba74; }
    h1 {
      margin: 0 0 10px;
      font-size: clamp(23px, 4vw, 30px);
      font-weight: 700;
      letter-spacing: -0.02em;
      line-height: 1.25;
    }
    .host-box {
      display: inline-block;
      font-family: var(--mono);
      font-size: 14px;
      color: #fdba74;
      background: rgba(249, 115, 22, 0.12);
      border: 1px solid rgba(249, 115, 22, 0.32);
      border-radius: 8px;
      padding: 5px 11px;
      margin-bottom: 16px;
      word-break: break-all;
    }
    p.lead {
      margin: 0 0 22px;
      color: var(--muted);
      font-size: 15px;
      line-height: 1.65;
    }
    .steps {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 12px;
      margin-bottom: 24px;
    }
    .step {
      background: rgba(2, 6, 23, 0.52);
      border: 1px solid var(--border);
      border-radius: 12px;
      padding: 14px 15px;
    }
    .step-num {
      font-family: var(--mono);
      font-size: 11px;
      font-weight: 700;
      color: var(--accent);
      margin-bottom: 5px;
    }
    .step-title {
      font-size: 13.5px;
      font-weight: 600;
      margin-bottom: 4px;
    }
    .step-desc {
      font-size: 12.5px;
      color: var(--muted);
      line-height: 1.5;
    }
    .foot {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
      flex-wrap: wrap;
      padding-top: 16px;
      border-top: 1px solid var(--border);
      font-size: 12px;
      color: #64748b;
      font-family: var(--mono);
    }
    @media (max-width: 600px) {
      .shell { padding: 24px 18px; }
    }
  </style>
</head>
<body>
  <main class="shell">
    <div class="top">
      <div class="pill"><span class="dot"></span><span>PURE-RUST EDGE ENGINE · ONLINE</span></div>
      <button type="button" class="lang-btn" id="langBtn">EN / VI</button>
    </div>
    <div class="host-box" id="hostLabel">{{HOST}}</div>
    <h1 id="tTitle">Máy chủ Web đang hoạt động ổn định</h1>
    <p class="lead" id="tLead">Địa chỉ IP hoặc tên miền này đã trỏ thành công về máy chủ nhưng chưa được gắn với Website nào trên hệ thống điều hành <strong>FerrousPanel</strong>.</p>
    <div class="steps">
      <div class="step">
        <div class="step-num">01 · DOMAIN</div>
        <div class="step-title" id="s1Title">Thêm Website mới</div>
        <div class="step-desc" id="s1Desc">Mở FerrousPanel → Website &amp; Ứng dụng → Thêm tên miền này để khởi tạo VirtualHost riêng.</div>
      </div>
      <div class="step">
        <div class="step-num">02 · DEPLOY</div>
        <div class="step-title" id="s2Title">Triển khai Mã nguồn</div>
        <div class="step-desc" id="s2Desc">Kết nối Git tự động build (Rust / PHP / Bun / Static) hoặc tải tệp lên /www/wwwroot.</div>
      </div>
      <div class="step">
        <div class="step-num">03 · HTTPS</div>
        <div class="step-title" id="s3Title">Bật SSL Let's Encrypt</div>
        <div class="step-desc" id="s3Desc">Kích hoạt chứng chỉ HTTPS chuẩn TLS 1.3 &amp; HTTP/2 tự động gia hạn chỉ với 1 cú nhấp.</div>
      </div>
    </div>
    <div class="foot">
      <span>Powered by FerrousPanel &amp; FerrousX</span>
      <span id="timeLabel">HTTP/2 · TLS 1.3 Ready</span>
    </div>
  </main>
  <script>
    (function () {
      var hostEl = document.getElementById('hostLabel');
      if (hostEl && (hostEl.textContent.indexOf('{{') !== -1 || !hostEl.textContent.trim())) {
        hostEl.textContent = window.location.host || window.location.hostname || 'edge-node';
      }
      var dict = {
        vi: {
          btn: 'English',
          title: 'Máy chủ Web đang hoạt động ổn định',
          lead: 'Địa chỉ IP hoặc tên miền này đã trỏ thành công về máy chủ nhưng chưa được gắn với Website nào trên hệ thống điều hành FerrousPanel.',
          s1t: 'Thêm Website mới',
          s1d: 'Mở FerrousPanel → Website & Ứng dụng → Thêm tên miền này để khởi tạo VirtualHost riêng.',
          s2t: 'Triển khai Mã nguồn',
          s2d: 'Kết nối Git tự động build (Rust / PHP / Bun / Static) hoặc tải tệp lên /www/wwwroot.',
          s3t: 'Bật SSL Let\'s Encrypt',
          s3d: 'Kích hoạt chứng chỉ HTTPS chuẩn TLS 1.3 & HTTP/2 tự động gia hạn chỉ với 1 cú nhấp.'
        },
        en: {
          btn: 'Tiếng Việt',
          title: 'Web Edge Server Is Up & Running',
          lead: 'This IP address or domain is pointing to this server, but no website virtual host has been assigned to it in FerrousPanel yet.',
          s1t: 'Add Your Domain',
          s1d: 'Open FerrousPanel → Websites & Apps → Add this domain to provision a dedicated VirtualHost.',
          s2t: 'Deploy Source Code',
          s2d: 'Connect a Git repository for zero-downtime builds or upload files into /www/wwwroot.',
          s3t: 'Enable Let\'s Encrypt SSL',
          s3d: 'Issue a trusted TLS 1.3 & HTTP/2 certificate with automatic renewal in one click.'
        }
      };
      var lang = localStorage.getItem('fp_welcome_lang') || ((navigator.language || '').toLowerCase().indexOf('vi') === 0 ? 'vi' : 'en');
      function apply(l) {
        lang = l === 'en' ? 'en' : 'vi';
        localStorage.setItem('fp_welcome_lang', lang);
        var d = dict[lang];
        document.documentElement.lang = lang;
        document.getElementById('langBtn').textContent = d.btn;
        document.getElementById('tTitle').textContent = d.title;
        document.getElementById('tLead').textContent = d.lead;
        document.getElementById('s1Title').textContent = d.s1t;
        document.getElementById('s1Desc').textContent = d.s1d;
        document.getElementById('s2Title').textContent = d.s2t;
        document.getElementById('s2Desc').textContent = d.s2d;
        document.getElementById('s3Title').textContent = d.s3t;
        document.getElementById('s3Desc').textContent = d.s3d;
      }
      document.getElementById('langBtn').addEventListener('click', function () {
        apply(lang === 'vi' ? 'en' : 'vi');
      });
      apply(lang);
    })();
  </script>
</body>
</html>
"##;

const SITE_PLACEHOLDER_TEMPLATE: &str = r##"<!DOCTYPE html>
<!-- FerrousPanel-Managed-Site-Placeholder -->
<html lang="vi">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>__SITE_DOMAIN__ — Ready on FerrousPanel</title>
  <style>
    :root {
      --bg: #050811;
      --card: rgba(15, 23, 42, 0.82);
      --border: rgba(148, 163, 184, 0.18);
      --text: #f8fafc;
      --muted: #94a3b8;
      --accent: #10b981;
      --orange: #f97316;
      --mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
      --sans: -apple-system, BlinkMacSystemFont, "Plus Jakarta Sans", "Segoe UI", system-ui, sans-serif;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      min-height: 100vh;
      font-family: var(--sans);
      color: var(--text);
      background:
        radial-gradient(circle at 20% 20%, rgba(16, 185, 129, 0.15), transparent 42%),
        radial-gradient(circle at 82% 78%, rgba(249, 115, 22, 0.14), transparent 44%),
        var(--bg);
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 24px 16px;
    }
    .card {
      width: 100%;
      max-width: 680px;
      background: var(--card);
      backdrop-filter: blur(18px);
      -webkit-backdrop-filter: blur(18px);
      border: 1px solid var(--border);
      border-radius: 20px;
      padding: 34px 32px;
      box-shadow: 0 28px 64px rgba(0, 0, 0, 0.55);
      position: relative;
      overflow: hidden;
    }
    .card::before {
      content: "";
      position: absolute;
      inset: 0 0 auto 0;
      height: 3px;
      background: linear-gradient(90deg, var(--accent), var(--orange));
    }
    .head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      flex-wrap: wrap;
      margin-bottom: 22px;
    }
    .badge {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      font-family: var(--mono);
      font-size: 12px;
      color: #a7f3d0;
      background: rgba(16, 185, 129, 0.12);
      border: 1px solid rgba(16, 185, 129, 0.35);
      border-radius: 999px;
      padding: 6px 13px;
    }
    .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 10px var(--accent); }
    .lang-btn {
      font-family: var(--mono);
      font-size: 12px;
      font-weight: 600;
      color: var(--text);
      background: rgba(30, 41, 59, 0.75);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 6px 12px;
      cursor: pointer;
    }
    h1 { margin: 0 0 10px; font-size: clamp(22px, 4vw, 29px); font-weight: 700; letter-spacing: -0.02em; }
    .domain { color: #fb923c; font-family: var(--mono); word-break: break-all; }
    p { margin: 0 0 18px; color: var(--muted); font-size: 15px; line-height: 1.65; }
    .meta-box {
      background: rgba(2, 6, 23, 0.58);
      border: 1px solid var(--border);
      border-radius: 12px;
      padding: 14px 16px;
      font-family: var(--mono);
      font-size: 12.5px;
      color: #cbd5e1;
      line-height: 1.75;
    }
    .foot {
      margin-top: 20px;
      padding-top: 14px;
      border-top: 1px solid var(--border);
      display: flex;
      justify-content: space-between;
      flex-wrap: wrap;
      gap: 8px;
      font-family: var(--mono);
      font-size: 12px;
      color: #64748b;
    }
  </style>
</head>
<body>
  <main class="card">
    <div class="head">
      <div class="badge"><span class="dot"></span><span id="bText">WEBSITE ĐÃ KHỞI TẠO THÀNH CÔNG</span></div>
      <button type="button" class="lang-btn" id="langBtn">English</button>
    </div>
    <h1><span class="domain">__SITE_DOMAIN__</span></h1>
    <p id="pDesc">Website đã sẵn sàng phục vụ! Hãy tải mã nguồn lên thư mục gốc hoặc bấm <strong>Git Pull &amp; Deploy</strong> trong <strong>FerrousPanel</strong> để thay thế trang mặc định này.</p>
    <div class="meta-box">
      <div>Document Root : <strong>/www/wwwroot/__SITE_DOMAIN__</strong></div>
      <div>Runtime Stack : <strong>__SITE_RUNTIME__</strong></div>
    </div>
    <div class="foot">
      <span>FerrousPanel Pure-Rust Hosting</span>
      <span>Zero-Node.js · TLS 1.3</span>
    </div>
  </main>
  <script>
    (function () {
      var lang = localStorage.getItem('fp_welcome_lang') || ((navigator.language || '').toLowerCase().indexOf('vi') === 0 ? 'vi' : 'en');
      function apply(l) {
        lang = l === 'en' ? 'en' : 'vi';
        localStorage.setItem('fp_welcome_lang', lang);
        document.documentElement.lang = lang;
        document.getElementById('langBtn').textContent = lang === 'vi' ? 'English' : 'Tiếng Việt';
        document.getElementById('bText').textContent = lang === 'vi' ? 'WEBSITE ĐÃ KHỞI TẠO THÀNH CÔNG' : 'VIRTUAL HOST READY & ONLINE';
        document.getElementById('pDesc').innerHTML = lang === 'vi'
          ? 'Website đã sẵn sàng phục vụ! Hãy tải mã nguồn lên thư mục gốc hoặc bấm <strong>Git Pull &amp; Deploy</strong> trong <strong>FerrousPanel</strong> để thay thế trang mặc định này.'
          : 'Your virtual host is live! Upload your project files to the document root or trigger <strong>Git Pull &amp; Deploy</strong> in <strong>FerrousPanel</strong> to replace this welcome page.';
      }
      document.getElementById('langBtn').addEventListener('click', function () { apply(lang === 'vi' ? 'en' : 'vi'); });
      apply(lang);
    })();
  </script>
</body>
</html>
"##;

fn html_escape(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[must_use]
pub fn render_unconfigured_host_html() -> &'static str {
    UNCONFIGURED_HOST_HTML
}

#[must_use]
pub fn render_site_placeholder_html(domain: &str, runtime_label: &str) -> String {
    SITE_PLACEHOLDER_TEMPLATE
        .replace("__SITE_DOMAIN__", &html_escape(domain))
        .replace("__SITE_RUNTIME__", &html_escape(runtime_label))
}

fn should_update_default_index(path: &Path) -> bool {
    if !path.exists() {
        return true;
    }
    match std::fs::read_to_string(path) {
        Ok(content) => {
            content.trim().is_empty()
                || content.contains(DEFAULT_INDEX_MARKER)
                || content.contains("Welcome to nginx!")
        }
        Err(_) => false,
    }
}

/// Ensures `/www/server/default/index.html` and `/www/wwwroot/index.html` have the polished
/// bilingual landing page when running in production on a Linux VPS.
pub fn ensure_system_default_landing_pages(wwwroot: &Path) {
    let default_dir = Path::new("/www/server/default");
    if std::fs::create_dir_all(default_dir).is_ok() {
        let _ = atomic_write_file(
            &default_dir.join("index.html"),
            UNCONFIGURED_HOST_HTML.as_bytes(),
            0o644,
        );
    }
    if wwwroot.is_dir() {
        let root_index = wwwroot.join("index.html");
        if should_update_default_index(&root_index) {
            let _ = atomic_write_file(&root_index, UNCONFIGURED_HOST_HTML.as_bytes(), 0o644);
        }
    }
}

/// Writes a polished welcome `index.html` (or `index.php` for PHP sites) into a newly created site directory
/// ONLY if the directory currently contains no `index.html`, `index.htm`, or `index.php`.
pub fn ensure_new_site_welcome_index(site_dir: &Path, domain: &str, runtime_label: &str, is_php: bool) {
    if !site_dir.is_dir() {
        return;
    }
    for existing_index in ["index.html", "index.htm", "index.php", "default.php"] {
        if site_dir.join(existing_index).exists() {
            return;
        }
    }
    let html = render_site_placeholder_html(domain, runtime_label);
    let target_name = if is_php { "index.php" } else { "index.html" };
    let _ = atomic_write_file(&site_dir.join(target_name), html.as_bytes(), 0o644);
}
