import { describe, it, expect } from 'vitest';
import { pathClean } from '../../src/core/path-manager';
import type { PathEntry } from '../../src/core/path-entry';

function pe(s: string, enabled: boolean = true): PathEntry {
  return { path: s, enabled };
}

const alwaysValid = () => true;
const validateFn = (path: string) => !path.includes('Invalid');

describe('pathClean', () => {
  it('移除无效路径', () => {
    const [kept, removed] = pathClean([pe('C:\\Valid'), pe('C:\\Invalid'), pe('D:\\Valid')], validateFn);
    expect(kept.map(e => e.path)).toEqual(['C:\\Valid', 'D:\\Valid']);
    expect(removed.map(e => e.path)).toEqual(['C:\\Invalid']);
  });

  it('移除重复路径保留第一个', () => {
    const [kept, removed] = pathClean([pe('C:\\Valid'), pe('C:\\Valid'), pe('D:\\Valid')], alwaysValid);
    expect(kept.length).toBe(2);
    expect(removed.length).toBe(1);
  });

  it('全部有效无变化', () => {
    const [kept, removed] = pathClean([pe('C:\\a'), pe('D:\\b')], alwaysValid);
    expect(kept.map(e => e.path)).toEqual(['C:\\a', 'D:\\b']);
    expect(removed.length).toBe(0);
  });

  it('全部无效全部移除', () => {
    const [kept, removed] = pathClean([pe('C:\\Invalid1'), pe('C:\\Invalid2')], validateFn);
    expect(kept.length).toBe(0);
    expect(removed.length).toBe(2);
  });
});
