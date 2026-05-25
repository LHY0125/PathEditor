/**
 * StringList — 纯 TypeScript 的字符串列表数据结构
 * 对应 C 版 include/utils/string_ext.h 的 StringList
 */
export class StringList {
  private items: string[] = [];

  /** 追加字符串 */
  add(str: string): void {
    this.items.push(str);
  }

  /** 在指定索引处插入 */
  insertAt(index: number, str: string): void {
    this.items.splice(index, 0, str);
  }

  /** 删除指定索引处的元素 */
  removeAt(index: number): void {
    this.items.splice(index, 1);
  }

  /** 读取索引处元素 */
  get(index: number): string | undefined {
    return this.items[index];
  }

  /** 设置索引处元素 */
  set(index: number, str: string): void {
    this.items[index] = str;
  }

  /** 不区分大小写查找是否包含 */
  contains(str: string): boolean {
    return this.items.some((item) => item.toLowerCase() === str.toLowerCase());
  }

  /** 查找不区分大小写的索引，未找到返回 -1 */
  indexOfIgnoreCase(str: string): number {
    const lower = str.toLowerCase();
    return this.items.findIndex((item) => item.toLowerCase() === lower);
  }

  /** 交换两个索引的元素 */
  swap(i: number, j: number): void {
    const tmp = this.items[i];
    this.items[i] = this.items[j];
    this.items[j] = tmp;
  }

  /** 清空所有元素 */
  clear(): void {
    this.items = [];
  }

  /** 深拷贝 */
  clone(): StringList {
    const list = new StringList();
    list.items = [...this.items];
    return list;
  }

  /** 转换为普通数组（传给 Rust 后端） */
  toArray(): string[] {
    return [...this.items];
  }

  /** 从数组初始化 */
  static fromArray(arr: string[]): StringList {
    const list = new StringList();
    list.items = [...arr];
    return list;
  }

  /** 元素数量 */
  get length(): number {
    return this.items.length;
  }

  /** 只读数组 */
  get all(): readonly string[] {
    return [...this.items];
  }
}
