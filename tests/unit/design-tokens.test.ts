import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const raw = readFileSync(resolve(__dirname, '../../src/styles/globals.css'), 'utf8');

/**
 * 剥离 CSS 注释后再匹配。
 * 不剥离时：注释里提到「已删除 --color-path-invalid」会假红；
 *          把死变量注释掉而不删会假绿。
 */
const css = raw.replace(/\/\*[\s\S]*?\*\//g, '');

function declares(name: string): boolean {
  // 只匹配「行首冒号」形式的声明，避免匹配到 var(--x) 的引用
  return new RegExp(`^\\s*${name}\\s*:`, 'm').test(css);
}

describe('设计 token 层', () => {
  it('声明全部语义变量', () => {
    for (const v of [
      '--app-bg',
      '--app-surface',
      '--app-surface-raised',
      '--app-border',
      '--app-border-strong',
      '--app-fg',
      '--app-fg-muted',
      '--app-fg-faint',
      '--app-accent',
      '--app-accent-fg',
      '--app-accent-bg',
      '--app-row-even',
      '--app-row-odd',
      '--app-row-selected',
      '--app-hover',
      '--app-danger',
      '--app-warning',
      '--app-disabled-fg',
      '--app-success',
      '--app-overlay',
      '--app-dialog-shadow',
    ]) {
      expect(declares(v), `缺少 ${v}`).toBe(true);
    }
  });

  it('声明几何变量', () => {
    for (const v of ['--radius-sm', '--radius', '--radius-lg', '--row-h', '--gap']) {
      expect(declares(v), `缺少 ${v}`).toBe(true);
    }
  });

  it('声明对话框专用工具类', () => {
    for (const cls of [
      '.field-input',
      '.dialog-list-item',
      '.btn-block',
      '.modal-overlay',
      '.modal-surface',
    ]) {
      expect(css.includes(cls), `缺少 ${cls}`).toBe(true);
    }
  });

  it('过渡别名已删除', () => {
    for (const v of ['--app-list-bg', '--app-list-alt', '--app-select-row']) {
      expect(declares(v), `过渡别名 ${v} 应已删除`).toBe(false);
    }
  });

  it('.tab-active 已删除，.tab-pill 保留', () => {
    expect(css).not.toMatch(/\.tab-active\s*\{/);
    expect(css).toMatch(/\.tab-pill\s*\{/);
  });

  it('不包含死代码', () => {
    for (const dead of [
      '.row-even',
      '.row-odd',
      '.btn-disabled',
      '.text-invalid',
      '.text-duplicate',
    ]) {
      expect(css.includes(dead), `死代码 ${dead} 未清理`).toBe(false);
    }
    for (const dead of ['--color-path-invalid', '--color-path-duplicate', '--color-path-valid']) {
      expect(css.includes(dead), `死变量 ${dead} 未清理`).toBe(false);
    }
  });
});
