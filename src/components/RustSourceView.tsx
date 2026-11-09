import React, { useState } from 'react';
import { Check, Code2, Copy, Download, Terminal } from 'lucide-react';
import { RustSourceFile } from '../types';

interface RustSourceViewProps {
  files: RustSourceFile[];
}

export const RustSourceView: React.FC<RustSourceViewProps> = ({ files }) => {
  const [selectedPath, setSelectedPath] = useState<string>(
    files[1]?.path || files[0]?.path || ''
  );
  const [copied, setCopied] = useState(false);

  const activeFile =
    files.find((f) => f.path === selectedPath) || files[0] || null;

  const handleCopy = () => {
    if (!activeFile) return;
    navigator.clipboard.writeText(activeFile.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleDownloadFile = () => {
    if (!activeFile) return;
    const blob = new Blob([activeFile.content], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = activeFile.title.replace('/', '_');
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="space-y-6">
      {/* Architecture & Compilation Banner */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-4">
        <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4">
          <div className="space-y-1">
            <div className="text-xs font-mono text-orange-600 dark:text-orange-400">
              rustc 1.83+ · actix-web 4.9 · rustls 0.23 · argon2 0.5 · sysinfo 0.32
            </div>
            <h1 className="text-lg font-bold text-slate-900 dark:text-slate-100">
              Mã nguồn Ferrous-panel (Actix-Web Rust) &amp; Script Triển khai Ubuntu
            </h1>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Toàn bộ mã nguồn <code className="font-mono">/Ferrous-panel/</code> được biên dịch với cờ tối ưu hóa <code className="font-mono">lto = "fat"</code> và <code className="font-mono">panic = "abort"</code> giúp tiêu thụ dưới 18 MB RAM trên Ubuntu
            </p>
          </div>

          <div className="flex items-center gap-2 shrink-0">
            <button
              onClick={handleDownloadFile}
              className="px-3.5 py-2 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
            >
              <Download className="w-3.5 h-3.5" />
              Tải tệp đang chọn
            </button>
            <button
              onClick={handleCopy}
              className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
            >
              {copied ? (
                <>
                  <Check className="w-3.5 h-3.5" />
                  Đã sao chép mã nguồn
                </>
              ) : (
                <>
                  <Copy className="w-3.5 h-3.5" />
                  Sao chép mã nguồn
                </>
              )}
            </button>
          </div>
        </div>

        <div className="p-4 rounded-lg bg-slate-950 border border-slate-800 text-xs font-mono text-slate-300 flex flex-col sm:flex-row sm:items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <Terminal className="w-4 h-4 text-orange-400 shrink-0" />
            <span>
              cd Ferrous-panel &amp;&amp; cargo build --release &amp;&amp; sudo ./install-ubuntu.sh
            </span>
          </div>
          <span className="text-slate-500">Ubuntu 22.04 / 24.04 LTS x86_64 &amp; ARM64</span>
        </div>
      </section>

      {/* File Tree + Code Viewer */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
        <div className="lg:col-span-4 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 divide-y divide-slate-200 dark:divide-slate-800 overflow-hidden">
          <div className="px-5 py-3.5 bg-slate-50/70 dark:bg-slate-950/40 text-xs font-semibold text-slate-600 dark:text-slate-400">
            Cấu trúc thư mục /Ferrous-panel
          </div>
          {files.map((file) => {
            const isSelected = activeFile?.path === file.path;
            return (
              <button
                key={file.path}
                onClick={() => setSelectedPath(file.path)}
                className={`w-full text-left p-4 transition-colors block cursor-pointer ${
                  isSelected
                    ? 'bg-orange-50/60 dark:bg-orange-950/20'
                    : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="font-mono text-sm font-semibold text-slate-900 dark:text-slate-100 flex items-center gap-2">
                    <Code2 className="w-4 h-4 text-orange-600 dark:text-orange-400 shrink-0" />
                    {file.title}
                  </span>
                  <span className="text-xs font-mono text-slate-400">
                    {file.lang}
                  </span>
                </div>
                <p className="mt-1.5 text-xs text-slate-500 dark:text-slate-400 leading-relaxed">
                  {file.description}
                </p>
              </button>
            );
          })}
        </div>

        {activeFile && (
          <div className="lg:col-span-8 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
            <div className="px-5 py-3.5 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
              <div className="font-mono text-xs font-semibold text-slate-800 dark:text-slate-200">
                {activeFile.path}
              </div>
              <span className="text-xs text-slate-500 dark:text-slate-400">
                {activeFile.description}
              </span>
            </div>
            <pre className="p-5 bg-slate-950 text-slate-100 font-mono text-xs leading-relaxed overflow-x-auto max-h-[640px]">
              {activeFile.content}
            </pre>
          </div>
        )}
      </div>
    </div>
  );
};
