import { describe, it, expect } from 'vitest';
import {
  exportToJson,
  exportToCsv,
  importFromJson,
  importFromCsv,
  importFromTxt,
  importFromContent,
  detectExportFormat,
  flattenImportResult,
} from '../../src/core/import-export';
import type { PathEntry } from '../../src/core/path-entry';

function pe(s: string, enabled: boolean = true): PathEntry {
  return { path: s, enabled };
}

const sampleData = {
  system: [pe('C:\\Windows'), pe('C:\\Program Files')],
  user: [pe('C:\\Users\\me\\AppData')],
};

describe('exportToJson', () => {
  it('导出结构化 JSON', () => {
    const json = exportToJson({
      system: sampleData.system.map(e => e.path),
      user: sampleData.user.map(e => e.path),
    });
    const parsed = JSON.parse(json);
    expect(parsed.version).toBe('1.0');
    expect(parsed.type).toBe('PathEditor');
    expect(parsed.system).toEqual(sampleData.system.map(e => e.path));
    expect(parsed.user).toEqual(sampleData.user.map(e => e.path));
    expect(parsed.exported).toBeDefined();
  });
});

describe('importFromJson', () => {
  it('正确导入 JSON', () => {
    const json = JSON.stringify({
      system: sampleData.system.map(e => e.path),
      user: sampleData.user.map(e => e.path),
    });
    const result = importFromJson(json);
    expect(result.system).toEqual(sampleData.system.map(e => e.path));
    expect(result.user).toEqual(sampleData.user.map(e => e.path));
  });

  it('过滤空字符串', () => {
    const json = JSON.stringify({ system: ['C:\\', '', '  '], user: [] });
    const result = importFromJson(json);
    expect(result.system).toEqual(['C:\\']);
  });
});

describe('exportToCsv', () => {
  it('导出 CSV 含 BOM', () => {
    const csv = exportToCsv({
      system: sampleData.system.map(e => e.path),
      user: sampleData.user.map(e => e.path),
    });
    expect(csv.startsWith('﻿')).toBe(true);
    expect(csv).toContain('type,path');
    expect(csv).toContain('system,C:\\Windows');
    expect(csv).toContain('user,C:\\Users\\me\\AppData');
  });

  it('CSV 字段转义', () => {
    const data = { system: ['C:\\Path,with,commas'], user: [] };
    const csv = exportToCsv(data);
    expect(csv).toContain('"C:\\Path,with,commas"');
  });

  it('CSV 双引号转义', () => {
    const data = { system: ['Path with "quotes"'], user: [] };
    const csv = exportToCsv(data);
    expect(csv).toContain('"Path with ""quotes"""');
  });
});

describe('importFromCsv', () => {
  it('正确导入 CSV', () => {
    const csv = '﻿type,path\nsystem,C:\\Windows\nuser,C:\\AppData\n';
    const result = importFromCsv(csv);
    expect(result.system).toEqual(['C:\\Windows']);
    expect(result.user).toEqual(['C:\\AppData']);
  });

  it('跳过未知类型', () => {
    const csv = 'type,path\nother,C:\\Unknown\nsystem,C:\\Valid';
    const result = importFromCsv(csv);
    expect(result.system.length).toBe(1);
    expect(result.user.length).toBe(0);
  });

  it('处理带引号的 CSV 字段', () => {
    const csv = 'type,path\nsystem,"C:\\Path,With,Commas"';
    const result = importFromCsv(csv);
    expect(result.system).toEqual(['C:\\Path,With,Commas']);
  });
});

describe('importFromTxt', () => {
  it('逐行导入，跳过注释和空行', () => {
    const txt = '# 这是注释\nC:\\Windows\n\nD:\\Projects\n# 另一个注释';
    const paths = importFromTxt(txt);
    expect(paths).toEqual(['C:\\Windows', 'D:\\Projects']);
  });

  it('跳过 BOM', () => {
    const txt = '﻿C:\\Windows';
    const paths = importFromTxt(txt);
    expect(paths).toEqual(['C:\\Windows']);
  });
});

describe('importFromContent', () => {
  it('根据扩展名选择格式', () => {
    const csvContent = 'type,path\nsystem,C:\\Test';
    const jsonContent = JSON.stringify({ system: ['C:\\Test'], user: [] });
    const txtContent = 'C:\\Test';

    expect(importFromContent(csvContent, 'test.csv').system).toEqual(['C:\\Test']);
    expect(importFromContent(jsonContent, 'test.json').system).toEqual(['C:\\Test']);
    expect(importFromContent(txtContent, 'test.txt').system).toEqual(['C:\\Test']);
  });
});

describe('detectExportFormat', () => {
  it('.csv 检测为 CSV', () => {
    expect(detectExportFormat('data.CSV')).toBe('csv');
  });
  it('.txt 检测为 TXT', () => {
    expect(detectExportFormat('data.txt')).toBe('txt');
  });
  it('其他扩展名检测为 JSON', () => {
    expect(detectExportFormat('data.json')).toBe('json');
  });
});

describe('flattenImportResult', () => {
  const data = { system: ['S1'], user: ['U1'] };

  it('仅系统', () => {
    const r = flattenImportResult(data, 'system');
    expect(r.system).toEqual(['S1']);
    expect(r.user).toEqual([]);
  });

  it('仅用户', () => {
    const r = flattenImportResult(data, 'user');
    expect(r.system).toEqual([]);
    expect(r.user).toEqual(['U1']);
  });

  it('两者都导入', () => {
    const r = flattenImportResult(data, 'both');
    expect(r.system).toEqual(['S1']);
    expect(r.user).toEqual(['U1']);
  });
});
