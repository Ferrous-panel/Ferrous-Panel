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
  Settings,
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
  >('tables');

  const [sqlInput, setSqlInput] = useState<string>(
    'SELECT * FROM orders LIMIT 25;'
  );
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
    mysqlConfig?.pmaSslOnly ?? true
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
    if (!activeDb) return;
    const lines: string[] = [
      `-- FerrousPanel SQL Dump (${activeDb.engine})`,
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
    if (!activeDb) return;
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
      `Đã tạo Database "${newDbName.trim()}" với mật khẩu tùy chỉnh và tự động đồng bộ DB_PASSWORD vào tệp /.env của Website!`
    );
    setTimeout(() => setConfigSavedBanner(null), 4500);
    setNewDbName('');
    setNewUsername('');
    setNewPassword(generateStrongSqlPassword());
    setSubTab('tables');
  };

  const handleRotatePasswordSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeDb || !rotatePasswordValue.trim() || !onChangeDatabasePassword)
      return;
    await onChangeDatabasePassword(activeDb.id, rotatePasswordValue.trim());
    setIsRotatingPass(false);
    setRotatePasswordValue('');
    setConfigSavedBanner(
      `Đã đổi mật khẩu SQL cho user "${activeDb.username}" (ALTER USER) và cập nhật tự động vào /.env của ${activeDb.linkedAppName}!`
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
        'Đã ghi /etc/mysql/mariadb.conf.d/50-server.cnf, áp dụng cấu hình phpMyAdmin 5.2.1 và reload dịch vụ thành công.'
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
            : `'pma_row_${Math.floor(100 + Math.random() * 899)}'`
      )
      .join(', ');
    const q = `INSERT INTO ${activeTable.tableName} (${cols}) VALUES (${vals});`;
    await onExecuteQuery(activeDb.id, q);
    setPmaQuickInsertToast(
      `Đã chèn 1 bản ghi mới vào bảng ${activeDb.name}.${activeTable.tableName} qua phpMyAdmin SSO`
    );
    setTimeout(() => setPmaQuickInsertToast(null), 3000);
  };

  return (
    <div className="space-y-6">
      {/* Top Summary Header */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 flex flex-col lg:flex-row lg:items-center justify-between gap-4">
        <div className="space-y-1">
          <div className="text-xs font-mono text-slate-500 dark:text-slate-400">
            MariaDB 11.4.4 LTS (MySQL) · PostgreSQL 17.2 · phpMyAdmin 5.2.1 SSO · Loopback Isolated (127.0.0.1)
          </div>
          <h1 className="text-lg font-bold text-slate-900 dark:text-slate-100">
            Quản trị Cơ sở Dữ liệu SQL, Cấu hình MySQL <code className="font-mono">my.cnf</code> &amp; phpMyAdmin 5.2
          </h1>
          <p className="text-xs text-slate-500 dark:text-slate-400">
            Quản lý schema, tối ưu bộ nhớ <code className="font-mono">innodb_buffer_pool_size</code>, quản trị bảng qua phpMyAdmin 5.2 SSO và thực thi truy vấn SQL trực tiếp.
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2 shrink-0">
          <button
            onClick={() => setSubTab('phpmyadmin')}
            className={`px-3.5 py-2 text-xs font-semibold rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
              subTab === 'phpmyadmin'
                ? 'bg-slate-900 dark:bg-orange-500 text-white'
                : 'text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700'
            }`}
          >
            <ExternalLink className="w-3.5 h-3.5 text-orange-600 dark:text-orange-300" />
            phpMyAdmin 5.2 (Port {pmaPort})
          </button>
          <button
            onClick={() => setSubTab('mysql_tuning')}
            className={`px-3.5 py-2 text-xs font-semibold rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
              subTab === 'mysql_tuning'
                ? 'bg-slate-900 dark:bg-orange-500 text-white'
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
            className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
          >
            <Plus className="w-3.5 h-3.5" />
            Tạo Database Mới
          </button>
        </div>
      </section>

      {configSavedBanner && (
        <div className="p-3.5 rounded-xl bg-emerald-50 dark:bg-emerald-950/40 border border-emerald-200 dark:border-emerald-800 text-xs font-medium text-emerald-800 dark:text-emerald-300 flex items-center justify-between">
          <span>{configSavedBanner}</span>
          <span className="font-mono text-[11px]">systemctl reload mariadb nginx</span>
        </div>
      )}

      {/* Main Split: Left Database List & Right Inspector / phpMyAdmin / MySQL Tuning */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
        {/* Left: Databases List + MySQL Status Summary */}
        <div className="lg:col-span-4 space-y-4">
          <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 divide-y divide-slate-200 dark:divide-slate-800 overflow-hidden">
            <div className="px-5 py-3.5 bg-slate-50/70 dark:bg-slate-950/40 flex items-center justify-between text-xs font-semibold text-slate-600 dark:text-slate-400">
              <span>Databases Đang Hoạt Động ({databases.length})</span>
              <Database className="w-3.5 h-3.5" />
            </div>

            {databases.length === 0 ? (
              <div className="p-8 text-center text-slate-500 dark:text-slate-400">
                <Database className="w-8 h-8 mx-auto mb-2 opacity-40 text-slate-400" />
                <div className="text-xs font-semibold">Chưa có database nào</div>
                <div className="text-[11px] mt-1 text-slate-400">
                  Chọn &quot;Khởi Tạo Database&quot; bên phải để bắt đầu.
                </div>
              </div>
            ) :
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
            })}
          </div>

          {/* Quick MySQL / phpMyAdmin Runtime Card */}
          {mysqlConfig && (
            <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-4 space-y-3 text-xs">
              <div className="flex items-center justify-between font-semibold text-slate-900 dark:text-white">
                <span>Trạng thái MySQL &amp; phpMyAdmin</span>
                <span className="font-mono text-emerald-600 dark:text-emerald-400">
                  Active
                </span>
              </div>
              <div className="space-y-1.5 font-mono text-[11px] text-slate-600 dark:text-slate-400 tabular-nums">
                <div className="flex justify-between">
                  <span>Engine:</span>
                  <span className="text-slate-900 dark:text-slate-200 truncate max-w-[180px]">
                    {mysqlConfig.mysqlVersion}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span>InnoDB Buffer Pool:</span>
                  <span className="text-slate-900 dark:text-slate-200">
                    {mysqlConfig.innodbBufferPoolMb} MB
                  </span>
                </div>
                <div className="flex justify-between">
                  <span>phpMyAdmin URI:</span>
                  <span className="text-orange-600 dark:text-orange-400">
                    :{mysqlConfig.pmaPort}
                    {mysqlConfig.pmaPath}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span>Socket / Charset:</span>
                  <span className="text-slate-900 dark:text-slate-200">
                    {mysqlConfig.defaultCharset}
                  </span>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Right: Database Schema Browser / SQL Query Console / phpMyAdmin / MySQL Tuning / Create Form */}
        <div className="lg:col-span-8 space-y-6">
          {subTab === 'create' ? (
            <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-5">
              <div className="border-b border-slate-200 dark:border-slate-800 pb-4">
                <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
                  Khởi tạo Cơ sở Dữ liệu SQL &amp; Tài khoản Truy cập
                </h2>
                <p className="text-xs text-slate-500 dark:text-slate-400">
                  Tự động tạo Database MariaDB / PostgreSQL, cấp quyền Role riêng biệt và đồng bộ vào phpMyAdmin
                </p>
              </div>

              <form onSubmit={handleCreateSubmit} className="space-y-4">
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <div className="space-y-1.5">
                    <label className="block text-xs font-medium text-slate-700 dark:text-slate-300">
                      Tên Database (VD: ecommerce_vn)
                    </label>
                    <input
                      type="text"
                      required
                      placeholder="ecommerce_vn"
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
                        PostgreSQL 17 (Khuyến nghị cho Rust SQLx / Prisma)
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
                      placeholder="ecommerce_vn_usr"
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
                    onClick={() => setSubTab('tables')}
                    className="px-4 py-2 text-xs font-medium text-slate-700 dark:text-slate-300 bg-slate-100 dark:bg-slate-800 rounded-lg cursor-pointer"
                  >
                    Hủy
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
          ) : (
            activeDb && (
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
                          className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
                            subTab === 'phpmyadmin'
                              ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100 shadow-xs'
                              : 'text-slate-600 dark:text-slate-400'
                          }`}
                        >
                          <Settings className="w-3.5 h-3.5" />
                          phpMyAdmin 5.2
                        </button>
                        <button
                          onClick={() => setSubTab('mysql_tuning')}
                          className={`px-3 py-1.5 text-xs font-medium rounded-md transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer ${
                            subTab === 'mysql_tuning'
                              ? 'bg-white dark:bg-slate-800 text-slate-900 dark:text-slate-100 shadow-xs'
                              : 'text-slate-600 dark:text-slate-400'
                          }`}
                        >
                          <Sliders className="w-3.5 h-3.5" />
                          MySQL my.cnf
                        </button>
                      </div>

                      {databases.length > 1 && (
                        <button
                          onClick={() => onDeleteDatabase(activeDb.id)}
                          className="p-2 text-slate-400 hover:text-rose-600 dark:hover:text-rose-400 rounded-lg transition-colors cursor-pointer"
                          title="Xóa Database"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      )}
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
                          ALTER USER '{activeDb.username}' IDENTIFIED BY:
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
                                onClick={() => setSubTab('phpmyadmin')}
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
                        <span className="text-xs text-slate-500 mr-1">Mẫu lệnh nhanh:</span>
                        <button
                          type="button"
                          onClick={() =>
                            setSqlInput(
                              `SELECT * FROM ${activeDb.tables[0]?.tableName || 'orders'} LIMIT 25;`
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
                              const sampleVals = tbl.columns
                                .map((c) =>
                                  c.dataType.includes('INT')
                                    ? '9500000'
                                    : c.dataType.includes('BOOL')
                                      ? 'true'
                                      : `'sample_${Math.floor(100 + Math.random() * 899)}'`
                                )
                                .join(', ');
                              setSqlInput(
                                `INSERT INTO ${tbl.tableName} (${cols}) VALUES (${sampleVals});`
                              );
                            }}
                            className="px-2.5 py-1 text-xs font-mono bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-md cursor-pointer"
                          >
                            INSERT INTO
                          </button>
                        )}
                        <button
                          type="button"
                          onClick={() =>
                            setSqlInput(
                              `EXPLAIN ANALYZE SELECT * FROM ${activeDb.tables[0]?.tableName || 'orders'};`
                            )
                          }
                          className="px-2.5 py-1 text-xs font-mono bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-md cursor-pointer"
                        >
                          EXPLAIN ANALYZE
                        </button>
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

                {/* SUB-TAB 3: phpMyAdmin 5.2 Web GUI & Security Configuration */}
                {subTab === 'phpmyadmin' && (
                  <div className="p-6 space-y-6">
                    {/* Embedded phpMyAdmin 5.2 Single Sign-On Workspace */}
                    <div className="rounded-xl border border-slate-300 dark:border-slate-700 overflow-hidden bg-slate-50 dark:bg-slate-950">
                      <div className="px-4 py-3 bg-slate-800 text-white flex flex-wrap items-center justify-between gap-2 text-xs">
                        <div className="flex items-center gap-2 font-mono">
                          <span className="font-bold text-amber-400">phpMyAdmin 5.2.1</span>
                          <span className="text-slate-400">·</span>
                          <span>Server: 127.0.0.1:3306 (MariaDB 11.4.4-LTS)</span>
                          <span className="text-slate-400">·</span>
                          <span className="text-emerald-400">Database: {activeDb.name}</span>
                        </div>
                        <div className="flex items-center gap-2">
                          <button
                            type="button"
                            onClick={handlePmaInsertSampleRow}
                            className="px-2.5 py-1 bg-amber-500 hover:bg-amber-400 text-slate-950 font-semibold rounded text-[11px] cursor-pointer"
                          >
                            + Chèn dòng (Insert Row)
                          </button>
                          <button
                            type="button"
                            onClick={handleExportSqlDump}
                            className="px-2.5 py-1 bg-slate-700 hover:bg-slate-600 text-white rounded text-[11px] cursor-pointer"
                          >
                            Export .SQL
                          </button>
                        </div>
                      </div>

                      {pmaQuickInsertToast && (
                        <div className="px-4 py-2 bg-emerald-600 text-white text-xs font-mono">
                          {pmaQuickInsertToast}
                        </div>
                      )}

                      <div className="p-4 space-y-4">
                        <div className="overflow-x-auto border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-900">
                          <table className="w-full text-left border-collapse text-xs font-mono">
                            <thead>
                              <tr className="border-b border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-950 text-slate-600 dark:text-slate-400">
                                <th className="py-2.5 px-3">Bảng (Table)</th>
                                <th className="py-2.5 px-3">Bản ghi (Rows)</th>
                                <th className="py-2.5 px-3">Engine</th>
                                <th className="py-2.5 px-3">Collation</th>
                                <th className="py-2.5 px-3">Kích thước</th>
                                <th className="py-2.5 px-3 text-right">Thao tác phpMyAdmin</th>
                              </tr>
                            </thead>
                            <tbody className="divide-y divide-slate-200 dark:divide-slate-800 tabular-nums">
                              {activeDb.tables.map((tbl) => (
                                <tr key={tbl.tableName} className="hover:bg-slate-50 dark:hover:bg-slate-800/50">
                                  <td className="py-2.5 px-3 font-bold text-slate-900 dark:text-white">
                                    {tbl.tableName}
                                  </td>
                                  <td className="py-2.5 px-3">{tbl.rowCount.toLocaleString()}</td>
                                  <td className="py-2.5 px-3">InnoDB</td>
                                  <td className="py-2.5 px-3 text-slate-500">{activeDb.collation}</td>
                                  <td className="py-2.5 px-3">{tbl.sizeKb} KB</td>
                                  <td className="py-2.5 px-3 text-right space-x-2">
                                    <button
                                      type="button"
                                      onClick={() => {
                                        setSelectedTableName(tbl.tableName);
                                        setSubTab('tables');
                                      }}
                                      className="text-sky-600 dark:text-sky-400 hover:underline cursor-pointer"
                                    >
                                      Duyệt (Browse)
                                    </button>
                                    <button
                                      type="button"
                                      onClick={() => {
                                        setSqlInput(`OPTIMIZE TABLE ${tbl.tableName};`);
                                        setSubTab('query');
                                      }}
                                      className="text-emerald-600 dark:text-emerald-400 hover:underline cursor-pointer"
                                    >
                                      Tối ưu (Optimize)
                                    </button>
                                  </td>
                                </tr>
                              ))}
                            </tbody>
                          </table>
                        </div>
                      </div>
                    </div>

                    {/* phpMyAdmin Security & Port Configuration Form */}
                    <form onSubmit={handleSaveMysqlAndPma} className="space-y-4 pt-2 border-t border-slate-200 dark:border-slate-800">
                      <div className="flex items-center justify-between">
                        <div>
                          <h3 className="text-sm font-bold text-slate-900 dark:text-white flex items-center gap-2">
                            <Lock className="w-4 h-4 text-orange-600 dark:text-orange-400" />
                            Cấu hình Bảo mật &amp; Cổng Truy cập phpMyAdmin (Nginx Guard)
                          </h3>
                          <p className="text-xs text-slate-500 dark:text-slate-400">
                            Đổi đường dẫn URI bí mật và bật xác thực 2 lớp Nginx Basic Auth để chống dò quét tự động.
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
                          <span>Bật khóa mật khẩu tầng 1 (Nginx HTTP Basic Auth trước khi vào phpMyAdmin)</span>
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

                {/* SUB-TAB 4: MySQL / MariaDB my.cnf Performance Tuning */}
                {subTab === 'mysql_tuning' && (
                  <form onSubmit={handleSaveMysqlAndPma} className="p-6 space-y-5">
                    <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
                      <div>
                        <h3 className="text-sm font-bold text-slate-900 dark:text-white">
                          Tinh chỉnh Hiệu năng MySQL / MariaDB (<code className="font-mono">/etc/mysql/mariadb.conf.d/50-server.cnf</code>)
                        </h3>
                        <p className="text-xs text-slate-500 dark:text-slate-400">
                          Tối ưu bộ đệm RAM InnoDB, giới hạn kết nối đồng thời và nhật ký truy vấn chậm (Slow Query Log).
                        </p>
                      </div>
                      <button
                        type="submit"
                        disabled={savingConfig}
                        className="px-4 py-2 bg-orange-600 hover:bg-orange-500 text-white text-xs font-semibold rounded-lg inline-flex items-center gap-1.5 shrink-0 cursor-pointer"
                      >
                        <RefreshCw className={`w-3.5 h-3.5 ${savingConfig ? 'animate-spin' : ''}`} />
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
                          onChange={(e) => setInnodbBufferPoolMb(Number(e.target.value))}
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
                          onChange={(e) => setMaxConnections(Number(e.target.value))}
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
                          onChange={(e) => setLongQueryTimeSec(Number(e.target.value))}
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
                )}
              </div>
            )
          )}
        </div>
      </div>
    </div>
  );
};
