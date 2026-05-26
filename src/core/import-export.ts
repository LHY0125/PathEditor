/**
 * 导入导出模块 — 对应 C 版 import_export.c
 * 支持 JSON、CSV、TXT 三种格式
 */
export type ExportFormat = 'json' | 'csv';

export interface ExportData {
  system: string[];
  user: string[];
}

/** 根据文件扩展名检测格式 */
export function detectExportFormat(filepath: string): ExportFormat {
  if (filepath.toLowerCase().endsWith('.csv')) return 'csv';
  return 'json';
}

// ── JSON 导出 ──

export function exportToJson(data: ExportData): string {
  const obj = {
    version: '1.0',
    type: 'PathEditor',
    exported: new Date().toISOString(),
    system: data.system,
    user: data.user,
  };
  return JSON.stringify(obj, null, 2);
}

// ── CSV 导出 ──

export function exportToCsv(data: ExportData): string {
  const lines: string[] = [];
  // UTF-8 BOM
  lines.push('﻿type,path');

  for (const path of data.system) {
    lines.push(`system,${escapeCsvField(path)}`);
  }
  for (const path of data.user) {
    lines.push(`user,${escapeCsvField(path)}`);
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
  system: string[];
  user: string[];
}

export function importFromCsv(content: string): ImportResult {
  const result: ImportResult = { system: [], user: [] };
  const lines = content.split(/\r?\n/);

  let hasHeader = false;

  for (const rawLine of lines) {
    // 跳过 BOM
    let line = rawLine;
    if (line.startsWith('﻿')) {
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

    if (type === 'system') {
      result.system.push(path);
    } else if (type === 'user') {
      result.user.push(path);
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
    return result; // 无效 JSON 返回空结果，由调用方显示错误
  }

  if (typeof obj !== 'object' || obj === null) return result;

  if (Array.isArray(obj.system)) {
    result.system = obj.system.filter(
      (p: unknown) => typeof p === 'string' && p.trim().length > 0,
    );
  }
  if (Array.isArray(obj.user)) {
    result.user = obj.user.filter(
      (p: unknown) => typeof p === 'string' && p.trim().length > 0,
    );
  }

  return result;
}

// ── TXT 导入 ──

export function importFromTxt(content: string): string[] {
  const paths: string[] = [];
  const lines = content.split(/\r?\n/);

  for (let line of lines) {
    // 跳过 BOM
    if (line.startsWith('﻿')) line = line.slice(1);

    const trimmed = line.trim();
    if (trimmed.length === 0 || trimmed.startsWith('#')) continue;

    paths.push(trimmed);
  }

  return paths;
}

// ── 自动检测导入 ──

export function importFromContent(
  content: string,
  filepath: string,
): ImportResult {
  const lower = filepath.toLowerCase();
  if (lower.endsWith('.csv')) {
    return importFromCsv(content);
  } else if (lower.endsWith('.json')) {
    return importFromJson(content);
  } else {
    // TXT 文件：所有路径放入 system（用户后续可选择目标）
    return { system: importFromTxt(content), user: [] };
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
