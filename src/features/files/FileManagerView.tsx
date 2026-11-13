import React, { useState, useEffect, useCallback, useRef } from 'react';
import {
  Archive,
  Check,
  CheckCircle2,
  Download,
  Edit3,
  FileArchive,
  FileCode,
  Folder,
  FolderOpen,
  FolderPlus,
  Globe,
  Lock,
  PackageOpen,
  Plus,
  Save,
  Search,
  ShieldCheck,
  Trash2,
  Upload,
  X
} from 'lucide-react';
import { ManagedApp, WebFileNode } from '../types';

interface FileManagerViewProps {
  apps: ManagedApp[];
  selectedAppId: string | null;
  onSelectApp: (id: string) => void;
}

interface ParsedZipEntry {
  relativePath: string;
  isDir: boolean;
  sizeBytes: number;
  content?: string;
}

/**
 * Parses a real .zip ArrayBuffer in the browser using Local File Headers (0x04034b50)
 * and decompresses stored (method 0) or deflated (method 8) text entries via DecompressionStream.
 */
async function parseZipArrayBuffer(
  buffer: ArrayBuffer
): Promise<ParsedZipEntry[]> {
  const view = new DataView(buffer);
  const bytes = new Uint8Array(buffer);
  const entries: ParsedZipEntry[] = [];
  const decoder = new TextDecoder('utf-8', { fatal: false });
  let offset = 0;

  while (offset + 30 <= buffer.byteLength) {
    const sig = view.getUint32(offset, true);
    if (sig !== 0x04034b50) {
      break;
    }
    const compressionMethod = view.getUint16(offset + 8, true);
    const compressedSize = view.getUint32(offset + 18, true);
    const uncompressedSize = view.getUint32(offset + 22, true);
    const fileNameLen = view.getUint16(offset + 26, true);
    const extraLen = view.getUint16(offset + 28, true);

    const nameBytes = bytes.subarray(offset + 30, offset + 30 + fileNameLen);
    const rawName = decoder.decode(nameBytes);
    const dataStart = offset + 30 + fileNameLen + extraLen;
    const dataEnd = Math.min(buffer.byteLength, dataStart + compressedSize);
    const compressedSlice = bytes.subarray(dataStart, dataEnd);

    const isDir = rawName.endsWith('/');
    let content: string | undefined = undefined;

    if (!isDir && uncompressedSize <= 512 * 1024) {
      try {
        if (compressionMethod === 0) {
          content = decoder.decode(compressedSlice);
        } else if (
          compressionMethod === 8 &&
          typeof DecompressionStream !== 'undefined'
        ) {
          const ds = new DecompressionStream('deflate-raw');
          const writer = ds.writable.getWriter();
          writer.write(compressedSlice);
          writer.close();
          const decompressedBuf = await new Response(ds.readable).arrayBuffer();
          content = decoder.decode(new Uint8Array(decompressedBuf));
        }
      } catch {
        content = `// Extracted from ZIP archive: ${rawName} (${uncompressedSize} bytes)\n`;
      }
    }

    entries.push({
      relativePath: rawName.replace(/\/$/, ''),
      isDir,
      sizeBytes: uncompressedSize || compressedSize || 512,
      content
    });

    offset = dataEnd;
    if (entries.length >= 120) break;
  }

  return entries;
}

export const FileManagerView: React.FC<FileManagerViewProps> = ({
  apps,
  selectedAppId,
  onSelectApp
}) => {
  const activeApp =
    apps.find((a) => a.id === selectedAppId) || apps[0] || null;

  const [files, setFiles] = useState<WebFileNode[]>([]);
  const [currentDir, setCurrentDir] = useState<string>('/');
  const [searchQuery, setSearchQuery] = useState<string>('');
  const [selectedFileId, setSelectedFileId] = useState<string | null>(null);
  const [editorContent, setEditorContent] = useState<string>('');
  const [editorPerms, setEditorPerms] = useState<string>('0644');
  const [saving, setSaving] = useState<boolean>(false);
  const [savedToast, setSavedToast] = useState<boolean>(false);
  const [bannerMsg, setBannerMsg] = useState<string | null>(null);

  // Create file/folder state
  const [showCreateForm, setShowCreateForm] = useState<boolean>(false);
  const [createType, setCreateType] = useState<'file' | 'directory'>('file');
  const [createName, setCreateName] = useState<string>('');
  const [createPerms, setCreatePerms] = useState<string>('0644');

  // Upload .zip & Extract Modal state
  const [isZipModalOpen, setIsZipModalOpen] = useState<boolean>(false);
  const [extractImmediately, setExtractImmediately] = useState<boolean>(true);
  const [uploadingZip, setUploadingZip] = useState<boolean>(false);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  // Remote URL Download Modal state
  const [isRemoteUrlOpen, setIsRemoteUrlOpen] = useState<boolean>(false);
  const [remoteUrl, setRemoteUrl] = useState<string>(
    'https://wordpress.org/latest.zip'
  );
  const [remoteFileName, setRemoteFileName] = useState<string>(
    'wordpress-latest.zip'
  );
  const [downloadingRemote, setDownloadingRemote] = useState<boolean>(false);

  // Rename Modal state
  const [renamingNode, setRenamingNode] = useState<WebFileNode | null>(null);
  const [renameValue, setRenameValue] = useState<string>('');

  const showNotice = (msg: string) => {
    setBannerMsg(msg);
    setTimeout(() => setBannerMsg(null), 4500);
  };

  const fetchFilesForApp = useCallback(async (appId: string) => {
    const res = await fetch(`/api/files/${appId}`);
    if (res.ok) {
      const data: WebFileNode[] = await res.json();
      setFiles(data);
      const firstFile = data.find((f) => f.type === 'file');
      if (firstFile) {
        setSelectedFileId(firstFile.id);
        setEditorContent(firstFile.content || '');
        setEditorPerms(firstFile.permissions);
      } else {
        setSelectedFileId(null);
      }
    }
  }, []);

  useEffect(() => {
    if (activeApp) {
      setCurrentDir('/');
      fetchFilesForApp(activeApp.id);
    }
  }, [activeApp?.id, fetchFilesForApp]);

  const activeFile = files.find((f) => f.id === selectedFileId) || null;
  const isArchiveFile = (name: string) =>
    /\.(zip|tar\.gz|tar\.zst|tgz)$/i.test(name);

  const handleSelectFile = (node: WebFileNode) => {
    if (node.type === 'directory') {
      setCurrentDir(node.path);
      return;
    }
    setSelectedFileId(node.id);
    setEditorContent(node.content || '');
    setEditorPerms(node.permissions);
  };

  const handleSaveFile = async () => {
    if (!activeApp || !activeFile) return;
    setSaving(true);
    try {
      const res = await fetch(`/api/files/${activeApp.id}/${activeFile.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          content: editorContent,
          permissions: editorPerms
        })
      });
      if (res.ok) {
        const updated: WebFileNode[] = await res.json();
        setFiles(updated);
        setSavedToast(true);
        setTimeout(() => setSavedToast(false), 2000);
      }
    } finally {
      setSaving(false);
    }
  };

  const handleCreateItem = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeApp || !createName.trim()) return;
    const res = await fetch(`/api/files/${activeApp.id}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        name: createName.trim(),
        parentDir: currentDir,
        type: createType,
        permissions: createPerms,
        content:
          createType === 'file'
            ? `// Created in ${activeApp.workingDir}${currentDir === '/' ? '' : currentDir}/${createName.trim()}\n`
            : undefined
      })
    });
    if (res.ok) {
      const updated: WebFileNode[] = await res.json();
      setFiles(updated);
      setCreateName('');
      setShowCreateForm(false);
      showNotice(
        `Đã tạo ${createType === 'directory' ? 'thư mục' : 'tệp tin'} ${createName.trim()} trong ${currentDir}`
      );
    }
  };

  const handleDeleteNode = async (nodeId: string) => {
    if (!activeApp) return;
    const res = await fetch(`/api/files/${activeApp.id}/${nodeId}`, {
      method: 'DELETE'
    });
    if (res.ok) {
      const updated: WebFileNode[] = await res.json();
      setFiles(updated);
      if (selectedFileId === nodeId) {
        const nextFile = updated.find((f) => f.type === 'file') || null;
        setSelectedFileId(nextFile?.id || null);
        setEditorContent(nextFile?.content || '');
      }
    }
  };

  // Real Browser File / .zip Upload Handler
  const handleBrowserFileUpload = async (
    e: React.ChangeEvent<HTMLInputElement>
  ) => {
    const selectedFiles = e.target.files;
    if (!selectedFiles || selectedFiles.length === 0 || !activeApp) return;

    setUploadingZip(true);
    try {
      for (let i = 0; i < selectedFiles.length; i++) {
        const fileObj = selectedFiles[i];
        const lowerName = fileObj.name.toLowerCase();

        if (lowerName.endsWith('.zip')) {
          const arrayBuf = await fileObj.arrayBuffer();
          const parsedEntries = await parseZipArrayBuffer(arrayBuf);
          const res = await fetch(`/api/files/${activeApp.id}/upload-zip`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              archiveName: fileObj.name,
              parentDir: currentDir,
              archiveSizeBytes: fileObj.size,
              extractImmediately,
              entries: parsedEntries
            })
          });
          if (res.ok) {
            const data = await res.json();
            setFiles(data.files);
            showNotice(
              extractImmediately
                ? `Đã tải lên ${fileObj.name} và giải nén ${data.extractedCount} mục vào ${activeApp.workingDir}${currentDir}`
                : `Đã tải lên tệp nén ${fileObj.name} (${(fileObj.size / 1024).toFixed(1)} KB). Nhấn "Giải nén" bất cứ lúc nào để bung tệp.`
            );
          }
        } else {
          // Regular source/text file upload
          const textContent = await fileObj.text();
          const res = await fetch(`/api/files/${activeApp.id}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              name: fileObj.name,
              parentDir: currentDir,
              type: 'file',
              permissions: '0644',
              content: textContent
            })
          });
          if (res.ok) {
            const updated: WebFileNode[] = await res.json();
            setFiles(updated);
            showNotice(
              `Đã tải lên tệp ${fileObj.name} vào ${activeApp.workingDir}${currentDir}`
            );
          }
        }
      }
      setIsZipModalOpen(false);
    } finally {
      setUploadingZip(false);
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
  };

  // Extract an existing .zip file in the directory tree
  const handleExtractArchiveNode = async (node: WebFileNode) => {
    if (!activeApp) return;
    const res = await fetch(
      `/api/files/${activeApp.id}/${node.id}/extract`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ targetDir: currentDir })
      }
    );
    if (res.ok) {
      const data = await res.json();
      setFiles(data.files);
      showNotice(
        `Rust ZIP Unpacker: Đã giải nén ${data.extractedCount} tệp/thư mục từ [${node.name}] vào ${activeApp.workingDir}${currentDir}`
      );
    }
  };

  // Compress current directory into a .zip archive
  const handleCompressCurrentDir = async () => {
    if (!activeApp) return;
    const defaultArchiveName = `${activeApp.domain.split('.')[0]}_${currentDir === '/' ? 'wwwroot' : currentDir.replace(/\//g, '_')}_${new Date().toISOString().slice(0, 10)}.zip`;
    const res = await fetch(`/api/files/${activeApp.id}/compress`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        sourceDir: currentDir,
        archiveName: defaultArchiveName
      })
    });
    if (res.ok) {
      const data = await res.json();
      setFiles(data.files);
      showNotice(
        `Đã nén thư mục ${currentDir} thành tệp [${data.archiveName}] (${(data.compressedBytes / 1024).toFixed(1)} KB)`
      );
    }
  };

  // Remote URL Download
  const handleRemoteDownload = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeApp || !remoteUrl.trim()) return;
    setDownloadingRemote(true);
    try {
      const res = await fetch(`/api/files/${activeApp.id}/remote-download`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          url: remoteUrl.trim(),
          parentDir: currentDir,
          fileName: remoteFileName.trim()
        })
      });
      if (res.ok) {
        const updated: WebFileNode[] = await res.json();
        setFiles(updated);
        setIsRemoteUrlOpen(false);
        showNotice(
          `Đã tải tệp từ URL về ${activeApp.workingDir}${currentDir}/${remoteFileName.trim()}`
        );
      }
    } finally {
      setDownloadingRemote(false);
    }
  };

  // Rename file/folder
  const handleConfirmRename = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeApp || !renamingNode || !renameValue.trim()) return;
    const res = await fetch(
      `/api/files/${activeApp.id}/${renamingNode.id}/rename`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ newName: renameValue.trim() })
      }
    );
    if (res.ok) {
      const updated: WebFileNode[] = await res.json();
      setFiles(updated);
      showNotice(`Đã đổi tên thành ${renameValue.trim()}`);
      setRenamingNode(null);
    }
  };

  // Fix Permissions (0755 folders / 0644 files)
  const handleBatchFixPermissions = async () => {
    if (!activeApp) return;
    const res = await fetch(`/api/files/${activeApp.id}/batch-chmod`, {
      method: 'POST'
    });
    if (res.ok) {
      const updated: WebFileNode[] = await res.json();
      setFiles(updated);
      showNotice(
        `Đã chuẩn hóa phân quyền POSIX (Thư mục 0755, Tệp 0644, Owner www-data:www-data) cho ${activeApp.workingDir}`
      );
    }
  };

  const handleDownloadFile = () => {
    if (!activeFile) return;
    const blob = new Blob([editorContent], {
      type: 'text/plain;charset=utf-8'
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = activeFile.name;
    a.click();
    URL.revokeObjectURL(url);
  };

  const visibleNodes = files
    .filter((f) => {
      if (searchQuery.trim()) {
        return (
          f.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
          f.path.toLowerCase().includes(searchQuery.toLowerCase())
        );
      }
      return f.parentDir === currentDir;
    })
    .sort((a, b) => {
      if (a.type !== b.type) return a.type === 'directory' ? -1 : 1;
      return a.name.localeCompare(b.name);
    });

  if (!activeApp) {
    return (
      <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-12 text-center space-y-3">
        <Folder className="w-12 h-12 mx-auto text-slate-400 opacity-50" />
        <h2 className="text-base font-bold text-slate-900 dark:text-slate-100">
          Chưa có Website nào được tạo
        </h2>
        <p className="text-xs text-slate-500 max-w-md mx-auto">
          Trình quản lý tệp tin gắn liền với từng Web Root (/www/wwwroot/domain). Hãy tạo Website đầu tiên để bắt đầu tải lên mã nguồn, quản lý tệp và giải nén ZIP.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Top Bar: Website Selector & Rich File Manager Actions */}
      <section className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 p-6 space-y-4">
        <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-4">
          <div className="space-y-1">
            <div className="text-xs font-mono text-slate-500 dark:text-slate-400">
              Rust ZIP &amp; POSIX Chroot Engine · ReadWritePaths=
              {activeApp?.workingDir} · Owner: www-data:www-data
            </div>
            <h1 className="text-lg font-bold text-slate-900 dark:text-slate-100 font-display">
              Trình Quản lý Tệp tin &amp; Giải nén ZIP Theo Từng Website (Web
              Root File Manager)
            </h1>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Hỗ trợ tải lên tệp <code className="font-mono">.zip</code> từ máy
              tính, giải nén 1-click chống Zip-Slip, nén thư mục, tải tệp từ URL
              từ xa và sửa trực tiếp mã nguồn.
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2 shrink-0">
            <button
              onClick={() => setIsZipModalOpen(true)}
              className="px-3.5 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
            >
              <Upload className="w-3.5 h-3.5" />
              Tải lên .ZIP &amp; Giải nén
            </button>

            <button
              onClick={handleCompressCurrentDir}
              className="px-3 py-2 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
              title="Nén toàn bộ thư mục hiện tại thành tệp .zip"
            >
              <Archive className="w-3.5 h-3.5 text-orange-500" />
              Nén .ZIP
            </button>

            <button
              onClick={() => setIsRemoteUrlOpen(true)}
              className="px-3 py-2 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
              title="Tải tệp hoặc gói .zip từ liên kết URL bên ngoài"
            >
              <Globe className="w-3.5 h-3.5 text-sky-500" />
              Tải từ URL
            </button>

            <button
              onClick={() => {
                setCreateType('file');
                setCreatePerms('0644');
                setShowCreateForm(true);
              }}
              className="px-3 py-2 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
            >
              <Plus className="w-3.5 h-3.5" />
              Tạo Tệp
            </button>

            <button
              onClick={() => {
                setCreateType('directory');
                setCreatePerms('0755');
                setShowCreateForm(true);
              }}
              className="px-3 py-2 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
            >
              <FolderPlus className="w-3.5 h-3.5" />
              Tạo Thư Mục
            </button>

            <button
              onClick={handleBatchFixPermissions}
              className="px-3 py-2 text-xs font-medium text-emerald-700 dark:text-emerald-300 bg-emerald-500/10 hover:bg-emerald-500/20 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
              title="Chuẩn hóa quyền 0755 cho thư mục & 0644 cho tệp tin"
            >
              <ShieldCheck className="w-3.5 h-3.5" />
              Chuẩn hóa Chmod
            </button>
          </div>
        </div>

        {/* Website / Service Selector Tabs + Search */}
        <div className="flex flex-wrap items-center justify-between gap-3 pt-2 border-t border-slate-200 dark:border-slate-800">
          <div className="flex items-center gap-1.5 overflow-x-auto">
            {apps.map((app) => {
              const isSelected = activeApp?.id === app.id;
              return (
                <button
                  key={app.id}
                  onClick={() => onSelectApp(app.id)}
                  className={`px-3.5 py-2 text-xs font-mono rounded-lg transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                    isSelected
                      ? 'bg-orange-600 text-white font-semibold'
                      : 'bg-slate-100 dark:bg-slate-950 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-800'
                  }`}
                >
                  {app.domain} ({app.workingDir})
                </button>
              );
            })}
          </div>

          <div className="relative">
            <Search className="w-3.5 h-3.5 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Tìm nhanh tệp trong Web Root..."
              className="pl-8 pr-3 py-1.5 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100 w-56"
            />
          </div>
        </div>
      </section>

      {bannerMsg && (
        <div className="bg-emerald-500/10 border border-emerald-500/30 rounded-xl px-4 py-3 flex items-center justify-between gap-3">
          <div className="flex items-center gap-2 text-xs font-medium text-emerald-700 dark:text-emerald-300">
            <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
            <span>{bannerMsg}</span>
          </div>
          <button
            onClick={() => setBannerMsg(null)}
            className="text-xs text-slate-500 hover:text-slate-800 dark:hover:text-slate-200 cursor-pointer"
          >
            Đóng
          </button>
        </div>
      )}

      {/* Optional Create File / Directory Inline Form */}
      {showCreateForm && activeApp && (
        <form
          onSubmit={handleCreateItem}
          className="rounded-xl border border-orange-300 dark:border-orange-800 bg-white dark:bg-slate-900 p-5 flex flex-col sm:flex-row sm:items-center justify-between gap-3"
        >
          <div className="flex flex-wrap items-center gap-3 flex-1">
            <span className="text-xs font-mono text-slate-500">
              {activeApp.workingDir}
              {currentDir === '/' ? '/' : `${currentDir}/`}
            </span>
            <input
              type="text"
              required
              placeholder={
                createType === 'file'
                  ? 'Tên tệp (VD: index.php hoặc routes.rs)'
                  : 'Tên thư mục (VD: uploads)'
              }
              value={createName}
              onChange={(e) => setCreateName(e.target.value)}
              className="px-3 py-1.5 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100 w-64"
            />
            <select
              value={createPerms}
              onChange={(e) => setCreatePerms(e.target.value)}
              className="px-3 py-1.5 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
            >
              <option value="0644">chmod 0644 (rw-r--r--)</option>
              <option value="0600">chmod 0600 (rw------- Bí mật)</option>
              <option value="0755">
                chmod 0755 (rwxr-xr-x Thực thi/Thư mục)
              </option>
              <option value="0750">chmod 0750 (rwxr-x---)</option>
            </select>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <button
              type="button"
              onClick={() => setShowCreateForm(false)}
              className="px-3 py-1.5 text-xs font-medium text-slate-600 dark:text-slate-400 hover:underline cursor-pointer"
            >
              Hủy
            </button>
            <button
              type="submit"
              className="px-4 py-1.5 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg cursor-pointer"
            >
              Tạo ngay
            </button>
          </div>
        </form>
      )}

      {/* Split Layout: Left Directory Table & Right File Code Editor */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
        {/* Left: Directory & Archive Browser */}
        <div className="lg:col-span-5 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
          {/* Breadcrumb Bar */}
          <div className="px-4 py-3 border-b border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-950/50 flex items-center justify-between text-xs font-mono">
            <div className="flex items-center gap-1.5 truncate">
              <FolderOpen className="w-3.5 h-3.5 text-orange-600 dark:text-orange-400 shrink-0" />
              <button
                onClick={() => setCurrentDir('/')}
                className="hover:underline font-semibold text-slate-900 dark:text-slate-100 cursor-pointer"
              >
                {activeApp?.workingDir}
              </button>
              {currentDir !== '/' && (
                <span className="text-orange-600 dark:text-orange-400 font-semibold">
                  {currentDir}
                </span>
              )}
            </div>
            {currentDir !== '/' && (
              <button
                onClick={() => {
                  const parts = currentDir.split('/').filter(Boolean);
                  parts.pop();
                  setCurrentDir(parts.length === 0 ? '/' : `/${parts.join('/')}`);
                }}
                className="text-xs text-orange-600 dark:text-orange-400 hover:underline shrink-0 cursor-pointer"
              >
                ← Lên 1 cấp
              </button>
            )}
          </div>

          <div className="divide-y divide-slate-200 dark:divide-slate-800">
            {visibleNodes.length === 0 ? (
              <div className="p-8 text-center text-xs text-slate-500">
                Thư mục trống. Nhấn &quot;Tải lên .ZIP &amp; Giải nén&quot; hoặc
                &quot;Tạo Tệp&quot; để thêm mã nguồn.
              </div>
            ) : (
              visibleNodes.map((node) => {
                const isSelected = activeFile?.id === node.id;
                const isArchive =
                  node.type === 'file' && isArchiveFile(node.name);
                return (
                  <div
                    key={node.id}
                    onClick={() => handleSelectFile(node)}
                    className={`px-4 py-3 flex items-center justify-between gap-3 transition-colors cursor-pointer ${
                      isSelected
                        ? 'bg-orange-50/70 dark:bg-orange-950/25'
                        : 'hover:bg-slate-50 dark:hover:bg-slate-800/40'
                    }`}
                  >
                    <div className="flex items-center gap-3 min-w-0">
                      {node.type === 'directory' ? (
                        <Folder className="w-4 h-4 text-amber-500 shrink-0" />
                      ) : isArchive ? (
                        <FileArchive className="w-4 h-4 text-orange-500 shrink-0" />
                      ) : (
                        <FileCode className="w-4 h-4 text-sky-600 dark:text-sky-400 shrink-0" />
                      )}
                      <div className="min-w-0">
                        <div className="font-mono text-xs font-semibold text-slate-900 dark:text-slate-100 truncate">
                          {node.name}
                          {node.type === 'directory' ? '/' : ''}
                        </div>
                        <div className="text-xs font-mono tabular-nums text-slate-500 dark:text-slate-400">
                          {node.permissions} ({node.symbolicPerms}) ·{' '}
                          {node.owner}
                        </div>
                      </div>
                    </div>

                    <div
                      className="flex items-center gap-1.5 shrink-0"
                      onClick={(e) => e.stopPropagation()}
                    >
                      {isArchive && (
                        <button
                          onClick={() => handleExtractArchiveNode(node)}
                          className="px-2 py-1 text-xs font-medium text-emerald-700 dark:text-emerald-300 bg-emerald-500/10 hover:bg-emerald-500/20 rounded inline-flex items-center gap-1 cursor-pointer"
                          title="Giải nén tệp .zip này vào thư mục hiện tại"
                        >
                          <PackageOpen className="w-3.5 h-3.5" />
                          Giải nén
                        </button>
                      )}
                      <span className="text-xs font-mono tabular-nums text-slate-500">
                        {node.type === 'directory'
                          ? 'DIR'
                          : node.sizeBytes > 10240
                            ? `${(node.sizeBytes / 1024).toFixed(1)} KB`
                            : `${node.sizeBytes.toLocaleString()} B`}
                      </span>
                      <button
                        onClick={() => {
                          setRenamingNode(node);
                          setRenameValue(node.name);
                        }}
                        className="p-1 text-slate-400 hover:text-sky-500 transition-colors cursor-pointer"
                        title="Đổi tên tệp / thư mục"
                      >
                        <Edit3 className="w-3.5 h-3.5" />
                      </button>
                      <button
                        onClick={() => handleDeleteNode(node.id)}
                        className="p-1 text-slate-400 hover:text-rose-600 dark:hover:text-rose-400 transition-colors cursor-pointer"
                        title="Xóa tệp / thư mục"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </div>

        {/* Right: Integrated File Editor & Archive Inspector */}
        <div className="lg:col-span-7 rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden">
          {activeFile ? (
            <>
              <div className="px-5 py-4 border-b border-slate-200 dark:border-slate-800 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                <div className="space-y-0.5">
                  <div className="font-mono text-xs font-bold text-slate-900 dark:text-slate-100">
                    {activeApp?.workingDir}
                    {activeFile.path}
                  </div>
                  <div className="text-xs font-mono tabular-nums text-slate-500 dark:text-slate-400">
                    Kích thước: {activeFile.sizeBytes.toLocaleString()} Bytes ·
                    Sửa lần cuối: {activeFile.modifiedAt}
                  </div>
                </div>

                <div className="flex flex-wrap items-center gap-2 shrink-0">
                  {isArchiveFile(activeFile.name) && (
                    <button
                      onClick={() => handleExtractArchiveNode(activeFile)}
                      className="px-3 py-1.5 text-xs font-medium text-white bg-emerald-600 hover:bg-emerald-500 rounded-lg inline-flex items-center gap-1.5 cursor-pointer"
                    >
                      <PackageOpen className="w-3.5 h-3.5" />
                      Giải nén vào {currentDir}
                    </button>
                  )}

                  <div className="flex items-center gap-1.5">
                    <Lock className="w-3.5 h-3.5 text-slate-400" />
                    <select
                      value={editorPerms}
                      onChange={(e) => setEditorPerms(e.target.value)}
                      className="px-2.5 py-1.5 text-xs font-mono rounded-lg border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-100"
                    >
                      <option value="0644">chmod 0644</option>
                      <option value="0640">chmod 0640</option>
                      <option value="0600">chmod 0600</option>
                      <option value="0755">chmod 0755</option>
                    </select>
                  </div>

                  <button
                    onClick={handleDownloadFile}
                    className="px-3 py-1.5 text-xs font-medium text-slate-700 dark:text-slate-200 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1 cursor-pointer"
                  >
                    <Download className="w-3.5 h-3.5" />
                    Tải về
                  </button>

                  <button
                    onClick={handleSaveFile}
                    disabled={saving}
                    className="px-3.5 py-1.5 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg transition-colors whitespace-nowrap shrink-0 inline-flex items-center gap-1.5 cursor-pointer"
                  >
                    {savedToast ? (
                      <>
                        <Check className="w-3.5 h-3.5" />
                        Đã lưu tệp
                      </>
                    ) : (
                      <>
                        <Save className="w-3.5 h-3.5" />
                        {saving ? 'Đang lưu...' : 'Lưu & Áp dụng chmod'}
                      </>
                    )}
                  </button>
                </div>
              </div>

              <textarea
                value={editorContent}
                onChange={(e) => setEditorContent(e.target.value)}
                rows={20}
                spellCheck={false}
                className="w-full p-5 bg-slate-950 text-slate-100 font-mono text-xs leading-relaxed focus:outline-none resize-y"
              />
            </>
          ) : (
            <div className="p-12 text-center text-xs text-slate-500">
              Chọn một tệp tin ở danh sách bên trái để xem hoặc chỉnh sửa nội
              dung.
            </div>
          )}
        </div>
      </div>

      {/* Upload .ZIP & Instant Extract Modal */}
      {isZipModalOpen && activeApp && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/75 backdrop-blur-xs p-4">
          <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl max-w-2xl w-full overflow-hidden shadow-2xl">
            <div className="px-6 py-4 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
              <div className="flex items-center gap-2.5">
                <FileArchive className="w-5 h-5 text-orange-500" />
                <div>
                  <h2 className="text-base font-bold text-slate-900 dark:text-slate-100 font-display">
                    Tải lên Tệp nén .ZIP &amp; Giải nén Mã nguồn
                  </h2>
                  <p className="text-xs font-mono text-slate-500">
                    Thư mục đích: {activeApp.workingDir}
                    {currentDir} (Zip-Slip Guard Active)
                  </p>
                </div>
              </div>
              <button
                onClick={() => setIsZipModalOpen(false)}
                className="p-1.5 text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 rounded-lg cursor-pointer"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="p-6 space-y-5">
              {/* Option to auto-extract */}
              <label className="flex items-center justify-between p-3.5 rounded-xl border border-orange-500/30 bg-orange-500/5 cursor-pointer">
                <div>
                  <div className="text-xs font-bold text-slate-900 dark:text-slate-100">
                    Tự động giải nén ngay sau khi tải lên (Auto-Unzip &amp;
                    chown www-data)
                  </div>
                  <div className="text-xs text-slate-500">
                    Giải nén toàn bộ cây thư mục bên trong file .zip vào{' '}
                    {activeApp.workingDir}
                    {currentDir}
                  </div>
                </div>
                <input
                  type="checkbox"
                  checked={extractImmediately}
                  onChange={(e) => setExtractImmediately(e.target.checked)}
                  className="rounded text-orange-600"
                />
              </label>

              {/* Browser File Picker Dropzone */}
              <div className="border-2 border-dashed border-slate-300 dark:border-slate-700 rounded-xl p-6 text-center space-y-3 hover:border-orange-500 transition-colors">
                <Upload className="w-8 h-8 text-orange-500 mx-auto" />
                <div className="space-y-1">
                  <div className="text-xs font-semibold text-slate-900 dark:text-slate-100">
                    Chọn tệp <code className="font-mono">.zip</code> hoặc tệp mã
                    nguồn từ máy tính của bạn
                  </div>
                  <div className="text-xs text-slate-500">
                    Hỗ trợ đọc cấu trúc ZIP trực tiếp và giải nén toàn bộ
                    tệp/thư mục con
                  </div>
                </div>
                <input
                  ref={fileInputRef}
                  type="file"
                  multiple
                  accept=".zip,.tar.gz,.php,.rs,.js,.ts,.json,.html,.css,.env,.toml,.sql,*/*"
                  onChange={handleBrowserFileUpload}
                  className="hidden"
                />
                <button
                  type="button"
                  disabled={uploadingZip}
                  onClick={() => fileInputRef.current?.click()}
                  className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg inline-flex items-center gap-1.5 cursor-pointer disabled:opacity-50"
                >
                  <Upload className="w-3.5 h-3.5" />
                  {uploadingZip
                    ? 'Đang xử lý & Giải nén...'
                    : 'Chọn Tệp .ZIP / Mã nguồn từ Máy tính'}
                </button>
              </div>

            </div>
          </div>
        </div>
      )}

      {/* Remote URL Download Modal (wget/curl) */}
      {isRemoteUrlOpen && activeApp && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/75 backdrop-blur-xs p-4">
          <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl max-w-lg w-full overflow-hidden shadow-2xl">
            <div className="px-6 py-4 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
              <div className="flex items-center gap-2">
                <Globe className="w-4 h-4 text-sky-500" />
                <h2 className="text-base font-bold text-slate-900 dark:text-slate-100 font-display">
                  Tải Tệp / Gói .ZIP từ URL Từ xa (Remote Download)
                </h2>
              </div>
              <button
                onClick={() => setIsRemoteUrlOpen(false)}
                className="p-1.5 text-slate-400 hover:text-slate-600 rounded-lg cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <form onSubmit={handleRemoteDownload} className="p-6 space-y-4">
              <div className="space-y-1.5">
                <label className="text-xs font-medium text-slate-700 dark:text-slate-300">
                  URL Nguồn (HTTP / HTTPS)
                </label>
                <input
                  type="url"
                  required
                  value={remoteUrl}
                  onChange={(e) => setRemoteUrl(e.target.value)}
                  placeholder="https://wordpress.org/latest.zip"
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                />
              </div>

              <div className="space-y-1.5">
                <label className="text-xs font-medium text-slate-700 dark:text-slate-300">
                  Lưu với tên tệp tại {activeApp.workingDir}
                  {currentDir}
                </label>
                <input
                  type="text"
                  required
                  value={remoteFileName}
                  onChange={(e) => setRemoteFileName(e.target.value)}
                  placeholder="wordpress-latest.zip"
                  className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
                />
              </div>

              <div className="flex items-center justify-end gap-2 pt-2 border-t border-slate-200 dark:border-slate-800">
                <button
                  type="button"
                  onClick={() => setIsRemoteUrlOpen(false)}
                  className="px-4 py-2 text-xs font-medium text-slate-600 dark:text-slate-400 rounded-lg cursor-pointer"
                >
                  Hủy
                </button>
                <button
                  type="submit"
                  disabled={downloadingRemote}
                  className="px-4 py-2 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg inline-flex items-center gap-1.5 cursor-pointer disabled:opacity-50"
                >
                  <Download className="w-3.5 h-3.5" />
                  {downloadingRemote
                    ? 'Đang tải về Server...'
                    : 'Bắt đầu Tải về'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Rename Modal */}
      {renamingNode && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/75 backdrop-blur-xs p-4">
          <div className="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-2xl max-w-md w-full overflow-hidden shadow-2xl">
            <div className="px-6 py-4 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between">
              <h2 className="text-sm font-bold text-slate-900 dark:text-slate-100 font-display">
                Đổi tên: {renamingNode.name}
              </h2>
              <button
                onClick={() => setRenamingNode(null)}
                className="p-1 text-slate-400 hover:text-slate-600 cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <form onSubmit={handleConfirmRename} className="p-6 space-y-4">
              <input
                type="text"
                required
                value={renameValue}
                onChange={(e) => setRenameValue(e.target.value)}
                className="w-full px-3 py-2 text-xs font-mono rounded-lg border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-950"
              />
              <div className="flex justify-end gap-2">
                <button
                  type="button"
                  onClick={() => setRenamingNode(null)}
                  className="px-3 py-1.5 text-xs text-slate-500 cursor-pointer"
                >
                  Hủy
                </button>
                <button
                  type="submit"
                  className="px-4 py-1.5 text-xs font-medium text-white bg-orange-600 hover:bg-orange-500 rounded-lg cursor-pointer"
                >
                  Lưu tên mới
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
