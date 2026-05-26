import { describe, it, expect, beforeEach } from 'vitest';
import { UndoRedoManager, OperationType, TargetType, type OpRecord } from '../../src/core/undo-redo';

function makeRecord(type: OperationType, target: TargetType, index: number, count: number, oldPaths: string[], newPaths: string[]): OpRecord {
  return { type, target, index, count, oldPaths, newPaths };
}

describe('UndoRedoManager', () => {
  let mgr: UndoRedoManager;
  let sys: string[];
  let user: string[];

  beforeEach(() => {
    mgr = new UndoRedoManager(50);
    sys = ['C:\\Windows', 'C:\\Program Files'];
    user = ['C:\\Users\\me\\AppData'];
  });

  it('初始不可撤销不可重做', () => {
    expect(mgr.canUndo()).toBe(false);
    expect(mgr.canRedo()).toBe(false);
  });

  it('ADD 撤销/重做', () => {
    sys.push('C:\\NewPath');
    mgr.push(makeRecord(OperationType.ADD, TargetType.SYSTEM, 2, 1, [], ['C:\\NewPath']));

    const u = mgr.undo(sys, user)!;
    expect(u[0]).toEqual(['C:\\Windows', 'C:\\Program Files']);

    const r = mgr.redo(...u)!;
    expect(r[0]).toEqual(['C:\\Windows', 'C:\\Program Files', 'C:\\NewPath']);
  });

  it('DELETE 撤销/重做', () => {
    const removed = sys[0];
    mgr.push(makeRecord(OperationType.DELETE, TargetType.SYSTEM, 0, 1, [removed], []));
    sys.splice(0, 1);

    const u = mgr.undo(sys, user)!;
    expect(u[0][0]).toBe(removed);

    const r = mgr.redo(...u)!;
    expect(r[0]).toEqual(['C:\\Program Files']);
  });

  it('EDIT 撤销/重做', () => {
    mgr.push(makeRecord(OperationType.EDIT, TargetType.SYSTEM, 0, 1, ['C:\\Windows'], ['C:\\Edited']));
    sys[0] = 'C:\\Edited';

    const u = mgr.undo(sys, user)!;
    expect(u[0][0]).toBe('C:\\Windows');

    const r = mgr.redo(...u)!;
    expect(r[0][0]).toBe('C:\\Edited');
  });

  it('MOVE_UP 撤销/重做', () => {
    mgr.push(makeRecord(OperationType.MOVE_UP, TargetType.SYSTEM, 1, 1, [], []));
    [sys[0], sys[1]] = [sys[1], sys[0]];

    const u = mgr.undo(sys, user)!;
    expect(u[0]).toEqual(['C:\\Windows', 'C:\\Program Files']);

    const r = mgr.redo(...u)!;
    expect(r[0]).toEqual(['C:\\Program Files', 'C:\\Windows']);
  });

  it('MOVE_DOWN 撤销/重做', () => {
    mgr.push(makeRecord(OperationType.MOVE_DOWN, TargetType.SYSTEM, 0, 1, [], []));
    [sys[0], sys[1]] = [sys[1], sys[0]];

    const u = mgr.undo(sys, user)!;
    expect(u[0]).toEqual(['C:\\Windows', 'C:\\Program Files']);
  });

  it('CLEAN 撤销/重做', () => {
    const old = [...sys];
    const cleaned = ['C:\\Windows'];
    mgr.push(makeRecord(OperationType.CLEAN, TargetType.SYSTEM, 0, 2, old, cleaned));
    sys = cleaned;

    const u = mgr.undo(sys, user)!;
    expect(u[0]).toEqual(old);

    const r = mgr.redo(...u)!;
    expect(r[0]).toEqual(cleaned);
  });

  it('CLEAR 撤销/重做', () => {
    const old = [...sys];
    mgr.push(makeRecord(OperationType.CLEAR, TargetType.SYSTEM, 0, 2, old, []));
    sys = [];

    const u = mgr.undo(sys, user)!;
    expect(u[0]).toEqual(old);

    const r = mgr.redo(...u)!;
    expect(r[0]).toEqual([]);
  });

  it('IMPORT 撤销/重做', () => {
    const old = [...sys];
    const imported = ['C:\\New1', 'C:\\New2'];
    mgr.push(makeRecord(OperationType.IMPORT, TargetType.SYSTEM, 0, 2, old, imported));
    sys = imported;

    const u = mgr.undo(sys, user)!;
    expect(u[0]).toEqual(old);

    const r = mgr.redo(...u)!;
    expect(r[0]).toEqual(imported);
  });

  it('新操作后截断重做分支', () => {
    mgr.push(makeRecord(OperationType.ADD, TargetType.SYSTEM, 0, 1, [], ['first']));
    mgr.undo(sys, user);
    expect(mgr.canRedo()).toBe(true);
    mgr.push(makeRecord(OperationType.ADD, TargetType.SYSTEM, 0, 1, [], ['second']));
    expect(mgr.canRedo()).toBe(false);
  });

  it('超出最大历史容量时移除最旧记录', () => {
    const small = new UndoRedoManager(3);
    for (let i = 0; i < 5; i++) {
      small.push(makeRecord(OperationType.ADD, TargetType.SYSTEM, 0, 1, [], [`path_${i}`]));
    }
    expect(small.historyLength).toBe(3);
  });

  it('非连续多选 DELETE 撤销恢复到原始位置', () => {
    // 扩展初始数组
    sys.push('C:\\Extra1', 'C:\\Extra2');
    const old = [...sys];
    // 删除 indices [1, 3]（C:\Program Files 和 C:\Extra2）
    const removed = [sys[1], sys[3]];
    mgr.push({
      type: OperationType.DELETE, target: TargetType.SYSTEM,
      index: 1, count: 2,
      oldPaths: removed, newPaths: [],
      indices: [1, 3],
    });
    sys.splice(3, 1);
    sys.splice(1, 1);

    const u = mgr.undo(sys, user)!;
    expect(u[0]).toEqual(old);

    const r = mgr.redo(...u)!;
    expect(r[0]).toEqual(['C:\\Windows', 'C:\\Extra1']);
  });

  it('操作 USER 路径', () => {
    user.push('C:\\NewUserPath');
    mgr.push(makeRecord(OperationType.ADD, TargetType.USER, 1, 1, [], ['C:\\NewUserPath']));
    const u = mgr.undo(sys, user)!;
    expect(u[1]).toEqual(['C:\\Users\\me\\AppData']);
    expect(u[0]).toEqual(['C:\\Windows', 'C:\\Program Files']);
  });
});
