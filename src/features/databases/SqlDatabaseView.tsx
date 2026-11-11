import React, { useState, useEffect } from 'react';
import {
  Check,
  CheckCircle2,
  Copy,
  Database,
  Download,
  ExternalLink,
  KeyRound,
  Lock,
  Play,
  Plus,
  RefreshCw,
  Save,
  Server,
  Settings,
  ShieldCheck,
  Sliders,
  Table2,
  Terminal,
  Trash2
} from 'lucide-react';
import {
  ManagedApp,
  MysqlPhpMyAdminConfig,
  SqlDatabase,
  SqlQueryResponse
} from '../types';

interface SqlDatabaseViewProps {
  databases: SqlDatabase[];
  apps: ManagedApp[];
  mysqlConfig: MysqlPhpMyAdminConfig | null;
  onCreateDatabase: (payload: {
    name: string;
    engine: SqlDatabase['engine'];
    username: string;
    password: string;
    collation: string;
    linkedAppId: string;
  }) => Promise<void>;
  onChangeDatabasePassword?: (id: string, password: string) => Promise<void>;
  onDeleteDatabase: (id: string) => Promise<void>;
  onExecuteQuery: (dbId: string, query: string) => Promise<SqlQueryResponse>;
  onUpdateMysqlConfig: (
    payload: Partial<MysqlPhpMyAdminConfig>
  ) => Promise<void>;
}

function generateStrongSqlPassword(): string {
  const chars =
    'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%^&*';
  let out = 'Sql#';
  for (let i = 0; i < 14; i++) {
    out += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return out + '!9';
}

export const SqlDatabaseView: React.FC<SqlDatabaseViewProps> = ({
  databases,
  apps,
  mysqlConfig,
  onCreateDatabase,
  onChangeDatabasePassword,
  onDeleteDatabase,
  onExecuteQuery,
  onUpdateMysqlConfig
}) => {
  const [selectedDbId, setSelectedDbId] = useState<string>(
    databases[0]?.id || ''
  );
  const activeDb =
    databases.find((d) => d.id === selectedDbId) || databases[0] || null;

  const [selectedTableName, setSelectedTableName] = useState<string>(
    activeDb?.tables[0]?.tableName || ''
  );
  const [subTab, setSubTab] = useState<
    'tables' | 'query' | 'phpmyadmin' | 'mysql_tuning' | 'create'
  >('phpmyadmin');

  // phpMyAdmin 5.2 internal workspace state
  const [pmaActiveTab, setPmaActiveTab] = useState<
    'databases' | 'structure' | 'browse' | 'sql' | 'new_table' | 'nginx_guard'
  >('databases');
  const [pmaQuickDbName, setPmaQuickDbName] = useState('');
  const [pmaQuickDbCollation, setPmaQuickDbCollation] = useState(
    'utf8mb4_unicode_ci'
  );
  const [pmaNewTableName, setPmaNewTableName] = useState('');
  const [pmaNewTableCol1, setPmaNewTableCol1] = useState('id');
  const [pmaNewTableCol2, setPmaNewTableCol2] = useState('title');
  const [pmaNewTableCol3, setPmaNewTableCol3] = useState('created_at');

  const [sqlInput, setSqlInput] = useState<string>('SHOW DATABASES;');
  const [queryResult, setQueryResult] = useState<SqlQueryResponse | null>(null);
  const [runningQuery, setRunningQuery] = useState<boolean>(false);
  const [copiedConn, setCopiedConn] = useState<boolean>(false);

  // Create New DB Form State (with Custom SQL Password)
  const [newDbName, setNewDbName] = useState('');
  const [newEngine, setNewEngine] =
    useState<SqlDatabase['engine']>('MariaDB 11.4');
  const [newUsername, setNewUsername] = useState('');
  const [newPassword, setNewPassword] = useState<string>(() =>
    generateStrongSqlPassword()
  );
  const [showNewPassword, setShowNewPassword] = useState<boolean>(true);
  const [newCollation, setNewCollation] = useState('utf8mb4_unicode_ci');
  const [newLinkedAppId, setNewLinkedAppId] = useState(apps[0]?.id || '');

  // Active DB Password Inspector & Rotation State
  const [showActivePassword, setShowActivePassword] = useState<boolean>(false);
  const [isRotatingPass, setIsRotatingPass] = useState<boolean>(false);
  const [rotatePasswordValue, setRotatePasswordValue] = useState<string>('');

  // MySQL / MariaDB & phpMyAdmin Config State
  const [innodbBufferPoolMb, setInnodbBufferPoolMb] = useState(
    mysqlConfig?.innodbBufferPoolMb ?? 1024
  );
  const [maxConnections, setMaxConnections] = useState(
    mysqlConfig?.maxConnections ?? 300
  );
  const [defaultCharset, setDefaultCharset] = useState(
    mysqlConfig?.defaultCharset ?? 'utf8mb4_unicode_ci'
  );
  const [slowQueryLog, setSlowQueryLog] = useState(
    mysqlConfig?.slowQueryLog ?? true
  );
  const [longQueryTimeSec, setLongQueryTimeSec] = useState(
    mysqlConfig?.longQueryTimeSec ?? 1.5
  );
  const [bindAddress, setBindAddress] = useState(
    mysqlConfig?.bindAddress ?? '127.0.0.1'
  );
  const [rawMyCnf, setRawMyCnf] = useState(mysqlConfig?.myCnfRaw ?? '');

  // phpMyAdmin Config State
  const [pmaPort, setPmaPort] = useState(mysqlConfig?.pmaPort ?? 888);
  const [pmaPath, setPmaPath] = useState(
    mysqlConfig?.pmaPath ?? '/phpmyadmin_ferrous_89a2'
  );
  const [pmaBasicAuth, setPmaBasicAuth] = useState(
    mysqlConfig?.pmaBasicAuth ?? true
  );
  const [pmaSslOnly, setPmaSslOnly] = useState(
    mysqlConfig?.pmaSslOnly ?? false
  );
  const [pmaUploadMaxMb, setPmaUploadMaxMb] = useState(
    mysqlConfig?.pmaMaxUploadMb ?? 256
  );
  const [pmaAllowedIps, setPmaAllowedIps] = useState(
    mysqlConfig?.pmaAllowedIps ?? '0.0.0.0/0 (Protected by Panel SSO)'
  );
  const [savingConfig, setSavingConfig] = useState(false);
  const [configSavedBanner, setConfigSavedBanner] = useState<string | null>(
    null
  );

  // Embedded phpMyAdmin quick row insert
  const [pmaQuickInsertToast, setPmaQuickInsertToast] = useState<string | null>(
    null
  );

  useEffect(() => {
    if (mysqlConfig) {
      setInnodbBufferPoolMb(mysqlConfig.innodbBufferPoolMb);
      setMaxConnections(mysqlConfig.maxConnections);
      setDefaultCharset(mysqlConfig.defaultCharset);
      setSlowQueryLog(mysqlConfig.slowQueryLog);
      setLongQueryTimeSec(mysqlConfig.longQueryTimeSec);
      setBindAddress(mysqlConfig.bindAddress);
      setRawMyCnf(mysqlConfig.myCnfRaw);
      setPmaPort(mysqlConfig.pmaPort);
      setPmaPath(mysqlConfig.pmaPath);
      setPmaBasicAuth(mysqlConfig.pmaBasicAuth);
      setPmaSslOnly(mysqlConfig.pmaSslOnly);
      setPmaUploadMaxMb(mysqlConfig.pmaMaxUploadMb);
      setPmaAllowedIps(mysqlConfig.pmaAllowedIps);
    }
  }, [mysqlConfig]);

  useEffect(() => {
    if (activeDb) {
      const firstTable = activeDb.tables[0]?.tableName || '';
      setSelectedTableName(firstTable);
      if (firstTable) {
        setSqlInput(`SELECT * FROM ${firstTable} LIMIT 25;`);
      }
      setQueryResult(null);
    }
  }, [activeDb?.id]);

  const activeTable =
    activeDb?.tables.find((t) => t.tableName === selectedTableName) ||
    activeDb?.tables[0] ||
    null;

  const getConnectionUri = (db: SqlDatabase, revealPassword = false) => {
    const pw = revealPassword
      ? db.password || 'Sql#2026!Default'
      : '••••••••••••';
    if (db.engine.includes('PostgreSQL')) {
      return `postgres://${db.username}:${pw}@${db.host}:${db.port}/${db.name}`;
    }
    if (db.engine.includes('MariaDB')) {
      return `mysql://${db.username}:${pw}@${db.host}:${db.port}/${db.name}`;
    }
    return `sqlite://${db.host}?mode=rwc`;
  };

  const handleCopyConnection = () => {
    if (!activeDb) return;
    navigator.clipboard.writeText(getConnectionUri(activeDb, true));
    setCopiedConn(true);
    setTimeout(() => setCopiedConn(false), 2000);
  };

  const handleExportSqlDump = () => {
    if (!activeDb) {
      setPmaQuickInsertToast(
        'Vui lòng tạo hoặc chọn ít nhất 1 Database trước khi xuất tệp .SQL'
      );
      setTimeout(() => setPmaQuickInsertToast(null), 3000);
      return;
    }
    const lines: string[] = [
      `-- FerrousPanel Rust Engine SQL Dump (${activeDb.engine})`,
      `-- Database: ${activeDb.name} · Host: ${activeDb.host}:${activeDb.port}`,
      `-- Generated At: ${new Date().toISOString()}`,
      ''
    ];
    for (const tbl of activeDb.tables) {
      const cols = tbl.columns
        .map(
          (c) =>
            `  ${c.name} ${c.dataType}${c.isPrimaryKey ? ' PRIMARY KEY' : ''}${
              !c.nullable ? ' NOT NULL' : ''
            }`
        )
        .join(',\n');
      lines.push(`CREATE TABLE IF NOT EXISTS ${tbl.tableName} (\n${cols}\n);\n`);
      for (const row of tbl.rows) {
        const colNames = Object.keys(row).join(', ');
        const vals = Object.values(row)
          .map((v) => (typeof v === 'string' ? `'${v}'` : String(v)))
          .join(', ');
        lines.push(
          `INSERT INTO ${tbl.tableName} (${colNames}) VALUES (${vals});`
        );
      }
      lines.push('');
    }
    const blob = new Blob([lines.join('\n')], {
      type: 'application/sql;charset=utf-8'
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${activeDb.name}_backup.sql`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleRunSql = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    if (!activeDb) {
      setQueryResult({
        success: true,
        executionTimeMs: 0.42,
        commandTag: 'SHOW DATABASES',
        affectedRows: 4 + databases.length,
        columns: ['Database', 'Engine', 'Collation'],
        rows: [
          {
            Database: 'information_schema',
            Engine: 'MEMORY / InnoDB',
            Collation: 'utf8mb3_general_ci'
          },
          {
            Database: 'mysql',
            Engine: 'InnoDB / Aria',
            Collation: 'utf8mb4_general_ci'
          },
          {
            Database: 'performance_schema',
            Engine: 'PERFORMANCE_SCHEMA',
            Collation: 'utf8mb3_general_ci'
          },
          {
            Database: 'sys',
            Engine: 'InnoDB',
            Collation: 'utf8mb3_general_ci'
          },
          ...databases.map((d) => ({
            Database: d.name,
            Engine: d.engine,
            Collation: d.collation
          }))
        ]
      });
      return;
    }
    setRunningQuery(true);
    try {
      const res = await onExecuteQuery(activeDb.id, sqlInput);
      setQueryResult(res);
    } finally {
      setRunningQuery(false);
    }
  };

  const handleCreateSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newDbName.trim()) return;
    const finalPassword =
      newPassword.trim() || generateStrongSqlPassword();
    await onCreateDatabase({
      name: newDbName.trim(),
      engine: newEngine,
      username: newUsername.trim() || `${newDbName.trim()}_user`,
      password: finalPassword,
      collation: newCollation,
      linkedAppId: newLinkedAppId || apps[0]?.id || ''
    });
    setConfigSavedBanner(
      `Đã tạo Database "${newDbName.trim()}" qua Rust DatabaseManager (src/database.rs) và đồng bộ vào phpMyAdmin 5.2!`
    );
    setTimeout(() => setConfigSavedBanner(null), 4500);
    setNewDbName('');
    setNewUsername('');
    setNewPassword(generateStrongSqlPassword());
    setSubTab('phpmyadmin');
    setPmaActiveTab('structure');
  };

  const handlePmaCreateQuickDatabase = async (e: React.FormEvent) => {
    e.preventDefault();
    const clean = pmaQuickDbName.trim().replace(/[^a-zA-Z0-9_]/g, '_');
    if (!clean) return;
    const autoUser = `${clean.slice(0, 16)}_usr`;
    const autoPass = generateStrongSqlPassword();
    await onCreateDatabase({
      name: clean,
      engine: 'MariaDB 11.4',
      username: autoUser,
      password: autoPass,
      collation: pmaQuickDbCollation,
      linkedAppId: apps[0]?.id || ''
    });
    setPmaQuickDbName('');
    setPmaQuickInsertToast(
      `[phpMyAdmin 5.2] CREATE DATABASE \`${clean}\` DEFAULT CHARACTER SET utf8mb4 COLLATE ${pmaQuickDbCollation}; (User: ${autoUser})`
    );
    setPmaActiveTab('structure');
    setTimeout(() => setPmaQuickInsertToast(null), 4500);
  };

  const handlePmaCreateTable = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeDb || !pmaNewTableName.trim()) return;
    const tName = pmaNewTableName.trim().replace(/[^a-zA-Z0-9_]/g, '_');
    const c1 = pmaNewTableCol1.trim() || 'id';
    const c2 = pmaNewTableCol2.trim() || 'title';
    const c3 = pmaNewTableCol3.trim() || 'created_at';
    const q = `CREATE TABLE ${tName} (${c1} INT PRIMARY KEY, ${c2} VARCHAR(255) NOT NULL, ${c3} TIMESTAMP);`;
    await onExecuteQuery(activeDb.id, q);
    setPmaNewTableName('');
    setSelectedTableName(tName);
    setPmaActiveTab('structure');
    setPmaQuickInsertToast(
      `[phpMyAdmin 5.2] Đã thực thi: ${q}`
    );
    setTimeout(() => setPmaQuickInsertToast(null), 4000);
  };

  const handleRotatePasswordSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeDb || !rotatePasswordValue.trim() || !onChangeDatabasePassword)
      return;
    await onChangeDatabasePassword(activeDb.id, rotatePasswordValue.trim());
    setIsRotatingPass(false);
    setRotatePasswordValue('');
    setConfigSavedBanner(
      `Đã đổi mật khẩu SQL cho user "${activeDb.username}" (ALTER USER trong src/database.rs) thành công!`
    );
    setTimeout(() => setConfigSavedBanner(null), 4000);
  };

  const handleSaveMysqlAndPma = async (e: React.FormEvent) => {
    e.preventDefault();
    setSavingConfig(true);
    setConfigSavedBanner(null);
    try {
      await onUpdateMysqlConfig({
        innodbBufferPoolMb,
        maxConnections,
        defaultCharset,
        slowQueryLog,
        longQueryTimeSec,
        bindAddress,
        myCnfRaw: rawMyCnf,
        pmaPort,
        pmaPath,
        pmaBasicAuth,
        pmaSslOnly,
        pmaMaxUploadMb: pmaUploadMaxMb,
        pmaAllowedIps
      });
      setConfigSavedBanner(
        'Đã ghi /etc/mysql/mariadb.conf.d/50-server.cnf & /etc/nginx/conf.d/ferrous-phpmyadmin.conf qua Rust Engine và reload dịch vụ thành công.'
      );
      setTimeout(() => setConfigSavedBanner(null), 4000);
    } finally {
      setSavingConfig(false);
    }
  };

  const handlePmaInsertSampleRow = async () => {
    if (!activeDb || !activeTable) return;
    const cols = activeTable.columns.map((c) => c.name).join(', ');
    const vals = activeTable.columns
      .map((c) =>
        c.dataType.includes('INT')
          ? Math.floor(1000 + Math.random() * 9000)
          : c.dataType.includes('BOOL')
            ? 'true'
            : `'row_${Math.floor(100 + Math.random() * 899)}'`
      )
      .join(', ');
    const q = `INSERT INTO ${activeTable.tableName} (${cols}) VALUES (${vals});`;
    await onExecuteQuery(activeDb.id, q);
    setPmaQuickInsertToast(
      `Đã chèn 1 bản ghi mới vào bảng ${activeDb.name}.${activeTable.tableName} qua phpMyAdmin 5.2`
    );
    setTimeout(() => setPmaQuickInsertToast(null), 3000);
  };

  const nginxPmaConfigPreview = `# /etc/nginx/conf.d/ferrous-phpmyadmin.conf (Sinh tự động bởi Ferrous-panel src/database.rs)
server {
    listen ${pmaPort}${pmaSslOnly ? ' ssl http2' : ''};
    server_name _;
    client_max_body_size ${pmaUploadMaxMb}M;
${
  pmaBasicAuth
    ? '    auth_basic "FerrousPanel phpMyAdmin Security Guard";\n    auth_basic_user_file /www/server/panel/vhost/htpasswd/phpmyadmin.htpasswd;\n'
    : ''
}
    location ${pmaPath} {
        alias /usr/share/phpmyadmin/;
        index index.php;

        location ~ \\.php$ {
            fastcgi_pass unix:/run/php/php8.3-fpm.sock;
            fastcgi_param SCRIPT_FILENAME $request_filename;
            include fastcgi_params;
        }
    }
}`;

  return (
    <div className="space-y-6">
      {/* Top Summary Header */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-4 sm:p-6 flex flex-col lg:flex-row lg:items-center justify-between gap-4 min-w-0 overflow-hidden">
        <div className="space-y-1 min-w-0">
          <div className="text-xs font-mono text-orange-600 dark:text-orange-400 flex flex-wrap items-center gap-x-2 gap-y-1">
            <span>Rust Core: Ferrous-panel/src/database.rs</span>
            <span>·</span>
            <span>MariaDB 11.4.4 LTS &amp; PostgreSQL 17.2</span>
            <span>·</span>
            <span>phpMyAdmin 5.2.1 (Port {pmaPort})</span>
          </div>
          <h1 className="text-base sm:text-lg font-bold text-slate-900 dark:text-slate-100">
            Quản trị Cơ sở Dữ liệu SQL, Cấu hình MySQL <code className="font-mono">my.cnf</code> &amp; phpMyAdmin 5.2
          </h1>
          <p className="text-xs text-slate-500 dark:text-slate-400 leading-relaxed">
            Xử lý trực tiếp bởi module Rust <code className="font-mono">DatabaseManager</code>: quản lý CSDL MariaDB/PostgreSQL, thực thi SQL, phpMyAdmin 5.2 SSO và Nginx Guard.
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2 w-full lg:w-auto">
          <button
            onClick={() => setSubTab('phpmyadmin')}
            className={`px-3.5 py-2 text-xs font-semibold rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
              subTab === 'phpmyadmin'
                ? 'bg-orange-600 text-white shadow-xs'
                : 'text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700'
            }`}
          >
            <ExternalLink className="w-3.5 h-3.5" />
            phpMyAdmin 5.2 (Port {pmaPort})
          </button>
          <button
            onClick={() => setSubTab('mysql_tuning')}
            className={`px-3.5 py-2 text-xs font-semibold rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
              subTab === 'mysql_tuning'
                ? 'bg-orange-600 text-white shadow-xs'
                : 'text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700'
            }`}
          >
            <Sliders className="w-3.5 h-3.5" />
            Cấu hình MySQL (my.cnf)
          </button>
          <button
            onClick={handleExportSqlDump}
            className="px-3.5 py-2 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
          >
            <Download className="w-3.5 h-3.5" />
            Xuất .SQL
          </button>
          <button
            onClick={() => setSubTab('create')}
            className="px-4 py-2 text-xs font-medium text-white bg-slate-900 dark:bg-orange-600 hover:bg-slate-800 dark:hover:bg-orange-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
          >
            <Plus className="w-3.5 h-3.5" />
            Tạo Database Mới
          </button>
        </div>
      </section>

      {configSavedBanner && (
        <div className="p-3.5 rounded-xl bg-emerald-50 dark:bg-emerald-950/40 border border-emerald-200 dark:border-emerald-800 text-xs font-medium text-emerald-800 dark:text-emerald-300 flex items-center justify-between">
          <span>{configSavedBanner}</span>
          <span className="font-mono text-[11px]">Rust DatabaseManager · systemctl reload mariadb nginx</span>
        </div>
      )}

      {/* Main Split: Left Database List & Right Inspector / phpMyAdmin / MySQL Tuning */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
        {/* Left: Databases List + MySQL Status Summary */}
        <div className="lg:col-span-4 space-y-4">
          <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 divide-y divide-slate-200 dark:divide-slate-800 overflow-hidden">
            <div className="px-5 py-3.5 bg-slate-50/70 dark:bg-slate-950/40 flex items-center justify-between text-xs font-semibold text-slate-600 dark:text-slate-400">
              <span>Databases Người Dùng ({databases.length})</span>
              <Database className="w-3.5 h-3.5" />
            </div>

            {databases.length === 0 ? (
              <div className="p-6 text-center text-slate-500 dark:text-slate-400 space-y-3">
                <Database className="w-8 h-8 mx-auto opacity-40 text-slate-400" />
                <div>
                  <div className="text-xs font-semibold text-slate-700 dark:text-slate-200">
                    Chưa có Database người dùng nào
                  </div>
                  <div className="text-[11px] mt-1 text-slate-400">
                    Bạn có thể tạo nhanh trong giao diện phpMyAdmin 5.2 bên phải hoặc bấm nút bên dưới.
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => setSubTab('create')}
                  className="px-3.5 py-1.5 text-xs font-semibold text-white bg-orange-600 hover:bg-orange-500 rounded-lg inline-flex items-center gap-1.5 cursor-pointer"
                >
                  <Plus className="w-3.5 h-3.5" />
                  Khởi Tạo Database Đầu Tiên
                </button>
              </div>
            ) : (
              databases.map((db) => {
                const isSelected = activeDb?.id === db.id;
                return (
                  <div
                    key={db.id}
                    onClick={() => {
                      setSelectedDbId(db.id);
                      if (subTab === 'create') setSubTab('tables');
                    }}
                    className={`p-4 transition-colors cursor-pointer ${
                      isSelected
                        ? 'bg-orange-50/60 dark:bg-orange-950/20'
                        : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
                    }`}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div className="space-y-0.5 min-w-0">
                        <div className="font-mono text-sm font-bold text-slate-900 dark:text-slate-100 truncate">
                          {db.name}
                        </div>
                        <div className="text-xs text-slate-500 dark:text-slate-400">
                          {db.engine} · {db.host}:{db.port}
                        </div>
                      </div>
                      <span className="text-xs font-mono tabular-nums text-emerald-600 dark:text-emerald-400 shrink-0 inline-flex items-center gap-1">
                        <CheckCircle2 className="w-3.5 h-3.5" />
                        {db.sizeMb} MB
                      </span>
                    </div>

                    <div className="mt-3 pt-2.5 border-t border-slate-100 dark:border-slate-800/70 flex items-center justify-between text-xs font-mono tabular-nums text-slate-500 dark:text-slate-400">
                      <span>User: {db.username}</span>
                      <span>
                        {db.tables.length} bảng · {db.activeConnections}/{db.maxConnections} conn
                      </span>
                    </div>
                  </div>
                );
              })
            )}
          </div>

          {/* Quick MySQL / phpMyAdmin Runtime Card */}
          <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-4 space-y-3 text-xs">
            <div className="flex items-center justify-between font-semibold text-slate-900 dark:text-white">
              <span>Trạng thái MariaDB &amp; phpMyAdmin 5.2</span>
              <span className="font-mono text-emerald-600 dark:text-emerald-400 inline-flex items-center gap-1">
                <CheckCircle2 className="w-3.5 h-3.5" />
                Active
              </span>
            </div>
            <div className="space-y-1.5 font-mono text-[11px] text-slate-600 dark:text-slate-400 tabular-nums">
              <div className="flex justify-between">
                <span>Engine:</span>
                <span className="text-slate-900 dark:text-slate-200 truncate max-w-[180px]">
                  {mysqlConfig?.mysqlVersion || 'MariaDB 11.4.4-LTS'}
                </span>
              </div>
              <div className="flex justify-between">
                <span>phpMyAdmin Version:</span>
                <span className="text-emerald-600 dark:text-emerald-400 font-semibold">
                  {mysqlConfig?.pmaVersion || '5.2.1-deb12u1'}
                </span>
              </div>
              <div className="flex justify-between">
                <span>InnoDB Buffer Pool:</span>
                <span className="text-slate-900 dark:text-slate-200">
                  {innodbBufferPoolMb} MB
                </span>
              </div>
              <div className="flex justify-between">
                <span>phpMyAdmin Endpoint:</span>
                <span className="text-orange-600 dark:text-orange-400">
                  :{pmaPort}
                  {pmaPath}
                </span>
              </div>
              <div className="flex justify-between">
                <span>FastCGI Upstream:</span>
                <span className="text-slate-900 dark:text-slate-200">
                  unix:/run/php/php8.3-fpm.sock
                </span>
              </div>
            </div>
            <div className="pt-2 border-t border-slate-100 dark:border-slate-800 flex gap-2">
              <button
                type="button"
                onClick={() => {
                  setSubTab('phpmyadmin');
                  setPmaActiveTab('databases');
                }}
                className="flex-1 py-1.5 px-2.5 text-center rounded-lg bg-orange-600/10 text-orange-600 dark:text-orange-400 font-semibold hover:bg-orange-600/20 transition-colors cursor-pointer"
              >
                Mở phpMyAdmin 5.2
              </button>
              <button
                type="button"
                onClick={() => setSubTab('mysql_tuning')}
                className="flex-1 py-1.5 px-2.5 text-center rounded-lg bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 font-medium hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors cursor-pointer"
              >
                Tệp my.cnf
              </button>
            </div>
          </div>
        </div>

        {/* Right: Database Schema Browser / SQL Query Console / phpMyAdmin 5.2 / MySQL Tuning / Create Form */}
        <div className="lg:col-span-8 space-y-6">
          {/* 1. CREATE NEW DATABASE FORM */}
          {subTab === 'create' && (
            <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-5">
              <div className="border-b border-slate-200 dark:border-slate-800 pb-4">
                <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
                  Khởi tạo Cơ sở Dữ liệu SQL &amp; Tài khoản Truy cập (Rust <code className="font-mono">src/database.rs</code>)
                </h2>
                <p className="text-xs text-slate-500 dark:text-slate-400">
                  Thực thi trực tiếp lệnh <code className="font-mono">CREATE DATABASE</code> &amp; <code className="font-mono">GRANT ALL PRIVILEGES</code> qua Rust <code className="font-mono">tokio::process::Command</code> và đồng bộ vào phpMyAdmin 5.2
                </p>
              </div>

              <form onSubmit={handleCreateSubmit} className="space-y-4">
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <div className="space-y-1.5">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      Tên Database (VD: app_production)
                    </label>
                    <input
                      type="text"
                      required
                      placeholder="app_production"
                      value={newDbName}
                      onChange={(e) => {
                        setNewDbName(e.target.value);
                        setNewUsername(
                          `${e.target.value.toLowerCase().replace(/[^a-z0-9_]/g, '_')}_usr`
                        );
                      }}
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                    />
                  </div>

                  <div className="space-y-1.5">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      Hệ quản trị CSDL (SQL Engine)
                    </label>
                    <select
                      value={newEngine}
                      onChange={(e) => {
                        const eng = e.target.value as SqlDatabase['engine'];
                        setNewEngine(eng);
                        setNewCollation(
                          eng.includes('MariaDB')
                            ? 'utf8mb4_unicode_ci'
                            : 'en_US.UTF-8'
                        );
                      }}
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                    >
                      <option value="MariaDB 11.4">
                        MariaDB 11.4 / MySQL (Hỗ trợ phpMyAdmin 5.2)
                      </option>
                      <option value="PostgreSQL 17">
                        PostgreSQL 17 (Khuyến nghị cho Rust SQLx / Actix-Web)
                      </option>
                      <option value="SQLite 3 (WAL)">
                        SQLite 3 (WAL Embedded)
                      </option>
                    </select>
                  </div>

                  <div className="space-y-1.5">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      Tên Role / User sở hữu
                    </label>
                    <input
                      type="text"
                      required
                      placeholder="app_production_usr"
                      value={newUsername}
                      onChange={(e) => setNewUsername(e.target.value)}
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                    />
                  </div>

                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                        Mật khẩu SQL Tùy chỉnh (Custom SQL Password)
                      </label>
                      <button
                        type="button"
                        onClick={() =>
                          setNewPassword(generateStrongSqlPassword())
                        }
                        className="text-[11px] font-mono text-orange-600 dark:text-orange-400 hover:underline cursor-pointer"
                      >
                        + Tạo mật khẩu ngẫu nhiên
                      </button>
                    </div>
                    <div className="flex items-center gap-2">
                      <input
                        type={showNewPassword ? 'text' : 'password'}
                        required
                        placeholder="Nhập mật khẩu SQL tùy chỉnh..."
                        value={newPassword}
                        onChange={(e) => setNewPassword(e.target.value)}
                        className="flex-1 px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                      />
                      <button
                        type="button"
                        onClick={() => setShowNewPassword((v) => !v)}
                        className="px-2.5 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 cursor-pointer shrink-0"
                      >
                        {showNewPassword ? 'Ẩn' : 'Hiện'}
                      </button>
                    </div>
                  </div>

                  <div className="space-y-1.5 sm:col-span-2">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      Gắn với Dịch vụ Web (Tự động chèn DB_PASSWORD &amp; DATABASE_URL vào tệp /.env)
                    </label>
                    <select
                      value={newLinkedAppId}
                      onChange={(e) => setNewLinkedAppId(e.target.value)}
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                    >
                      <option value="">-- Độc lập (Standalone SQL Database) --</option>
                      {apps.map((a) => (
                        <option key={a.id} value={a.id}>
                          {a.domain} ({a.framework})
                        </option>
                      ))}
                    </select>
                  </div>
                </div>

                <div className="pt-3 border-t border-slate-200 dark:border-slate-800 flex items-center justify-end gap-2">
                  <button
                    type="button"
                    onClick={() => setSubTab('phpmyadmin')}
                    className="px-4 py-2 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 rounded-lg cursor-pointer"
                  >
                    Quay lại phpMyAdmin
                  </button>
                  <button
                    type="submit"
                    className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg cursor-pointer"
                  >
                    Khởi tạo Database &amp; Ghi Mật khẩu
                  </button>
                </div>
              </form>
            </div>
          )}

          {/* 2. PHPMYADMIN 5.2.1 EMBEDDED WEB GUI & NGINX SECURITY CONFIGURATION (ALWAYS ACCESSIBLE) */}
          {subTab === 'phpmyadmin' && (
            <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden space-y-6 p-6">
              {/* phpMyAdmin 5.2.1 Full Interactive Frame */}
              <div className="rounded-xl border border-slate-300 dark:border-slate-700 overflow-hidden bg-slate-50 dark:bg-slate-950 shadow-sm">
                {/* phpMyAdmin Top Header Bar */}
                <div className="px-4 py-3 bg-slate-900 text-white flex flex-wrap items-center justify-between gap-2 text-xs border-b border-slate-800">
                  <div className="flex flex-wrap items-center gap-2 font-mono">
                    <span className="px-2 py-0.5 rounded bg-amber-500 text-slate-950 font-bold">
                      phpMyAdmin 5.2.1
                    </span>
                    <span className="text-slate-400">·</span>
                    <span className="inline-flex items-center gap-1 text-slate-200">
                      <Server className="w-3.5 h-3.5 text-orange-400" />
                      127.0.0.1:3306 (MariaDB 11.4.4-LTS)
                    </span>
                    <span className="text-slate-400">·</span>
                    <span className="text-emerald-400 font-semibold">
                      {activeDb ? `DB: ${activeDb.name}` : 'Server: root@localhost (SSO Active)'}
                    </span>
                  </div>
                  <div className="flex items-center gap-2">
                    {activeDb && (
                      <button
                        type="button"
                        onClick={handlePmaInsertSampleRow}
                        className="px-2.5 py-1 bg-amber-500 hover:bg-amber-400 text-slate-950 font-semibold rounded text-[11px] cursor-pointer"
                      >
                        + Chèn dòng (Insert Row)
                      </button>
                    )}
                    <button
                      type="button"
                      onClick={handleExportSqlDump}
                      className="px-2.5 py-1 bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 rounded text-[11px] cursor-pointer"
                    >
                      Export .SQL
                    </button>
                  </div>
                </div>

                {/* phpMyAdmin Navigation Tabs */}
                <div className="px-4 py-2 bg-slate-800/95 border-b border-slate-700 flex flex-wrap items-center gap-1.5 text-xs font-mono">
                  <button
                    type="button"
                    onClick={() => setPmaActiveTab('databases')}
                    className={`px-3 py-1.5 rounded transition-colors cursor-pointer ${
                      pmaActiveTab === 'databases'
                        ? 'bg-orange-600 text-white font-semibold'
                        : 'text-slate-300 hover:bg-slate-700'
                    }`}
                  >
                    Databases ({databases.length + 4})
                  </button>
                  <button
                    type="button"
                    onClick={() => setPmaActiveTab('structure')}
                    className={`px-3 py-1.5 rounded transition-colors cursor-pointer ${
                      pmaActiveTab === 'structure'
                        ? 'bg-orange-600 text-white font-semibold'
                        : 'text-slate-300 hover:bg-slate-700'
                    }`}
                  >
                    Structure ({activeDb ? activeDb.tables.length : 0} bảng)
                  </button>
                  <button
                    type="button"
                    onClick={() => setPmaActiveTab('browse')}
                    className={`px-3 py-1.5 rounded transition-colors cursor-pointer ${
                      pmaActiveTab === 'browse'
                        ? 'bg-orange-600 text-white font-semibold'
                        : 'text-slate-300 hover:bg-slate-700'
                    }`}
                  >
                    Browse (Duyệt dữ liệu)
                  </button>
                  <button
                    type="button"
                    onClick={() => setPmaActiveTab('sql')}
                    className={`px-3 py-1.5 rounded transition-colors cursor-pointer ${
                      pmaActiveTab === 'sql'
                        ? 'bg-orange-600 text-white font-semibold'
                        : 'text-slate-300 hover:bg-slate-700'
                    }`}
                  >
                    SQL Query
                  </button>
                  <button
                    type="button"
                    onClick={() => setPmaActiveTab('new_table')}
                    className={`px-3 py-1.5 rounded transition-colors cursor-pointer ${
                      pmaActiveTab === 'new_table'
                        ? 'bg-orange-600 text-white font-semibold'
                        : 'text-slate-300 hover:bg-slate-700'
                    }`}
                  >
                    + Tạo Bảng (New Table)
                  </button>
                  <button
                    type="button"
                    onClick={() => setPmaActiveTab('nginx_guard')}
                    className={`px-3 py-1.5 rounded transition-colors cursor-pointer ${
                      pmaActiveTab === 'nginx_guard'
                        ? 'bg-orange-600 text-white font-semibold'
                        : 'text-slate-300 hover:bg-slate-700'
                    }`}
                  >
                    Cấu hình Nginx (:888)
                  </button>
                </div>

                {pmaQuickInsertToast && (
                  <div className="px-4 py-2.5 bg-emerald-600 text-white text-xs font-mono flex items-center justify-between">
                    <span>{pmaQuickInsertToast}</span>
                    <button
                      type="button"
                      onClick={() => setPmaQuickInsertToast(null)}
                      className="text-white/80 hover:text-white text-[11px] underline cursor-pointer"
                    >
                      Đóng
                    </button>
                  </div>
                )}

                {/* phpMyAdmin Body Split: Left Schema Tree + Right Active Tab */}
                <div className="grid grid-cols-1 md:grid-cols-12 min-h-[360px]">
                  {/* Left phpMyAdmin Tree */}
                  <div className="md:col-span-4 border-b md:border-b-0 md:border-r border-slate-200 dark:border-slate-800 bg-slate-100/70 dark:bg-slate-900/60 p-3.5 space-y-3 text-xs font-mono">
                    <div className="flex items-center justify-between text-[11px] font-sans font-bold uppercase tracking-wider text-slate-500">
                      <span>Cây CSDL (Schema Tree)</span>
                      <button
                        type="button"
                        onClick={() => setPmaActiveTab('databases')}
                        className="text-orange-600 dark:text-orange-400 hover:underline font-mono lowercase cursor-pointer"
                      >
                        + mới
                      </button>
                    </div>

                    {/* User Databases in phpMyAdmin Tree */}
                    <div className="space-y-1">
                      <div className="text-[11px] text-slate-400 font-sans">
                        CSDL Người dùng ({databases.length}):
                      </div>
                      {databases.length === 0 ? (
                        <div className="p-2.5 rounded bg-white/70 dark:bg-slate-950/60 border border-dashed border-slate-300 dark:border-slate-800 text-[11px] text-slate-500 font-sans">
                          Chưa có CSDL người dùng. Tạo mới ở khung bên phải.
                        </div>
                      ) : (
                        databases.map((db) => (
                          <div key={db.id} className="space-y-1">
                            <button
                              type="button"
                              onClick={() => {
                                setSelectedDbId(db.id);
                                setPmaActiveTab('structure');
                              }}
                              className={`w-full text-left px-2.5 py-1.5 rounded flex items-center justify-between cursor-pointer ${
                                activeDb?.id === db.id
                                  ? 'bg-orange-600 text-white font-bold'
                                  : 'hover:bg-slate-200/70 dark:hover:bg-slate-800 text-slate-800 dark:text-slate-200'
                              }`}
                            >
                              <span className="truncate">🗄️ {db.name}</span>
                              <span className="text-[10px] opacity-80">
                                {db.tables.length} tbl
                              </span>
                            </button>
                            {activeDb?.id === db.id && db.tables.length > 0 && (
                              <div className="pl-4 space-y-0.5">
                                {db.tables.map((t) => (
                                  <button
                                    key={t.tableName}
                                    type="button"
                                    onClick={() => {
                                      setSelectedTableName(t.tableName);
                                      setPmaActiveTab('browse');
                                    }}
                                    className={`w-full text-left px-2 py-1 rounded text-[11px] flex items-center justify-between cursor-pointer ${
                                      activeTable?.tableName === t.tableName &&
                                      pmaActiveTab === 'browse'
                                        ? 'text-orange-600 dark:text-orange-400 font-bold bg-orange-500/10'
                                        : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
                                    }`}
                                  >
                                    <span>↳ {t.tableName}</span>
                                    <span>{t.rowCount}</span>
                                  </button>
                                ))}
                              </div>
                            )}
                          </div>
                        ))
                      )}
                    </div>

                    {/* System Schemas */}
                    <div className="pt-2 border-t border-slate-200 dark:border-slate-800 space-y-1">
                      <div className="text-[11px] text-slate-400 font-sans">
                        CSDL Hệ thống MariaDB:
                      </div>
                      {[
                        { name: 'information_schema', tables: 79 },
                        { name: 'mysql', tables: 31 },
                        { name: 'performance_schema', tables: 87 },
                        { name: 'sys', tables: 101 }
                      ].map((sysDb) => (
                        <div
                          key={sysDb.name}
                          className="px-2.5 py-1 text-[11px] text-slate-500 dark:text-slate-400 flex items-center justify-between"
                        >
                          <span>🔒 {sysDb.name}</span>
                          <span className="text-[10px]">{sysDb.tables} tbl</span>
                        </div>
                      ))}
                    </div>
                  </div>

                  {/* Right phpMyAdmin Active Workspace */}
                  <div className="md:col-span-8 p-4 space-y-4 bg-white dark:bg-slate-900">
                    {/* PMA TAB 1: DATABASES OVERVIEW & QUICK CREATE */}
                    {pmaActiveTab === 'databases' && (
                      <div className="space-y-4">
                        <form
                          onSubmit={handlePmaCreateQuickDatabase}
                          className="p-3.5 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 space-y-2.5"
                        >
                          <div className="text-xs font-bold text-slate-900 dark:text-white flex items-center gap-1.5">
                            <Plus className="w-3.5 h-3.5 text-orange-500" />
                            Tạo Cơ sở dữ liệu mới trực tiếp trong phpMyAdmin 5.2
                          </div>
                          <div className="flex flex-wrap items-center gap-2">
                            <input
                              type="text"
                              required
                              value={pmaQuickDbName}
                              onChange={(e) => setPmaQuickDbName(e.target.value)}
                              placeholder="Nhập tên CSDL (VD: shop_db)..."
                              className="flex-1 min-w-[160px] px-3 py-1.5 text-xs font-mono rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-white"
                            />
                            <select
                              value={pmaQuickDbCollation}
                              onChange={(e) =>
                                setPmaQuickDbCollation(e.target.value)
                              }
                              className="px-2.5 py-1.5 text-xs font-mono rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-white"
                            >
                              <option value="utf8mb4_unicode_ci">
                                utf8mb4_unicode_ci
                              </option>
                              <option value="utf8mb4_general_ci">
                                utf8mb4_general_ci
                              </option>
                              <option value="utf8mb4_bin">utf8mb4_bin</option>
                            </select>
                            <button
                              type="submit"
                              className="px-3.5 py-1.5 bg-orange-600 hover:bg-orange-500 text-white text-xs font-semibold rounded cursor-pointer"
                            >
                              Tạo (Create)
                            </button>
                          </div>
                        </form>

                        <div className="overflow-x-auto border border-slate-200 dark:border-slate-800 rounded-lg">
                          <table className="w-full text-left border-collapse text-xs font-mono">
                            <thead>
                              <tr className="border-b border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-950 text-slate-600 dark:text-slate-400">
                                <th className="py-2 px-3">Database</th>
                                <th className="py-2 px-3">Collation</th>
                                <th className="py-2 px-3">Số bảng</th>
                                <th className="py-2 px-3 text-right">Thao tác</th>
                              </tr>
                            </thead>
                            <tbody className="divide-y divide-slate-200 dark:divide-slate-800">
                              {databases.map((db) => (
                                <tr
                                  key={db.id}
                                  className="hover:bg-slate-50 dark:hover:bg-slate-800/50"
                                >
                                  <td className="py-2 px-3 font-bold text-orange-600 dark:text-orange-400">
                                    {db.name}
                                  </td>
                                  <td className="py-2 px-3 text-slate-500">
                                    {db.collation}
                                  </td>
                                  <td className="py-2 px-3">{db.tables.length}</td>
                                  <td className="py-2 px-3 text-right space-x-2">
                                    <button
                                      type="button"
                                      onClick={() => {
                                        setSelectedDbId(db.id);
                                        setPmaActiveTab('structure');
                                      }}
                                      className="text-sky-600 dark:text-sky-400 hover:underline cursor-pointer"
                                    >
                                      Mở cấu trúc
                                    </button>
                                    <button
                                      type="button"
                                      onClick={() => onDeleteDatabase(db.id)}
                                      className="text-rose-600 dark:text-rose-400 hover:underline cursor-pointer"
                                    >
                                      Drop
                                    </button>
                                  </td>
                                </tr>
                              ))}
                              {[
                                {
                                  name: 'information_schema',
                                  collation: 'utf8mb3_general_ci',
                                  tables: 79
                                },
                                {
                                  name: 'mysql',
                                  collation: 'utf8mb4_general_ci',
                                  tables: 31
                                },
                                {
                                  name: 'performance_schema',
                                  collation: 'utf8mb3_general_ci',
                                  tables: 87
                                }
                              ].map((sys) => (
                                <tr
                                  key={sys.name}
                                  className="text-slate-400 dark:text-slate-500"
                                >
                                  <td className="py-2 px-3">{sys.name}</td>
                                  <td className="py-2 px-3">{sys.collation}</td>
                                  <td className="py-2 px-3">{sys.tables}</td>
                                  <td className="py-2 px-3 text-right">
                                    System Schema
                                  </td>
                                </tr>
                              ))}
                            </tbody>
                          </table>
                        </div>
                      </div>
                    )}

                    {/* PMA TAB 2: TABLE STRUCTURE */}
                    {pmaActiveTab === 'structure' && (
                      <div className="space-y-4">
                        {!activeDb ? (
                          <div className="p-6 text-center border border-dashed border-slate-300 dark:border-slate-800 rounded-lg space-y-2">
                            <div className="text-xs font-semibold text-slate-700 dark:text-slate-200">
                              Chưa chọn Database người dùng
                            </div>
                            <p className="text-[11px] text-slate-500">
                              Hãy tạo một Database mới trong tab &quot;Databases&quot; để bắt đầu tạo bảng.
                            </p>
                            <button
                              type="button"
                              onClick={() => setPmaActiveTab('databases')}
                              className="px-3 py-1.5 bg-orange-600 text-white text-xs rounded cursor-pointer"
                            >
                              Chuyển tới Tạo Database
                            </button>
                          </div>
                        ) : (
                          <>
                            <div className="flex items-center justify-between">
                              <div className="text-xs font-mono font-bold text-slate-900 dark:text-white">
                                Danh sách bảng trong `{activeDb.name}` ({activeDb.tables.length})
                              </div>
                              <button
                                type="button"
                                onClick={() => setPmaActiveTab('new_table')}
                                className="px-2.5 py-1 bg-orange-600 hover:bg-orange-500 text-white text-[11px] font-semibold rounded cursor-pointer"
                              >
                                + Tạo bảng mới
                              </button>
                            </div>
                            <div className="overflow-x-auto border border-slate-200 dark:border-slate-800 rounded-lg">
                              <table className="w-full text-left border-collapse text-xs font-mono">
                                <thead>
                                  <tr className="border-b border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-950 text-slate-600 dark:text-slate-400">
                                    <th className="py-2.5 px-3">Bảng (Table)</th>
                                    <th className="py-2.5 px-3">Bản ghi (Rows)</th>
                                    <th className="py-2.5 px-3">Engine</th>
                                    <th className="py-2.5 px-3">Kích thước</th>
                                    <th className="py-2.5 px-3 text-right">Thao tác phpMyAdmin</th>
                                  </tr>
                                </thead>
                                <tbody className="divide-y divide-slate-200 dark:divide-slate-800 tabular-nums">
                                  {activeDb.tables.map((tbl) => (
                                    <tr
                                      key={tbl.tableName}
                                      className="hover:bg-slate-50 dark:hover:bg-slate-800/50"
                                    >
                                      <td className="py-2.5 px-3 font-bold text-slate-900 dark:text-white">
                                        {tbl.tableName}
                                      </td>
                                      <td className="py-2.5 px-3">
                                        {tbl.rowCount.toLocaleString()}
                                      </td>
                                      <td className="py-2.5 px-3">InnoDB</td>
                                      <td className="py-2.5 px-3">{tbl.sizeKb} KB</td>
                                      <td className="py-2.5 px-3 text-right space-x-2">
                                        <button
                                          type="button"
                                          onClick={() => {
                                            setSelectedTableName(tbl.tableName);
                                            setPmaActiveTab('browse');
                                          }}
                                          className="text-sky-600 dark:text-sky-400 hover:underline cursor-pointer"
                                        >
                                          Browse
                                        </button>
                                        <button
                                          type="button"
                                          onClick={() => {
                                            setSqlInput(
                                              `SELECT * FROM ${tbl.tableName} LIMIT 25;`
                                            );
                                            setPmaActiveTab('sql');
                                          }}
                                          className="text-emerald-600 dark:text-emerald-400 hover:underline cursor-pointer"
                                        >
                                          SQL
                                        </button>
                                      </td>
                                    </tr>
                                  ))}
                                </tbody>
                              </table>
                            </div>
                          </>
                        )}
                      </div>
                    )}

                    {/* PMA TAB 3: BROWSE ROWS */}
                    {pmaActiveTab === 'browse' && (
                      <div className="space-y-3">
                        {!activeDb || !activeTable ? (
                          <div className="p-6 text-center text-xs text-slate-500">
                            Hãy chọn hoặc tạo một Database và Bảng để duyệt dữ liệu.
                          </div>
                        ) : (
                          <>
                            <div className="flex items-center justify-between">
                              <div className="text-xs font-mono text-slate-700 dark:text-slate-300">
                                <span className="text-emerald-600 dark:text-emerald-400 font-bold">
                                  SELECT * FROM `{activeDb.name}`.`{activeTable.tableName}`
                                </span>{' '}
                                ({activeTable.rows.length} dòng)
                              </div>
                              <button
                                type="button"
                                onClick={handlePmaInsertSampleRow}
                                className="px-2.5 py-1 bg-orange-600 hover:bg-orange-500 text-white text-[11px] font-semibold rounded cursor-pointer"
                              >
                                + Chèn dòng mới
                              </button>
                            </div>
                            <div className="overflow-x-auto border border-slate-200 dark:border-slate-800 rounded-lg">
                              <table className="w-full text-left border-collapse text-xs font-mono">
                                <thead>
                                  <tr className="border-b border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-950 text-slate-600 dark:text-slate-400">
                                    {activeTable.columns.map((col) => (
                                      <th key={col.name} className="py-2 px-3">
                                        {col.name}{' '}
                                        <span className="text-[10px] opacity-70">
                                          ({col.dataType})
                                        </span>
                                      </th>
                                    ))}
                                  </tr>
                                </thead>
                                <tbody className="divide-y divide-slate-200 dark:divide-slate-800">
                                  {activeTable.rows.length === 0 ? (
                                    <tr>
                                      <td
                                        colSpan={activeTable.columns.length || 1}
                                        className="py-6 text-center text-slate-400"
                                      >
                                        Bảng chưa có dữ liệu. Nhấn &quot;+ Chèn dòng mới&quot; ở trên.
                                      </td>
                                    </tr>
                                  ) : (
                                    activeTable.rows.map((row, idx) => (
                                      <tr key={idx}>
                                        {activeTable.columns.map((col) => (
                                          <td
                                            key={col.name}
                                            className="py-2 px-3 text-slate-800 dark:text-slate-200"
                                          >
                                            {String(row[col.name] ?? 'NULL')}
                                          </td>
                                        ))}
                                      </tr>
                                    ))
                                  )}
                                </tbody>
                              </table>
                            </div>
                          </>
                        )}
                      </div>
                    )}

                    {/* PMA TAB 4: SQL CONSOLE INSIDE PHPMYADMIN */}
                    {pmaActiveTab === 'sql' && (
                      <div className="space-y-3">
                        <div className="flex flex-wrap items-center justify-between gap-2">
                          <div className="flex items-center gap-1.5 text-xs font-mono">
                            <button
                              type="button"
                              onClick={() => setSqlInput('SHOW TABLES;')}
                              className="px-2 py-1 bg-slate-100 dark:bg-slate-800 rounded cursor-pointer"
                            >
                              SHOW TABLES
                            </button>
                            {activeTable && (
                              <button
                                type="button"
                                onClick={() =>
                                  setSqlInput(
                                    `SELECT * FROM ${activeTable.tableName} LIMIT 25;`
                                  )
                                }
                                className="px-2 py-1 bg-slate-100 dark:bg-slate-800 rounded cursor-pointer"
                              >
                                SELECT *
                              </button>
                            )}
                          </div>
                          <button
                            type="button"
                            onClick={() => handleRunSql()}
                            disabled={runningQuery}
                            className="px-3.5 py-1.5 bg-orange-600 hover:bg-orange-500 text-white text-xs font-semibold rounded inline-flex items-center gap-1 cursor-pointer"
                          >
                            <Play className="w-3 h-3" />
                            {runningQuery ? 'Đang chạy...' : 'Thực thi (Go)'}
                          </button>
                        </div>

                        <textarea
                          rows={4}
                          spellCheck={false}
                          value={sqlInput}
                          onChange={(e) => setSqlInput(e.target.value)}
                          className="w-full p-3 rounded-lg bg-slate-950 text-slate-100 font-mono text-xs border border-slate-800"
                        />

                        {queryResult && (
                          <div className="space-y-2 pt-1">
                            <div className="text-xs font-mono flex items-center justify-between">
                              <span
                                className={
                                  queryResult.success
                                    ? 'text-emerald-600 dark:text-emerald-400 font-semibold'
                                    : 'text-rose-600 font-semibold'
                                }
                              >
                                {queryResult.commandTag} · {queryResult.affectedRows} dòng
                              </span>
                              <span className="text-slate-400">
                                {queryResult.executionTimeMs} ms
                              </span>
                            </div>
                            {queryResult.error ? (
                              <div className="p-3 rounded bg-rose-950/40 border border-rose-800 text-rose-200 font-mono text-xs">
                                {queryResult.error}
                              </div>
                            ) : (
                              queryResult.columns.length > 0 && (
                                <div className="overflow-x-auto border border-slate-200 dark:border-slate-800 rounded-lg">
                                  <table className="w-full text-left border-collapse text-xs font-mono">
                                    <thead>
                                      <tr className="border-b border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-950">
                                        {queryResult.columns.map((c) => (
                                          <th key={c} className="py-2 px-3">
                                            {c}
                                          </th>
                                        ))}
                                      </tr>
                                    </thead>
                                    <tbody className="divide-y divide-slate-200 dark:divide-slate-800">
                                      {queryResult.rows.map((r, i) => (
                                        <tr key={i}>
                                          {queryResult.columns.map((c) => (
                                            <td key={c} className="py-2 px-3">
                                              {String(r[c] ?? 'NULL')}
                                            </td>
                                          ))}
                                        </tr>
                                      ))}
                                    </tbody>
                                  </table>
                                </div>
                              )
                            )}
                          </div>
                        )}
                      </div>
                    )}

                    {/* PMA TAB 5: CREATE NEW TABLE */}
                    {pmaActiveTab === 'new_table' && (
                      <div className="space-y-4">
                        {!activeDb ? (
                          <div className="p-6 text-center text-xs text-slate-500">
                            Vui lòng tạo hoặc chọn một Database trước khi tạo bảng mới.
                          </div>
                        ) : (
                          <form onSubmit={handlePmaCreateTable} className="space-y-3">
                            <div className="text-xs font-bold text-slate-900 dark:text-white">
                              Tạo bảng mới trong Database `{activeDb.name}`
                            </div>
                            <div className="space-y-1">
                              <label className="block text-xs text-slate-600 dark:text-slate-400">
                                Tên Bảng (Table Name)
                              </label>
                              <input
                                type="text"
                                required
                                value={pmaNewTableName}
                                onChange={(e) => setPmaNewTableName(e.target.value)}
                                placeholder="VD: products, invoices, logs..."
                                className="w-full px-3 py-1.5 text-xs font-mono rounded border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                              />
                            </div>
                            <div className="grid grid-cols-3 gap-2">
                              <div>
                                <label className="block text-[11px] text-slate-500">
                                  Cột 1 (INT PK)
                                </label>
                                <input
                                  type="text"
                                  value={pmaNewTableCol1}
                                  onChange={(e) => setPmaNewTableCol1(e.target.value)}
                                  className="w-full px-2.5 py-1.5 text-xs font-mono rounded border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                                />
                              </div>
                              <div>
                                <label className="block text-[11px] text-slate-500">
                                  Cột 2 (VARCHAR)
                                </label>
                                <input
                                  type="text"
                                  value={pmaNewTableCol2}
                                  onChange={(e) => setPmaNewTableCol2(e.target.value)}
                                  className="w-full px-2.5 py-1.5 text-xs font-mono rounded border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                                />
                              </div>
                              <div>
                                <label className="block text-[11px] text-slate-500">
                                  Cột 3 (TIMESTAMP)
                                </label>
                                <input
                                  type="text"
                                  value={pmaNewTableCol3}
                                  onChange={(e) => setPmaNewTableCol3(e.target.value)}
                                  className="w-full px-2.5 py-1.5 text-xs font-mono rounded border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                                />
                              </div>
                            </div>
                            <button
                              type="submit"
                              className="px-4 py-1.5 bg-orange-600 hover:bg-orange-500 text-white text-xs font-semibold rounded cursor-pointer"
                            >
                              Tạo Bảng (CREATE TABLE)
                            </button>
                          </form>
                        )}
                      </div>
                    )}

                    {/* PMA TAB 6: NGINX VHOST PREVIEW */}
                    {pmaActiveTab === 'nginx_guard' && (
                      <div className="space-y-2">
                        <div className="text-xs font-semibold text-slate-800 dark:text-slate-200 flex items-center gap-1.5">
                          <ShieldCheck className="w-4 h-4 text-emerald-500" />
                          Cấu hình Nginx Vhost bảo vệ phpMyAdmin (`/etc/nginx/conf.d/ferrous-phpmyadmin.conf`)
                        </div>
                        <pre className="p-3.5 rounded-lg bg-slate-950 text-slate-200 font-mono text-[11px] overflow-x-auto leading-relaxed border border-slate-800">
                          {nginxPmaConfigPreview}
                        </pre>
                      </div>
                    )}
                  </div>
                </div>
              </div>

              {/* phpMyAdmin Security & Port Configuration Form */}
              <form
                onSubmit={handleSaveMysqlAndPma}
                className="space-y-4 pt-2 border-t border-slate-200 dark:border-slate-800"
              >
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div>
                    <h3 className="text-sm font-bold text-slate-900 dark:text-white flex items-center gap-2">
                      <Lock className="w-4 h-4 text-orange-600 dark:text-orange-400" />
                      Cấu hình Bảo mật &amp; Cổng Truy cập phpMyAdmin (Rust <code className="font-mono">DatabaseManager::apply_mysql_and_phpmyadmin_config</code>)
                    </h3>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      Đổi đường dẫn URI bí mật và bật xác thực 2 lớp Nginx Basic Auth để chống bot dò quét tự động.
                    </p>
                  </div>
                  <button
                    type="submit"
                    disabled={savingConfig}
                    className="px-4 py-2 bg-orange-600 hover:bg-orange-500 text-white text-xs font-semibold rounded-lg inline-flex items-center gap-1.5 cursor-pointer"
                  >
                    <Save className="w-3.5 h-3.5" />
                    {savingConfig ? 'Đang lưu...' : 'Lưu Cấu hình phpMyAdmin'}
                  </button>
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
                  <div className="space-y-1">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      Cổng lắng nghe (Nginx Port)
                    </label>
                    <input
                      type="number"
                      value={pmaPort}
                      onChange={(e) => setPmaPort(Number(e.target.value))}
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
                    />
                  </div>

                  <div className="space-y-1">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      Đường dẫn Bảo mật (Secret URI)
                    </label>
                    <input
                      type="text"
                      value={pmaPath}
                      onChange={(e) => setPmaPath(e.target.value)}
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
                    />
                  </div>

                  <div className="space-y-1">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      Giới hạn Import .SQL (Upload MB)
                    </label>
                    <input
                      type="number"
                      value={pmaUploadMaxMb}
                      onChange={(e) => setPmaUploadMaxMb(Number(e.target.value))}
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
                    />
                  </div>

                  <div className="space-y-1">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      Dải IP Cho phép (Allowed IPs)
                    </label>
                    <input
                      type="text"
                      value={pmaAllowedIps}
                      onChange={(e) => setPmaAllowedIps(e.target.value)}
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
                    />
                  </div>
                </div>

                <div className="flex flex-wrap items-center gap-6 pt-2 text-xs">
                  <label className="inline-flex items-center gap-2 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={pmaBasicAuth}
                      onChange={(e) => setPmaBasicAuth(e.target.checked)}
                      className="w-4 h-4 accent-orange-600"
                    />
                    <span>
                      Bật khóa mật khẩu tầng 1 (Nginx HTTP Basic Auth trước khi vào phpMyAdmin)
                    </span>
                  </label>
                  <label className="inline-flex items-center gap-2 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={pmaSslOnly}
                      onChange={(e) => setPmaSslOnly(e.target.checked)}
                      className="w-4 h-4 accent-orange-600"
                    />
                    <span>Bắt buộc kết nối qua HTTPS TLS 1.3</span>
                  </label>
                </div>
              </form>
            </div>
          )}

          {/* 3. MYSQL / MARIADB my.cnf PERFORMANCE TUNING (ALWAYS ACCESSIBLE) */}
          {subTab === 'mysql_tuning' && (
            <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
              <form onSubmit={handleSaveMysqlAndPma} className="p-6 space-y-5">
                <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
                  <div>
                    <h3 className="text-sm font-bold text-slate-900 dark:text-white">
                      Tinh chỉnh Hiệu năng MySQL / MariaDB (
                      <code className="font-mono">
                        /etc/mysql/mariadb.conf.d/50-server.cnf
                      </code>
                      )
                    </h3>
                    <p className="text-xs text-slate-500 dark:text-slate-400">
                      Tối ưu bộ đệm RAM InnoDB, giới hạn kết nối đồng thời và nhật ký truy vấn chậm (Slow Query Log) qua Rust Engine.
                    </p>
                  </div>
                  <button
                    type="submit"
                    disabled={savingConfig}
                    className="px-4 py-2 bg-orange-600 hover:bg-orange-500 text-white text-xs font-semibold rounded-lg inline-flex items-center gap-1.5 shrink-0 cursor-pointer"
                  >
                    <RefreshCw
                      className={`w-3.5 h-3.5 ${savingConfig ? 'animate-spin' : ''}`}
                    />
                    {savingConfig ? 'Đang áp dụng...' : 'Lưu & Reload MariaDB'}
                  </button>
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
                  <div className="space-y-1">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      innodb_buffer_pool_size (MB)
                    </label>
                    <input
                      type="number"
                      step={256}
                      value={innodbBufferPoolMb}
                      onChange={(e) =>
                        setInnodbBufferPoolMb(Number(e.target.value))
                      }
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
                    />
                  </div>

                  <div className="space-y-1">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      max_connections (Kết nối tối đa)
                    </label>
                    <input
                      type="number"
                      step={50}
                      value={maxConnections}
                      onChange={(e) =>
                        setMaxConnections(Number(e.target.value))
                      }
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
                    />
                  </div>

                  <div className="space-y-1">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      character-set / collation
                    </label>
                    <input
                      type="text"
                      value={defaultCharset}
                      onChange={(e) => setDefaultCharset(e.target.value)}
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
                    />
                  </div>

                  <div className="space-y-1">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      Cổng Nội bộ (MySQL Port)
                    </label>
                    <input
                      type="number"
                      readOnly
                      value={mysqlConfig?.port ?? 3306}
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-950"
                    />
                  </div>

                  <div className="space-y-1">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      long_query_time (Giây cảnh báo chậm)
                    </label>
                    <input
                      type="number"
                      step={0.5}
                      value={longQueryTimeSec}
                      onChange={(e) =>
                        setLongQueryTimeSec(Number(e.target.value))
                      }
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
                    />
                  </div>

                  <div className="space-y-1">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      bind-address (Cô lập Mạng)
                    </label>
                    <input
                      type="text"
                      value={bindAddress}
                      onChange={(e) => setBindAddress(e.target.value)}
                      className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950"
                    />
                  </div>
                </div>

                <div className="space-y-1.5">
                  <div className="flex items-center justify-between text-xs">
                    <span className="font-semibold text-slate-700 dark:text-slate-300">
                      Tệp cấu hình trực tiếp: /etc/mysql/mariadb.conf.d/50-server.cnf
                    </span>
                    <label className="inline-flex items-center gap-2 cursor-pointer">
                      <input
                        type="checkbox"
                        checked={slowQueryLog}
                        onChange={(e) => setSlowQueryLog(e.target.checked)}
                        className="w-3.5 h-3.5 accent-orange-600"
                      />
                      <span>Ghi nhật ký truy vấn chậm (slow_query_log = 1)</span>
                    </label>
                  </div>
                  <textarea
                    rows={10}
                    spellCheck={false}
                    value={rawMyCnf}
                    onChange={(e) => setRawMyCnf(e.target.value)}
                    className="w-full p-4 rounded-lg bg-slate-950 text-slate-100 font-mono text-xs leading-relaxed border border-slate-800"
                  />
                </div>
              </form>
            </div>
          )}

          {/* 4. ACTIVE DB SCHEMA & SQL STUDIO (OR EMPTY STATE IF NO DB SELECTED) */}
          {(subTab === 'tables' || subTab === 'query') &&
            (!activeDb ? (
              <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-10 text-center space-y-4">
                <Database className="w-10 h-10 mx-auto text-orange-500 opacity-75" />
                <div className="space-y-1">
                  <h3 className="text-sm font-bold text-slate-900 dark:text-white">
                    Chưa có Database nào được chọn
                  </h3>
                  <p className="text-xs text-slate-500 dark:text-slate-400 max-w-md mx-auto">
                    Khởi tạo một Database MariaDB 11.4 hoặc PostgreSQL 17 mới, hoặc sử dụng trực tiếp trình quản trị phpMyAdmin 5.2.
                  </p>
                </div>
                <div className="flex flex-wrap items-center justify-center gap-3">
                  <button
                    type="button"
                    onClick={() => setSubTab('create')}
                    className="px-4 py-2 text-xs font-semibold text-white bg-orange-600 hover:bg-orange-500 rounded-lg inline-flex items-center gap-1.5 cursor-pointer"
                  >
                    <Plus className="w-3.5 h-3.5" />
                    Tạo Database Mới
                  </button>
                  <button
                    type="button"
                    onClick={() => setSubTab('phpmyadmin')}
                    className="px-4 py-2 text-xs font-semibold text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg inline-flex items-center gap-1.5 cursor-pointer"
                  >
                    <ExternalLink className="w-3.5 h-3.5 text-orange-500" />
                    Mở phpMyAdmin 5.2
                  </button>
                </div>
              </div>
            ) : (
              <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
                {/* Active DB Header & Sub-tab Switcher */}
                <div className="p-6 border-b border-slate-200 dark:border-slate-800 space-y-4">
                  <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                    <div>
                      <div className="text-xs font-mono text-slate-500 dark:text-slate-400">
                        {activeDb.engine} · Collation: {activeDb.collation} · Dịch vụ:{' '}
                        {activeDb.linkedAppName}
                      </div>
                      <h2 className="text-lg font-bold font-mono text-slate-900 dark:text-slate-100">
                        {activeDb.name}
                      </h2>
                    </div>

                    <div className="flex flex-wrap items-center gap-2">
                      <div className="flex flex-wrap items-center gap-1 p-1 bg-slate-100 dark:bg-slate-950 rounded-lg">
                        <button
                          onClick={() => setSubTab('tables')}
                          className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
                            subTab === 'tables'
                              ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100 shadow-xs'
                              : 'text-slate-600 dark:text-slate-400'
                          }`}
                        >
                          <Table2 className="w-3.5 h-3.5" />
                          Duyệt Bảng ({activeDb.tables.length})
                        </button>
                        <button
                          onClick={() => setSubTab('query')}
                          className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
                            subTab === 'query'
                              ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100 shadow-xs'
                              : 'text-slate-600 dark:text-slate-400'
                          }`}
                        >
                          <Terminal className="w-3.5 h-3.5" />
                          SQL Studio
                        </button>
                        <button
                          onClick={() => setSubTab('phpmyadmin')}
                          className="px-3 py-1.5 text-xs font-medium rounded-md transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer text-slate-600 dark:text-slate-400"
                        >
                          <Settings className="w-3.5 h-3.5" />
                          phpMyAdmin 5.2
                        </button>
                        <button
                          onClick={() => setSubTab('mysql_tuning')}
                          className="px-3 py-1.5 text-xs font-medium rounded-md transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer text-slate-600 dark:text-slate-400"
                        >
                          <Sliders className="w-3.5 h-3.5" />
                          MySQL my.cnf
                        </button>
                      </div>

                      <button
                        onClick={() => onDeleteDatabase(activeDb.id)}
                        className="p-2 text-slate-400 hover:text-rose-600 dark:hover:text-rose-400 rounded-lg transition-colors cursor-pointer"
                        title="Xóa Database"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  </div>

                  {/* Connection URI & Custom Password Management Strip */}
                  <div className="px-3.5 py-2.5 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 space-y-2.5">
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <div className="flex items-center gap-2 min-w-0 text-xs font-mono text-slate-700 dark:text-slate-300 truncate">
                        <KeyRound className="w-3.5 h-3.5 text-orange-600 dark:text-orange-400 shrink-0" />
                        <span className="truncate">
                          {getConnectionUri(activeDb, showActivePassword)}
                        </span>
                      </div>
                      <div className="flex items-center gap-2 shrink-0">
                        <button
                          type="button"
                          onClick={() => setShowActivePassword((v) => !v)}
                          className="px-2.5 py-1 text-xs font-mono text-slate-700 dark:text-slate-200 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-md cursor-pointer"
                        >
                          {showActivePassword ? 'Ẩn mật khẩu' : 'Hiện mật khẩu'}
                        </button>
                        <button
                          type="button"
                          onClick={() => {
                            setIsRotatingPass((v) => !v);
                            setRotatePasswordValue(
                              activeDb.password || generateStrongSqlPassword()
                            );
                          }}
                          className="px-2.5 py-1 text-xs font-medium text-orange-600 dark:text-orange-400 bg-white dark:bg-slate-800 border border-orange-500/30 rounded-md cursor-pointer inline-flex items-center gap-1"
                        >
                          <Lock className="w-3 h-3" />
                          Đổi mật khẩu SQL
                        </button>
                        <button
                          onClick={handleCopyConnection}
                          className="px-2.5 py-1 text-xs font-medium text-slate-700 dark:text-slate-200 bg-white dark:bg-slate-800 hover:bg-slate-100 dark:hover:bg-slate-700 border border-slate-200 dark:border-slate-700 rounded-md transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1 cursor-pointer"
                        >
                          {copiedConn ? (
                            <>
                              <Check className="w-3 h-3 text-emerald-500" />
                              Đã chép URI
                            </>
                          ) : (
                            <>
                              <Copy className="w-3 h-3" />
                              Chép URI
                            </>
                          )}
                        </button>
                      </div>
                    </div>

                    {isRotatingPass && (
                      <form
                        onSubmit={handleRotatePasswordSubmit}
                        className="pt-2 border-t border-slate-200 dark:border-slate-800 flex flex-wrap items-center gap-2"
                      >
                        <span className="text-xs font-mono text-slate-500">
                          ALTER USER &apos;{activeDb.username}&apos; IDENTIFIED BY:
                        </span>
                        <input
                          type="text"
                          required
                          value={rotatePasswordValue}
                          onChange={(e) =>
                            setRotatePasswordValue(e.target.value)
                          }
                          placeholder="Nhập mật khẩu SQL mới..."
                          className="flex-1 min-w-[180px] px-2.5 py-1 text-xs font-mono rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100"
                        />
                        <button
                          type="button"
                          onClick={() =>
                            setRotatePasswordValue(generateStrongSqlPassword())
                          }
                          className="px-2.5 py-1 text-xs font-mono text-slate-600 dark:text-slate-300 bg-slate-200 dark:bg-slate-800 rounded cursor-pointer"
                        >
                          Ngẫu nhiên
                        </button>
                        <button
                          type="submit"
                          className="px-3 py-1 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded cursor-pointer"
                        >
                          Lưu &amp; Đồng bộ .env
                        </button>
                      </form>
                    )}
                  </div>
                </div>

                {/* SUB-TAB 1: Table Schema & Row Data Browser */}
                {subTab === 'tables' && (
                  <div className="p-6 space-y-6">
                    <div className="flex flex-wrap items-center gap-2">
                      {activeDb.tables.map((t) => (
                        <button
                          key={t.tableName}
                          onClick={() => setSelectedTableName(t.tableName)}
                          className={`px-3 py-1.5 text-xs font-mono rounded-lg border transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                            activeTable?.tableName === t.tableName
                              ? 'bg-orange-600 text-white border-orange-600'
                              : 'bg-slate-50 dark:bg-slate-950 border-slate-200 dark:border-slate-800 text-slate-700 dark:text-slate-300 hover:border-slate-300'
                          }`}
                        >
                          {t.tableName} ({t.rowCount.toLocaleString()} dòng · {t.sizeKb} KB)
                        </button>
                      ))}
                    </div>

                    {activeTable && (
                      <>
                        <div className="space-y-2">
                          <div className="text-xs font-semibold text-slate-600 dark:text-slate-400">
                            Cấu trúc Cột (Schema: {activeDb.name}.{activeTable.tableName})
                          </div>
                          <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
                            {activeTable.columns.map((col) => (
                              <div
                                key={col.name}
                                className="p-2.5 rounded-lg bg-slate-50 dark:bg-slate-950 border border-slate-200/80 dark:border-slate-800 text-xs font-mono"
                              >
                                <div className="font-semibold text-slate-900 dark:text-slate-100">
                                  {col.name}
                                </div>
                                <div className="text-slate-500 dark:text-slate-400 mt-0.5">
                                  {col.dataType}
                                  {col.isPrimaryKey ? ' · PK' : ''}
                                  {!col.nullable ? ' · NOT NULL' : ''}
                                </div>
                              </div>
                            ))}
                          </div>
                        </div>

                        <div className="space-y-2">
                          <div className="flex items-center justify-between">
                            <span className="text-xs font-semibold text-slate-600 dark:text-slate-400">
                              Bản ghi hiện tại trong bảng {activeTable.tableName}
                            </span>
                            <div className="flex items-center gap-3">
                              <button
                                onClick={() => {
                                  setSubTab('phpmyadmin');
                                  setPmaActiveTab('browse');
                                }}
                                className="text-xs font-medium text-emerald-600 dark:text-emerald-400 hover:underline cursor-pointer"
                              >
                                Quản lý trong phpMyAdmin 5.2 →
                              </button>
                              <button
                                onClick={() => {
                                  setSqlInput(
                                    `SELECT * FROM ${activeTable.tableName} LIMIT 50;`
                                  );
                                  setSubTab('query');
                                }}
                                className="text-xs font-medium text-orange-600 dark:text-orange-400 hover:underline cursor-pointer"
                              >
                                Mở trong SQL Studio →
                              </button>
                            </div>
                          </div>

                          <div className="overflow-x-auto border border-slate-200 dark:border-slate-800 rounded-lg">
                            <table className="w-full text-left border-collapse">
                              <thead>
                                <tr className="border-b border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-500 dark:text-slate-400">
                                  {activeTable.columns.map((col) => (
                                    <th key={col.name} className="py-2.5 px-4">
                                      {col.name}
                                    </th>
                                  ))}
                                </tr>
                              </thead>
                              <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-xs font-mono tabular-nums">
                                {activeTable.rows.map((row, idx) => (
                                  <tr
                                    key={idx}
                                    className="hover:bg-slate-50/80 dark:hover:bg-slate-800/40"
                                  >
                                    {activeTable.columns.map((col) => (
                                      <td
                                        key={col.name}
                                        className="py-2.5 px-4 text-slate-800 dark:text-slate-200 whitespace-nowrap"
                                      >
                                        {String(row[col.name] ?? 'NULL')}
                                      </td>
                                    ))}
                                  </tr>
                                ))}
                              </tbody>
                            </table>
                          </div>
                        </div>
                      </>
                    )}
                  </div>
                )}

                {/* SUB-TAB 2: Interactive SQL Query Console */}
                {subTab === 'query' && (
                  <div className="p-6 space-y-4">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <span className="text-xs text-slate-500 mr-1">Lệnh SQL nhanh:</span>
                        <button
                          type="button"
                          onClick={() =>
                            setSqlInput(
                              `SELECT * FROM ${activeDb.tables[0]?.tableName || 'users'} LIMIT 25;`
                            )
                          }
                          className="px-2.5 py-1 text-xs font-mono bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-md cursor-pointer"
                        >
                          SELECT *
                        </button>
                        <button
                          type="button"
                          onClick={() => setSqlInput('SHOW TABLES;')}
                          className="px-2.5 py-1 text-xs font-mono bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-md cursor-pointer"
                        >
                          SHOW TABLES
                        </button>
                        {activeDb.tables[0] && (
                          <button
                            type="button"
                            onClick={() => {
                              const tbl = activeDb.tables[0];
                              const cols = tbl.columns.map((c) => c.name).join(', ');
                              const vals = tbl.columns
                                .map((c) =>
                                  c.dataType.includes('INT')
                                    ? '1001'
                                    : c.dataType.includes('BOOL')
                                      ? 'true'
                                      : `'record_${Math.floor(100 + Math.random() * 899)}'`
                                )
                                .join(', ');
                              setSqlInput(
                                `INSERT INTO ${tbl.tableName} (${cols}) VALUES (${vals});`
                              );
                            }}
                            className="px-2.5 py-1 text-xs font-mono bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-md cursor-pointer"
                          >
                            INSERT INTO
                          </button>
                        )}
                      </div>

                      <button
                        onClick={() => handleRunSql()}
                        disabled={runningQuery}
                        className="px-4 py-1.5 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
                      >
                        <Play className="w-3.5 h-3.5" />
                        {runningQuery ? 'Đang thực thi...' : 'Thực thi SQL'}
                      </button>
                    </div>

                    <textarea
                      value={sqlInput}
                      onChange={(e) => setSqlInput(e.target.value)}
                      rows={5}
                      spellCheck={false}
                      className="w-full p-4 rounded-lg bg-slate-950 text-slate-100 font-mono text-xs leading-relaxed border border-slate-800 focus:outline-none focus:ring-2 focus:ring-orange-500/40"
                    />

                    {queryResult && (
                      <div className="space-y-3 pt-2">
                        <div className="flex items-center justify-between text-xs font-mono tabular-nums">
                          <span
                            className={
                              queryResult.success
                                ? 'text-emerald-600 dark:text-emerald-400 font-semibold'
                                : 'text-rose-600 dark:text-rose-400 font-semibold'
                            }
                          >
                            {queryResult.commandTag} · {queryResult.affectedRows} bản ghi
                          </span>
                          <span className="text-slate-500">
                            Thời gian thực thi: {queryResult.executionTimeMs} ms
                          </span>
                        </div>

                        {queryResult.error ? (
                          <div className="p-4 rounded-lg bg-rose-950/40 border border-rose-800 text-rose-200 font-mono text-xs">
                            {queryResult.error}
                          </div>
                        ) : (
                          queryResult.columns.length > 0 && (
                            <div className="overflow-x-auto border border-slate-200 dark:border-slate-800 rounded-lg">
                              <table className="w-full text-left border-collapse">
                                <thead>
                                  <tr className="border-b border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-xs font-mono text-slate-500 dark:text-slate-400">
                                    {queryResult.columns.map((c) => (
                                      <th key={c} className="py-2.5 px-4">
                                        {c}
                                      </th>
                                    ))}
                                  </tr>
                                </thead>
                                <tbody className="divide-y divide-slate-200 dark:divide-slate-800 text-xs font-mono tabular-nums">
                                  {queryResult.rows.map((r, i) => (
                                    <tr key={i}>
                                      {queryResult.columns.map((c) => (
                                        <td
                                          key={c}
                                          className="py-2.5 px-4 text-slate-800 dark:text-slate-200 whitespace-nowrap"
                                        >
                                          {String(r[c] ?? 'NULL')}
                                        </td>
                                      ))}
                                    </tr>
                                  ))}
                                </tbody>
                              </table>
                            </div>
                          )
                        )}
                      </div>
                    )}
                  </div>
                )}
              </div>
            ))}
        </div>
      </div>
    </div>
  );
};
