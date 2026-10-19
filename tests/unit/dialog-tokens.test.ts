import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';

const ROOT = resolve(__dirname, '../../src/components/dialogs');

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) return walk(full);
    return full.endsWith('.tsx') ? [full] : [];
  });
}

const files = walk(ROOT);
/**
 * 剥离注释后再匹配 —— 与 design-tokens.test.ts 的 B4 口径保持一致。
 * 不剥离时，注释里写「原为 #3b82f6」这类说明会造成假红。
 * 注：计划原文的转义在文档传输中被吞（变成无效语法），此处按意图重写。
 */
const read = (f: string) =>
  readFileSync(f, 'utf8')
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/\/\/[^\n]*/g, '');

const LEGACY_TOKENS = ['--app-list-bg', '--app-list-alt', '--app-select-row'];
const LEGACY_APIS = ['btnClass', 'btnStyle', 'tab-active'];
const HARDCODED_COLOR = /#[0-9a-fA-F]{6}\b|rgba?\(/;

describe('对话框不依赖旧样式设施（Wave 4.1 验收）', () => {
  it('扫描到全部 14 个对话框文件', () => {
    expect(files.length).toBeGreaterThanOrEqual(14);
  });

  it.each([...LEGACY_TOKENS, ...LEGACY_APIS])('不使用 %s', (token) => {
    const offenders = files.filter((f) => read(f).includes(token));
    expect(offenders, `${token} 仍出现在: ${offenders.join(', ')}`).toEqual([]);
  });

  it('不含硬编码色值', () => {
    const offenders = files.filter((f) => HARDCODED_COLOR.test(read(f)));
    expect(offenders, `硬编码色值仍出现在: ${offenders.join(', ')}`).toEqual([]);
  });
});
