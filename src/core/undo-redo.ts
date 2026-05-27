/**
 * 撤销/重做管理器 — 纯逻辑，操作不可变 PathEntry[]
 */

import type { PathEntry } from './path-entry';

export const OperationType = {
  ADD: 0, DELETE: 1, EDIT: 2, MOVE_UP: 3, MOVE_DOWN: 4, CLEAN: 5, CLEAR: 6, IMPORT: 7, TOGGLE: 8, IMPORT_BOTH: 9,
} as const;
export type OperationType = (typeof OperationType)[keyof typeof OperationType];

export const TargetType = { SYSTEM: 0, USER: 1 } as const;
export type TargetType = (typeof TargetType)[keyof typeof TargetType];

export interface OpRecord {
  type: OperationType;
  target: TargetType;
  index: number;
  count: number;
  oldPaths: PathEntry[];
  newPaths: PathEntry[];
  /** DELETE 操作专用：被删除的各路径的原始 index（升序） */
  indices?: number[];
  /** IMPORT_BOTH 专用：用户 hive 的旧路径 */
  oldPathsOther?: PathEntry[];
  /** IMPORT_BOTH 专用：用户 hive 的新路径 */
  newPathsOther?: PathEntry[];
}

const DEFAULT_MAX_SIZE = 50;

export class UndoRedoManager {
  private records: OpRecord[] = [];
  private current: number = -1;
  private readonly maxSize: number;

  constructor(maxSize: number = DEFAULT_MAX_SIZE) {
    this.maxSize = maxSize;
  }

  push(record: OpRecord): void {
    this.records = this.records.slice(0, this.current + 1);
    if (this.records.length >= this.maxSize) {
      this.records.shift();
    }
    this.records.push(record);
    this.current = this.records.length - 1;
  }

  undo(sysPaths: readonly PathEntry[], userPaths: readonly PathEntry[]): [PathEntry[], PathEntry[]] | null {
    if (this.current < 0) return null;

    const rec = this.records[this.current];
    this.current--;

    const sys = [...sysPaths];
    const user = [...userPaths];
    const target = rec.target === TargetType.SYSTEM ? sys : user;

    switch (rec.type) {
      case OperationType.ADD:
        target.splice(target.length - rec.count, rec.count);
        break;
      case OperationType.DELETE:
        if (rec.indices) {
          for (let i = 0; i < rec.indices.length; i++) {
            target.splice(rec.indices[i], 0, rec.oldPaths[i]);
          }
        } else {
          for (let i = 0; i < rec.count; i++) {
            target.splice(rec.index + i, 0, rec.oldPaths[i]);
          }
        }
        break;
      case OperationType.EDIT:
        target[rec.index] = rec.oldPaths[0];
        break;
      case OperationType.MOVE_UP:
        [target[rec.index - 1], target[rec.index]] = [target[rec.index], target[rec.index - 1]];
        break;
      case OperationType.MOVE_DOWN:
        [target[rec.index], target[rec.index + 1]] = [target[rec.index + 1], target[rec.index]];
        break;
      case OperationType.CLEAN:
      case OperationType.IMPORT:
        target.length = 0;
        target.push(...rec.oldPaths);
        break;
      case OperationType.CLEAR:
        target.push(...rec.oldPaths);
        break;
      case OperationType.TOGGLE:
        target[rec.index] = rec.oldPaths[0];
        break;
      case OperationType.IMPORT_BOTH:
        sys.length = 0;
        sys.push(...rec.oldPaths);
        user.length = 0;
        user.push(...(rec.oldPathsOther || []));
        return [sys, user];
    }

    return [sys, user];
  }

  redo(sysPaths: readonly PathEntry[], userPaths: readonly PathEntry[]): [PathEntry[], PathEntry[]] | null {
    if (this.current >= this.records.length - 1) return null;

    this.current++;
    const rec = this.records[this.current];

    const sys = [...sysPaths];
    const user = [...userPaths];
    const target = rec.target === TargetType.SYSTEM ? sys : user;

    switch (rec.type) {
      case OperationType.ADD:
        target.push(...rec.newPaths);
        break;
      case OperationType.DELETE:
        if (rec.indices) {
          for (let i = rec.indices.length - 1; i >= 0; i--) {
            target.splice(rec.indices[i], 1);
          }
        } else {
          for (let i = rec.count - 1; i >= 0; i--) {
            target.splice(rec.index + i, 1);
          }
        }
        break;
      case OperationType.EDIT:
        target[rec.index] = rec.newPaths[0];
        break;
      case OperationType.MOVE_UP:
        [target[rec.index - 1], target[rec.index]] = [target[rec.index], target[rec.index - 1]];
        break;
      case OperationType.MOVE_DOWN:
        [target[rec.index], target[rec.index + 1]] = [target[rec.index + 1], target[rec.index]];
        break;
      case OperationType.CLEAN:
      case OperationType.IMPORT:
        target.length = 0;
        target.push(...rec.newPaths);
        break;
      case OperationType.CLEAR:
        target.length = 0;
        break;
      case OperationType.TOGGLE:
        target[rec.index] = rec.newPaths[0];
        break;
      case OperationType.IMPORT_BOTH:
        sys.length = 0;
        sys.push(...rec.newPaths);
        user.length = 0;
        user.push(...(rec.newPathsOther || []));
        return [sys, user];
    }

    return [sys, user];
  }

  canUndo(): boolean { return this.current >= 0; }
  canRedo(): boolean { return this.current < this.records.length - 1; }
  clear(): void { this.records = []; this.current = -1; }
  get historyLength(): number { return this.records.length; }
}
