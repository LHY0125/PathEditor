import { describe, it, expect } from 'vitest';
import { is_valid_path_format, join_path, split_path } from '../../src/core/validation';

describe('is_valid_path_format', () => {
  it('合法的驱动器路径', () => {
    expect(is_valid_path_format('C:\\Windows\\System32')).toBe(true);
    expect(is_valid_path_format('D:/Projects')).toBe(true);
    expect(is_valid_path_format('E:\\')).toBe(true);
  });

  it('合法的 UNC 路径', () => {
    expect(is_valid_path_format('\\\\server\\share')).toBe(true);
    expect(is_valid_path_format('//server/share')).toBe(true);
  });

  it('环境变量路径', () => {
    expect(is_valid_path_format('%JAVA_HOME%\\bin')).toBe(true);
    expect(is_valid_path_format('%APPDATA%')).toBe(true);
  });

  it('包含分隔符的相对路径', () => {
    expect(is_valid_path_format('bin/debug')).toBe(true);
    expect(is_valid_path_format('tools\\utils')).toBe(true);
  });

  it('空字符串非法', () => {
    expect(is_valid_path_format('')).toBe(false);
    expect(is_valid_path_format('   ')).toBe(false);
  });

  it('纯名称不包含分隔符也非法', () => {
    expect(is_valid_path_format('notepad.exe')).toBe(false);
  });
});

describe('join_path', () => {
  it('用分号连接', () => {
    expect(join_path(['C:\\', 'D:\\'])).toBe('C:\\;D:\\');
  });

  it('空数组返回空字符串', () => {
    expect(join_path([])).toBe('');
  });
});

describe('split_path', () => {
  it('用分号分割', () => {
    expect(split_path('C:\\;D:\\;E:\\')).toEqual(['C:\\', 'D:\\', 'E:\\']);
  });

  it('过滤空字符串', () => {
    expect(split_path('C:\\;;D:\\')).toEqual(['C:\\', 'D:\\']);
  });

  it('去除首尾空格', () => {
    expect(split_path(' C:\\ ; D:\\ ')).toEqual(['C:\\', 'D:\\']);
  });

  it('空字符串返回空数组', () => {
    expect(split_path('')).toEqual([]);
  });
});
