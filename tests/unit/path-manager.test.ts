import { describe, it, expect } from 'vitest';
import { pathClean, analyzePaths } from '../../src/core/path-manager';
import type { PathEntry } from '../../src/core/path-entry';

function pe(s: string, enabled: boolean = true): PathEntry {
  return { path: s, enabled };
}

const alwaysValid = () => true;
const validateFn = (path: string) => !path.includes('Invalid');

describe('analyzePaths', () => {
  it('检测大小写重复', () => {
    const result = analyzePaths([pe('C:\\Windows'), pe('c:\\windows')], alwaysValid);
    expect(result[0].isDuplicate).toBe(false);
    expect(result[1].isDuplicate).toBe(true);
  });

  it('识别环境变量路径', () => {
    const result = analyzePaths([pe('C:\\Normal'), pe('%JAVA_HOME%\\bin')], alwaysValid);
    expect(result[0].isEnvVar).toBe(false);
    expect(result[1].isEnvVar).toBe(true);
  });

  it('标记无效路径', () => {
    const result = analyzePaths([pe('C:\\Valid'), pe('C:\\Invalid')], validateFn);
    expect(result[0].isValid).toBe(true);
    expect(result[1].isValid).toBe(false);
  });

  it('空数组返回空', () => {
    const result = analyzePaths([], alwaysValid);
    expect(result).toEqual([]);
  });
});

describe('pathClean', () => {
  it('移除无效路径', () => {
    const [kept, removed] = pathClean(
      [pe('C:\\Valid'), pe('C:\\Invalid'), pe('D:\\Valid')],
      validateFn,
    );
    expect(kept.map((e) => e.path)).toEqual(['C:\\Valid', 'D:\\Valid']);
    expect(removed.map((e) => e.path)).toEqual(['C:\\Invalid']);
  });

  it('移除重复路径保留第一个', () => {
    const [kept, removed] = pathClean(
      [pe('C:\\Valid'), pe('C:\\Valid'), pe('D:\\Valid')],
      alwaysValid,
    );
    expect(kept.length).toBe(2);
    expect(removed.length).toBe(1);
  });

  it('保留第一个出现的 enabled 状态', () => {
    const [kept, removed] = pathClean([pe('C:\\Valid', false), pe('C:\\Valid', true)], alwaysValid);
    expect(kept.length).toBe(1);
    expect(kept[0].enabled).toBe(false); // 第一个状态
    expect(removed.length).toBe(1);
  });

  it('全部有效无变化', () => {
    const [kept, removed] = pathClean([pe('C:\\a'), pe('D:\\b')], alwaysValid);
    expect(kept.map((e) => e.path)).toEqual(['C:\\a', 'D:\\b']);
    expect(removed.length).toBe(0);
  });

  it('空数组处理', () => {
    const [kept, removed] = pathClean([], alwaysValid);
    expect(kept.length).toBe(0);
    expect(removed.length).toBe(0);
  });

  it('全部无效全部移除', () => {
    const [kept, removed] = pathClean([pe('C:\\Invalid1'), pe('C:\\Invalid2')], validateFn);
    expect(kept.length).toBe(0);
    expect(removed.length).toBe(2);
  });
});
