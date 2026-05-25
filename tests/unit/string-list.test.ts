import { describe, it, expect } from 'vitest';
import { StringList } from '../../src/core/string-list';

describe('StringList', () => {
  it('初始为空', () => {
    const list = new StringList();
    expect(list.length).toBe(0);
  });

  it('添加和读取元素', () => {
    const list = new StringList();
    list.add('C:\\Windows');
    list.add('C:\\Users');
    expect(list.length).toBe(2);
    expect(list.get(0)).toBe('C:\\Windows');
    expect(list.get(1)).toBe('C:\\Users');
    expect(list.get(99)).toBeUndefined();
  });

  it('在指定位置插入', () => {
    const list = new StringList();
    list.add('a');
    list.add('c');
    list.insertAt(1, 'b');
    expect(list.toArray()).toEqual(['a', 'b', 'c']);
  });

  it('在开头插入', () => {
    const list = StringList.fromArray(['b', 'c']);
    list.insertAt(0, 'a');
    expect(list.toArray()).toEqual(['a', 'b', 'c']);
  });

  it('删除指定位置', () => {
    const list = StringList.fromArray(['a', 'b', 'c']);
    list.removeAt(1);
    expect(list.toArray()).toEqual(['a', 'c']);
  });

  it('设置元素', () => {
    const list = StringList.fromArray(['old']);
    list.set(0, 'new');
    expect(list.get(0)).toBe('new');
  });

  it('不区分大小写查找', () => {
    const list = StringList.fromArray(['C:\\Windows', 'C:\\Users']);
    expect(list.contains('c:\\windows')).toBe(true);
    expect(list.contains('C:\\WINDOWS')).toBe(true);
    expect(list.contains('C:\\Other')).toBe(false);
  });

  it('不区分大小写索引', () => {
    const list = StringList.fromArray(['C:\\Windows', 'C:\\Users']);
    expect(list.indexOfIgnoreCase('c:\\windows')).toBe(0);
    expect(list.indexOfIgnoreCase('c:\\users')).toBe(1);
    expect(list.indexOfIgnoreCase('nope')).toBe(-1);
  });

  it('交换元素', () => {
    const list = StringList.fromArray(['a', 'b']);
    list.swap(0, 1);
    expect(list.toArray()).toEqual(['b', 'a']);
  });

  it('清空', () => {
    const list = StringList.fromArray(['a', 'b', 'c']);
    list.clear();
    expect(list.length).toBe(0);
  });

  it('深拷贝', () => {
    const original = StringList.fromArray(['a', 'b']);
    const cloned = original.clone();
    cloned.set(0, 'modified');
    expect(original.get(0)).toBe('a');
    expect(cloned.get(0)).toBe('modified');
  });

  it('fromArray 和 toArray', () => {
    const arr = ['x', 'y', 'z'];
    const list = StringList.fromArray(arr);
    expect(list.toArray()).toEqual(arr);
    expect(list.length).toBe(3);
  });

  it('all 返回只读数组', () => {
    const list = StringList.fromArray(['a', 'b']);
    expect(list.all).toEqual(['a', 'b']);
  });
});
