import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';

const raw = readFileSync(resolve(__dirname, '../../src/styles/globals.css'), 'utf8');

/** 递归列出组件源码（只收 .tsx/.ts）。 */
function componentFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) return componentFiles(full);
    return full.endsWith('.tsx') || full.endsWith('.ts') ? [full] : [];
  });
}

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
      '--app-warning-fg',
      '--app-warning-text',
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

  it('内容区允许选中复制', () => {
    expect(declarations('.path-text')).toContain('user-select: text');
    expect(declarations('.env-var-cell')).toContain('user-select: text');
  });

  it('文字不使用低对比前景色（subtle 4.02:1 / faint 2.46:1）', () => {
    expect(css).not.toMatch(/color:\s*var\(--app-fg-subtle\)/);
    expect(css).not.toMatch(/color:\s*var\(--app-fg-faint\)/);

    const offenders: string[] = [];
    for (const file of componentFiles(resolve(__dirname, '../../src/components'))) {
      const text = readFileSync(file, 'utf8');
      for (const token of ['--app-fg-subtle', '--app-fg-faint']) {
        if (new RegExp(`color:\\s*['"]var\\(${token}\\)`).test(text)) {
          offenders.push(`${file}:${token}`);
        }
      }
    }
    expect(offenders, offenders.join('; ')).toEqual([]);
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

/** 取某个选择器的第一个声明块（不含嵌套块）。 */
function declarations(selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = css.match(new RegExp(`${escaped}\\s*\\{([\\s\\S]*?)\\}`));
  if (!match) throw new Error(`缺少 ${selector} 声明块`);
  return match[1];
}

/** 从声明块里取十六进制 token 值。 */
function hexToken(block: string, name: string): string {
  const match = block.match(new RegExp(`${name}\\s*:\\s*(#[0-9a-fA-F]{6})`));
  if (!match) throw new Error(`缺少 ${name}`);
  return match[1];
}

/** WCAG 相对亮度。 */
function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG 对比度。 */
function contrast(fg: string, bg: string): number {
  const a = luminance(fg);
  const b = luminance(bg);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

describe('语义色对比度（WCAG AA 4.5:1）', () => {
  const light = declarations(':root');
  const dark = declarations(':root.dark');

  it('警告底文字与警告正文在两套主题下都达标', () => {
    const pairs: [string, string, string, string][] = [
      ['浅色 警告底文字', light, '--app-warning-fg', '--app-warning'],
      ['深色 警告底文字', dark, '--app-warning-fg', '--app-warning'],
      ['浅色 警告正文', light, '--app-warning-text', '--app-bg'],
      ['深色 警告正文', dark, '--app-warning-text', '--app-bg'],
      ['浅色 次要文字', light, '--app-fg-muted', '--app-bg'],
      ['深色 次要文字', dark, '--app-fg-muted', '--app-bg'],
    ];
    for (const [label, block, fg, bg] of pairs) {
      const ratio = contrast(hexToken(block, fg), hexToken(block, bg));
      expect(ratio, `${label} 对比度 ${ratio.toFixed(2)}:1`).toBeGreaterThanOrEqual(4.5);
    }
  });
});
