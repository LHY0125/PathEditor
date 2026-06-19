import { describe, it, expect } from 'vitest';
import { importFromCsv, importFromJson, importFromTxt } from '../../src/core/import-export';

describe('导入一致性（TS 端）', () => {
  it('JSON 含 system + user', () => {
    const json = JSON.stringify({ system: ['C:\\a', 'C:\\b'], user: ['D:\\c'] });
    const r = importFromJson(json);
    expect(r.system.map((e) => e.path)).toEqual(['C:\\a', 'C:\\b']);
    expect(r.user.map((e) => e.path)).toEqual(['D:\\c']);
  });

  it('CSV system/user 分类', () => {
    const csv = 'type,path\nsystem,C:\\sys\nuser,D:\\usr\n';
    const r = importFromCsv(csv);
    expect(r.system.map((e) => e.path)).toEqual(['C:\\sys']);
    expect(r.user.map((e) => e.path)).toEqual(['D:\\usr']);
  });

  it('CSV 含 BOM + header', () => {
    const csv = '﻿type,path\nsystem,C:\\x\n';
    const r = importFromCsv(csv);
    expect(r.system.map((e) => e.path)).toEqual(['C:\\x']);
  });

  it('TXT 逐行读取，跳过注释', () => {
    const txt = '# comment\nC:\\a\n\nD:\\b\n';
    const r = importFromTxt(txt);
    expect(r.map((e) => e.path)).toEqual(['C:\\a', 'D:\\b']);
  });

  it('JSON 空数据不崩溃', () => {
    const r = importFromJson('{}');
    expect(r.system).toEqual([]);
    expect(r.user).toEqual([]);
  });
});
