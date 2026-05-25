import { describe, it, expect } from 'vitest';
import { pathClean } from '../../src/core/path-manager';

const alwaysValid = () => true;
const validateFn = (path: string) => !path.includes('Invalid');

describe('pathClean', () => {
  it('移除无效路径', () => {
    const [kept, removed] = pathClean(['C:\\Valid', 'C:\\Invalid', 'D:\\Valid'], validateFn);
    expect(kept).toEqual(['C:\\Valid', 'D:\\Valid']);
    expect(removed).toEqual(['C:\\Invalid']);
  });

  it('移除重复路径保留第一个', () => {
    const [kept, removed] = pathClean(['C:\\Valid', 'C:\\Valid', 'D:\\Valid'], alwaysValid);
    expect(kept.length).toBe(2);
    expect(removed.length).toBe(1);
  });

  it('全部有效无变化', () => {
    const [kept, removed] = pathClean(['C:\\a', 'D:\\b'], alwaysValid);
    expect(kept).toEqual(['C:\\a', 'D:\\b']);
    expect(removed.length).toBe(0);
  });

  it('全部无效全部移除', () => {
    const [kept, removed] = pathClean(['C:\\Invalid1', 'C:\\Invalid2'], validateFn);
    expect(kept.length).toBe(0);
    expect(removed.length).toBe(2);
  });
});
