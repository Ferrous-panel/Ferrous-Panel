import React from 'react';
import {
  Activity,
  Code2,
  Database,
  FileCode2,
  FolderKanban,
  Gauge,
  Globe,
  Mail,
  Rocket,
  ShieldCheck,
  Terminal,
  Wrench
} from 'lucide-react';
import { ActiveTab } from '../types';

export interface SidebarNavItem {
  id: ActiveTab;
  label: string;
  icon: React.FC<{ className?: string }>;
}

export function getSidebarItems(
  tr: (viText: string, enText: string) => string
): SidebarNavItem[] {
  return [
    {
      id: 'overview',
      label: tr('Trang chủ Panel', 'Panel Overview'),
      icon: Activity
    },
    {
      id: 'websites',
      label: tr('Websites & Modern JS', 'Websites & Modern JS'),
      icon: Globe
    },
    {
      id: 'php',
      label: tr('Multi-PHP (7.4-8.4) & INI', 'Multi-PHP (7.4-8.4) & INI'),
      icon: FileCode2
    },
    {
      id: 'files',
      label: tr('File Manager & Giải nén ZIP', 'File Manager & ZIP Archive'),
      icon: FolderKanban
    },
    {
      id: 'ssh',
      label: tr('Web SSH & Panel CLI (fp)', 'Web SSH & Panel CLI (fp)'),
      icon: Terminal
    },
    {
      id: 'oneclick',
      label: tr('Package Rust & 1-Click', 'Rust Packages & 1-Click'),
      icon: Rocket
    },
    {
      id: 'databases',
      label: tr('MySQL phpMyAdmin & SQL', 'MySQL phpMyAdmin & SQL'),
      icon: Database
    },
    {
      id: 'mail',
      label: tr('Mail Server & Webmail', 'Mail Server & Webmail'),
      icon: Mail
    },
    {
      id: 'optimizer',
      label: tr('Models Tối ưu & Watchdog', 'System Optimizer & Watchdog'),
      icon: Gauge
    },
    {
      id: 'cpanel',
      label: tr('Cron, DNS, Backup & FTP', 'Cron, DNS, Backup & FTP'),
      icon: Wrench
    },
    {
      id: 'security',
      label: tr('Tường lửa & Bảo mật', 'Firewall & Security Center'),
      icon: ShieldCheck
    },
    {
      id: 'rust-source',
      label: tr('Nhân Rust & Đa Ngôn Ngữ', 'Rust Core & Polyglot FFI'),
      icon: Code2
    }
  ];
}
