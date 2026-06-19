/**
 * 导入导出模块 — 支持 JSON、CSV、TXT 三种格式
 *
 * 注意：Rust 端 core/src/fs.rs 有对应的导入导出实现，
 * 前端使用此模块（需 ImportDialog 交互），CLI 使用 Rust 版，修改时需同步两端。
 */
import { version } from '../../package.json';
import type { PathEntry } from './path-entry';

export type ExportFormat = 'json' | 'csv' | 'txt';

export interface ExportData {
  system: PathEntry[];
  user: PathEntry[];
}

/** 根据文件扩展名检测格式 */
export function detectExportFormat(filepath: string): ExportFormat {
  const lower = filepath.toLowerCase();
  if (lower.endsWith('.csv')) return 'csv';
  if (lower.endsWith('.txt')) return 'txt';
  return 'json';
}

// ── JSON 导出 ──

export function exportToJson(data: ExportData): string {
  const obj = {
    version,
    timestamp: new Date().toISOString(),
    system: data.system.map((e) => ({ path: e.path, enabled: e.enabled })),
    user: data.user.map((e) => ({ path: e.path, enabled: e.enabled })),
  };
  return JSON.stringify(obj, null, 2);
}

// ── CSV 导出 ──

export function exportToCsv(data: ExportData): string {
  const lines: string[] = [];
  // UTF-8 BOM
  lines.push('﻿type,path,enabled');

  for (const entry of data.system) {
    lines.push(`system,${escapeCsvField(entry.path)},${entry.enabled}`);
  }
  for (const entry of data.user) {
    lines.push(`user,${escapeCsvField(entry.path)},${entry.enabled}`);
  }

  return lines.join('\n') + '\n';
}

function escapeCsvField(field: string): string {
  if (field.includes(',') || field.includes('"') || field.includes('\n')) {
    return `"${field.replace(/"/g, '""')}"`;
  }
  return field;
}

// ── CSV 导入 ──

export interface ImportResult {
  system: PathEntry[];
  user: PathEntry[];
}

export function importFromCsv(content: string): ImportResult {
  const result: ImportResult = { system: [], user: [] };
  const lines = content.split(/\r?\n/);

  let hasHeader = false;

  for (let i = 0; i < lines.length; i++) {
    // 跳过 BOM（仅首行）
    let line = lines[i];
    if (i === 0 && line.startsWith('﻿')) {
      line = line.slice(1);
    }

    if (line.trim() === '') continue;

    const fields = parseCsvLine(line);
    if (fields.length < 2) continue;

    // 检测头部行
    if (!hasHeader && isHeaderRow(fields[0], fields[1])) {
      hasHeader = true;
      continue;
    }

    const type = fields[0].trim().toLowerCase();
    const path = fields[1].trim();

    if (path.length === 0) continue;

    // 第三列 enabled（可选，默认 true）
    const enabled = fields.length >= 3 ? fields[2].trim().toLowerCase() !== 'false' : true;

    if (type === 'system') {
      result.system.push({ path, enabled });
    } else if (type === 'user') {
      result.user.push({ path, enabled });
    }
    // 未知类型忽略
  }

  return result;
}

function isHeaderRow(col0: string, col1: string): boolean {
  const c0 = col0.trim().toLowerCase();
  const c1 = col1.trim().toLowerCase();
  return c0 === 'type' && c1 === 'path';
}

function parseCsvLine(line: string): string[] {
  const fields: string[] = [];
  let current = '';
  let inQuotes = false;

  for (let i = 0; i < line.length; i++) {
    const ch = line[i];

    if (inQuotes) {
      if (ch === '"') {
        if (i + 1 < line.length && line[i + 1] === '"') {
          current += '"';
          i++; // 跳过转义引号
        } else {
          inQuotes = false;
        }
      } else {
        current += ch;
      }
    } else {
      if (ch === '"') {
        inQuotes = true;
      } else if (ch === ',') {
        fields.push(current);
        current = '';
      } else {
        current += ch;
      }
    }
  }

  fields.push(current);
  return fields;
}

// ── JSON 导入 ──

export function importFromJson(content: string): ImportResult {
  const result: ImportResult = { system: [], user: [] };

  let obj: Record<string, unknown>;
  try {
    obj = JSON.parse(content);
  } catch {
    return result;
  }

  if (typeof obj !== 'object' || obj === null) return result;

  const parseEntry = (item: unknown): { path: string; enabled: boolean } | null => {
    if (typeof item === 'string') {
      const trimmed = item.trim();
      return trimmed.length > 0 ? { path: trimmed, enabled: true } : null;
    }
    if (typeof item === 'object' && item !== null) {
      const rec = item as Record<string, unknown>;
      const path = typeof rec.path === 'string' ? rec.path.trim() : '';
      if (path.length === 0) return null;
      const enabled = typeof rec.enabled === 'boolean' ? rec.enabled : true;
      return { path, enabled };
    }
    return null;
  };

  if (Array.isArray(obj.system)) {
    result.system = obj.system
      .map(parseEntry)
      .filter((e): e is { path: string; enabled: boolean } => e !== null);
  }
  if (Array.isArray(obj.user)) {
    result.user = obj.user
      .map(parseEntry)
      .filter((e): e is { path: string; enabled: boolean } => e !== null);
  }

  return result;
}

// ── TXT 导入 ──

export function importFromTxt(content: string): PathEntry[] {
  const paths: PathEntry[] = [];
  const lines = content.split(/\r?\n/);

  for (let i = 0; i < lines.length; i++) {
    // 跳过 BOM（仅首行）
    let line = lines[i];
    if (i === 0 && line.startsWith('﻿')) line = line.slice(1);

    const trimmed = line.trim();
    if (trimmed.length === 0 || trimmed.startsWith('#')) continue;

    paths.push({ path: trimmed, enabled: true });
  }

  return paths;
}

// ── 自动检测导入 ──

export function importFromContent(content: string, filepath: string): ImportResult {
  const lower = filepath.toLowerCase();
  if (lower.endsWith('.csv')) {
    return importFromCsv(content);
  } else if (lower.endsWith('.json')) {
    return importFromJson(content);
  } else if (lower.endsWith('.txt')) {
    return { system: importFromTxt(content), user: [] };
  } else {
    throw new Error(`不支持的导入格式: ${filepath}`);
  }
}

/** 将 ImportResult 合并为单个路径数组 */
export function flattenImportResult(
  result: ImportResult,
  target: 'system' | 'user' | 'both',
): ExportData {
  if (target === 'system') return { system: result.system, user: [] };
  if (target === 'user') return { system: [], user: result.user };
  return result; // both
}
