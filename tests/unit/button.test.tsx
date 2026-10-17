import { describe, it, expect, afterEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/react';
import { Button } from '@/components/ui/Button';
import { buttonVariants, btnClass, btnStyle } from '@/components/ui/buttons';

// 本仓库未开启 vitest globals，RTL 的自动 cleanup 不会生效（同 env-var-toolbar.test.tsx）。
afterEach(() => {
  cleanup();
});

describe('buttonVariants', () => {
  it('每个变体返回不同 class', () => {
    const all = new Set(
      (['primary', 'secondary', 'ghost', 'danger'] as const).map((v) => buttonVariants(v)),
    );
    expect(all.size).toBe(4);
  });

  it('所有变体都含统一圆角与间距', () => {
    for (const v of ['primary', 'secondary', 'ghost', 'danger'] as const) {
      const cls = buttonVariants(v);
      expect(cls).toContain('rounded-lg');
      expect(cls).toContain('whitespace-nowrap');
    }
  });
});

describe('Button', () => {
  it('渲染 children 并透传 onClick', () => {
    let clicked = false;
    render(<Button onClick={() => (clicked = true)}>保存</Button>);
    screen.getByRole('button', { name: '保存' }).click();
    expect(clicked).toBe(true);
  });

  it('disabled 时阻止点击', () => {
    let clicked = false;
    render(
      <Button disabled onClick={() => (clicked = true)}>
        新建
      </Button>,
    );
    const btn = screen.getByRole('button', { name: '新建' });
    expect(btn).toBeDisabled();
    btn.click();
    expect(clicked).toBe(false);
  });

  it('primary 变体带强调底色', () => {
    render(<Button variant="primary">保存</Button>);
    expect(screen.getByRole('button').className).toContain('btn-primary');
  });
});

describe('兼容导出', () => {
  it('btnClass / btnStyle 仍可用', () => {
    expect(typeof btnClass).toBe('string');
    expect(btnStyle).toHaveProperty('backgroundColor');
  });
});
