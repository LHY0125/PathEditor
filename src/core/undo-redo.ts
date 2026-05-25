/**
 * 撤销/重做管理器 — 对应 C 版 undo_redo.c
 * 支持 8 种操作类型的完整撤销/重做
 */
import { StringList } from './string-list';

export const OperationType = {
  ADD: 0,        // 新增路径
  DELETE: 1,     // 删除路径
  EDIT: 2,       // 编辑路径
  MOVE_UP: 3,    // 上移
  MOVE_DOWN: 4,  // 下移
  CLEAN: 5,      // 一键清理
  CLEAR: 6,      // 清空
  IMPORT: 7,     // 导入
} as const;
export type OperationType = (typeof OperationType)[keyof typeof OperationType];

export const TargetType = {
  SYSTEM: 0,
  USER: 1,
} as const;
export type TargetType = (typeof TargetType)[keyof typeof TargetType];

export interface OpRecord {
  type: OperationType;
  target: TargetType;
  index: number;
  count: number;
  oldPaths: string[];
  newPaths: string[];
}

const DEFAULT_MAX_SIZE = 50;

export class UndoRedoManager {
  private records: OpRecord[] = [];
  private current: number = -1;
  private readonly maxSize: number;

  constructor(maxSize: number = DEFAULT_MAX_SIZE) {
    this.maxSize = maxSize;
  }

  /** 推送新操作记录，推送后截断重做分支 */
  push(record: OpRecord): void {
    // 截断重做分支
    this.records = this.records.slice(0, this.current + 1);

    // 如果已满，移除最旧的记录
    if (this.records.length >= this.maxSize) {
      this.records.shift();
    }

    this.records.push(record);
    this.current = this.records.length - 1;
  }

  /** 撤销当前操作 */
  undo(sysPaths: StringList, userPaths: StringList): boolean {
    if (this.current < 0) return false;

    const rec = this.records[this.current];
    this.current--;

    const target = rec.target === TargetType.SYSTEM ? sysPaths : userPaths;

    switch (rec.type) {
      case OperationType.ADD:
        // 撤销添加 — 删除最后 count 个元素
        for (let i = 0; i < rec.count; i++) {
          target.removeAt(target.length - 1);
        }
        break;

      case OperationType.DELETE:
        // 撤销删除 — 逐个恢复
        for (let i = 0; i < rec.count; i++) {
          target.insertAt(rec.index + i, rec.oldPaths[i]);
        }
        break;

      case OperationType.EDIT:
        target.set(rec.index, rec.oldPaths[0]);
        break;

      case OperationType.MOVE_UP:
        target.swap(rec.index - 1, rec.index);
        break;

      case OperationType.MOVE_DOWN:
        target.swap(rec.index, rec.index + 1);
        break;

      case OperationType.CLEAN:
      case OperationType.IMPORT:
        // 恢复到操作前的完整列表
        target.clear();
        for (const path of rec.oldPaths) {
          target.add(path);
        }
        break;

      case OperationType.CLEAR:
        for (const path of rec.oldPaths) {
          target.add(path);
        }
        break;
    }

    return true;
  }

  /** 重做下一个操作 */
  redo(sysPaths: StringList, userPaths: StringList): boolean {
    if (this.current >= this.records.length - 1) return false;

    this.current++;
    const rec = this.records[this.current];
    const target = rec.target === TargetType.SYSTEM ? sysPaths : userPaths;

    switch (rec.type) {
      case OperationType.ADD:
        for (let i = 0; i < rec.count; i++) {
          target.add(rec.newPaths[i]);
        }
        break;

      case OperationType.DELETE:
        // 从后往前删，避免索引偏移
        for (let i = rec.count - 1; i >= 0; i--) {
          target.removeAt(rec.index + i);
        }
        break;

      case OperationType.EDIT:
        target.set(rec.index, rec.newPaths[0]);
        break;

      case OperationType.MOVE_UP:
        target.swap(rec.index - 1, rec.index);
        break;

      case OperationType.MOVE_DOWN:
        target.swap(rec.index, rec.index + 1);
        break;

      case OperationType.CLEAN:
      case OperationType.IMPORT:
        target.clear();
        for (const path of rec.newPaths) {
          target.add(path);
        }
        break;

      case OperationType.CLEAR:
        target.clear();
        break;
    }

    return true;
  }

  canUndo(): boolean {
    return this.current >= 0;
  }

  canRedo(): boolean {
    return this.current < this.records.length - 1;
  }

  clear(): void {
    this.records = [];
    this.current = -1;
  }

  get historyLength(): number {
    return this.records.length;
  }
}
