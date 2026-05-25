import { describe, it, expect, beforeEach } from 'vitest';
import {
  pathRemoveAt,
  pathMoveUp,
  pathMoveDown,
  pathClean,
  batchRemoveAt,
} from '../../src/core/path-manager';
import { StringList } from '../../src/core/string-list';

// 模拟验证函数：所有路径都"有效"
const alwaysValid = () => true;

// 模拟验证函数：C:\\Invalid 无效
const validateFn = (path: string) => !path.includes('Invalid');

describe('pathRemoveAt', () => {
  it('删除指定索引', () => {
    const list = StringList.fromArray(['a', 'b', 'c']);
    pathRemoveAt(list, 1);
    expect(list.toArray()).toEqual(['a', 'c']);
  });
});

describe('pathMoveUp', () => {
  it('上移元素', () => {
    const list = StringList.fromArray(['a', 'b', 'c']);
    pathMoveUp(list, 1);
    expect(list.toArray()).toEqual(['b', 'a', 'c']);
  });

  it('第一个元素不能上移', () => {
    const list = StringList.fromArray(['a', 'b']);
    expect(pathMoveUp(list, 0)).toBe(false);
    expect(list.toArray()).toEqual(['a', 'b']);
  });

  it('无效索引不能上移', () => {
    const list = StringList.fromArray(['a']);
    expect(pathMoveUp(list, -1)).toBe(false);
    expect(pathMoveUp(list, 5)).toBe(false);
  });
});

describe('pathMoveDown', () => {
  it('下移元素', () => {
    const list = StringList.fromArray(['a', 'b', 'c']);
    pathMoveDown(list, 0);
    expect(list.toArray()).toEqual(['b', 'a', 'c']);
  });

  it('最后一个元素不能下移', () => {
    const list = StringList.fromArray(['a', 'b']);
    expect(pathMoveDown(list, 1)).toBe(false);
  });
});

describe('batchRemoveAt', () => {
  it('批量删除（按从大到小排序）', () => {
    const list = StringList.fromArray(['a', 'b', 'c', 'd', 'e']);
    batchRemoveAt(list, [0, 2, 4]);
    expect(list.toArray()).toEqual(['b', 'd']);
  });

  it('删除乱序索引', () => {
    const list = StringList.fromArray(['a', 'b', 'c', 'd']);
    batchRemoveAt(list, [3, 0]);
    expect(list.toArray()).toEqual(['b', 'c']);
  });
});

describe('pathClean', () => {
  it('移除无效路径', () => {
    const list = StringList.fromArray(['C:\\Valid', 'C:\\Invalid', 'D:\\Valid']);
    const removed = pathClean(list, validateFn);
    expect(list.toArray()).toEqual(['C:\\Valid', 'D:\\Valid']);
    expect(removed).toEqual(['C:\\Invalid']);
  });

  it('移除重复路径（保留一个）', () => {
    const list = StringList.fromArray(['C:\\Valid', 'C:\\Valid', 'D:\\Valid']);
    const removed = pathClean(list, alwaysValid);
    expect(list.length).toBe(2);
    expect(removed.length).toBeGreaterThanOrEqual(1);
  });

  it('全部有效无变化', () => {
    const list = StringList.fromArray(['C:\\a', 'D:\\b']);
    const removed = pathClean(list, alwaysValid);
    expect(list.toArray()).toEqual(['C:\\a', 'D:\\b']);
    expect(removed.length).toBe(0);
  });

  it('全部无效全部移除', () => {
    const list = StringList.fromArray(['C:\\Invalid1', 'C:\\Invalid2']);
    const removed = pathClean(list, validateFn);
    expect(list.length).toBe(0);
    expect(removed.length).toBe(2);
  });
});
