import { describe, it, expect } from 'vitest';
import zh from '@/i18n/locales/zh-CN.json';
import en from '@/i18n/locales/en.json';

// 把嵌套文案展平为 "a.b.c" 形式的键值表
function flat(obj: unknown, prefix = ''): Record<string, string> {
  const out: Record<string, string> = {};
  if (obj && typeof obj === 'object') {
    for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
      if (typeof v === 'string') out[prefix + k] = v;
      else if (v && typeof v === 'object') Object.assign(out, flat(v, `${prefix}${k}.`));
    }
  }
  return out;
}

const Z = flat(zh);
const E = flat(en);

describe('新增 i18n 键', () => {
  const NEW_KEYS = [
    'button.savePath',
    'button.cancelAndClose',
    'menu.label',
    'menu.maintenance',
    'menu.application',
    'menu.clean',
    'menu.analyze',
    'menu.profiles',
    'menu.language',
    'menu.help',
  ];

  it('中英文都声明了全部新键', () => {
    for (const k of NEW_KEYS) {
      expect(Z[k], `zh 缺少 ${k}`).toBeTruthy();
      expect(E[k], `en 缺少 ${k}`).toBeTruthy();
    }
  });

  it('工具栏保存用「保存」/Save，对话框仍用「确定」/OK', () => {
    expect(Z['button.savePath']).toBe('保存');
    expect(E['button.savePath']).toBe('Save');
    expect(Z['button.save']).toBe('确定');
    expect(E['button.save']).toBe('OK');
  });

  it('工具栏取消用「取消并关闭」，对话框仍用「取消」/Cancel', () => {
    expect(Z['button.cancelAndClose']).toBe('取消并关闭');
    expect(E['button.cancelAndClose']).toBe('Cancel');
    expect(Z['button.cancel']).toBe('取消');
    expect(E['button.cancel']).toBe('Cancel');
  });

  it('中英文键集合一致', () => {
    // 跳过以 _comment 结尾的纯注释键（如 error.code 下的说明）
    const onlyZh = Object.keys(Z).filter((k) => !(k in E) && !k.includes('_comment'));
    const onlyEn = Object.keys(E).filter((k) => !(k in Z) && !k.includes('_comment'));
    expect(onlyZh).toEqual([]);
    expect(onlyEn).toEqual([]);
  });
});
