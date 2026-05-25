import { describe, it, expect, beforeEach } from 'vitest';
import {
  UndoRedoManager,
  OperationType,
  TargetType,
  type OpRecord,
} from '../../src/core/undo-redo';
import { StringList } from '../../src/core/string-list';

function makeRecord(
  type: OperationType,
  target: TargetType,
  index: number,
  count: number,
  oldPaths: string[],
  newPaths: string[],
): OpRecord {
  return { type, target, index, count, oldPaths, newPaths };
}

describe('UndoRedoManager', () => {
  let mgr: UndoRedoManager;
  let sysPaths: StringList;
  let userPaths: StringList;

  beforeEach(() => {
    mgr = new UndoRedoManager(50);
    sysPaths = StringList.fromArray(['C:\\Windows', 'C:\\Program Files']);
    userPaths = StringList.fromArray(['C:\\Users\\me\\AppData']);
  });

  // ── 基本状态 ──

  it('初始不可撤销不可重做', () => {
    expect(mgr.canUndo()).toBe(false);
    expect(mgr.canRedo()).toBe(false);
  });

  // ── ADD ──

  it('ADD 撤销/重做', () => {
    sysPaths.add('C:\\NewPath');

    mgr.push(
      makeRecord(OperationType.ADD, TargetType.SYSTEM, 2, 1, [], ['C:\\NewPath']),
    );

    expect(mgr.canUndo()).toBe(true);

    mgr.undo(sysPaths, userPaths);
    expect(sysPaths.toArray()).toEqual(['C:\\Windows', 'C:\\Program Files']);

    mgr.redo(sysPaths, userPaths);
    expect(sysPaths.toArray()).toEqual(['C:\\Windows', 'C:\\Program Files', 'C:\\NewPath']);
  });

  // ── DELETE ──

  it('DELETE 撤销/重做', () => {
    const removed = sysPaths.get(0)!;
    mgr.push(
      makeRecord(OperationType.DELETE, TargetType.SYSTEM, 0, 1, [removed], []),
    );

    sysPaths.removeAt(0);

    mgr.undo(sysPaths, userPaths);
    expect(sysPaths.get(0)).toBe(removed);

    mgr.redo(sysPaths, userPaths);
    expect(sysPaths.toArray()).toEqual(['C:\\Program Files']);
  });

  // ── EDIT ──

  it('EDIT 撤销/重做', () => {
    const oldVal = sysPaths.get(0)!;
    mgr.push(
      makeRecord(OperationType.EDIT, TargetType.SYSTEM, 0, 1, [oldVal], ['C:\\Edited']),
    );

    sysPaths.set(0, 'C:\\Edited');

    mgr.undo(sysPaths, userPaths);
    expect(sysPaths.get(0)).toBe(oldVal);

    mgr.redo(sysPaths, userPaths);
    expect(sysPaths.get(0)).toBe('C:\\Edited');
  });

  // ── MOVE_UP ──

  it('MOVE_UP 撤销/重做', () => {
    mgr.push(
      makeRecord(OperationType.MOVE_UP, TargetType.SYSTEM, 1, 1, [], []),
    );

    sysPaths.swap(0, 1);

    expect(sysPaths.toArray()).toEqual(['C:\\Program Files', 'C:\\Windows']);

    mgr.undo(sysPaths, userPaths);
    expect(sysPaths.toArray()).toEqual(['C:\\Windows', 'C:\\Program Files']);

    mgr.redo(sysPaths, userPaths);
    expect(sysPaths.toArray()).toEqual(['C:\\Program Files', 'C:\\Windows']);
  });

  // ── MOVE_DOWN ──

  it('MOVE_DOWN 撤销/重做', () => {
    mgr.push(
      makeRecord(OperationType.MOVE_DOWN, TargetType.SYSTEM, 0, 1, [], []),
    );

    sysPaths.swap(0, 1);

    mgr.undo(sysPaths, userPaths);
    expect(sysPaths.toArray()).toEqual(['C:\\Windows', 'C:\\Program Files']);

    mgr.redo(sysPaths, userPaths);
    expect(sysPaths.toArray()).toEqual(['C:\\Program Files', 'C:\\Windows']);
  });

  // ── CLEAN ──

  it('CLEAN 撤销/重做', () => {
    const oldPaths = sysPaths.toArray();
    const newPaths = ['C:\\Windows']; // 假设 Program Files 被清理掉了

    mgr.push(
      makeRecord(OperationType.CLEAN, TargetType.SYSTEM, 0, 2, oldPaths, newPaths),
    );

    sysPaths.clear();
    for (const p of newPaths) sysPaths.add(p);

    mgr.undo(sysPaths, userPaths);
    expect(sysPaths.toArray()).toEqual(oldPaths);

    mgr.redo(sysPaths, userPaths);
    expect(sysPaths.toArray()).toEqual(newPaths);
  });

  // ── CLEAR ──

  it('CLEAR 撤销/重做', () => {
    const oldPaths = sysPaths.toArray();

    mgr.push(
      makeRecord(OperationType.CLEAR, TargetType.SYSTEM, 0, 2, oldPaths, []),
    );

    sysPaths.clear();

    mgr.undo(sysPaths, userPaths);
    expect(sysPaths.toArray()).toEqual(oldPaths);

    mgr.redo(sysPaths, userPaths);
    expect(sysPaths.length).toBe(0);
  });

  // ── IMPORT ──

  it('IMPORT 撤销/重做', () => {
    const oldPaths = sysPaths.toArray();
    const imported = ['C:\\New1', 'C:\\New2'];

    mgr.push(
      makeRecord(OperationType.IMPORT, TargetType.SYSTEM, 0, 2, oldPaths, imported),
    );

    sysPaths.clear();
    for (const p of imported) sysPaths.add(p);

    mgr.undo(sysPaths, userPaths);
    expect(sysPaths.toArray()).toEqual(oldPaths);

    mgr.redo(sysPaths, userPaths);
    expect(sysPaths.toArray()).toEqual(imported);
  });

  // ── 重做分支截断 ──

  it('新操作后截断重做分支', () => {
    mgr.push(
      makeRecord(OperationType.ADD, TargetType.SYSTEM, 0, 1, [], ['first']),
    );
    mgr.undo(sysPaths, userPaths);
    expect(mgr.canRedo()).toBe(true);

    // 推入新操作，重做分支被截断
    mgr.push(
      makeRecord(OperationType.ADD, TargetType.SYSTEM, 0, 1, [], ['second']),
    );
    expect(mgr.canRedo()).toBe(false);
  });

  // ── 历史限制 ──

  it('超出最大历史容量时移除最旧记录', () => {
    const small = new UndoRedoManager(3);
    for (let i = 0; i < 5; i++) {
      small.push(
        makeRecord(OperationType.ADD, TargetType.SYSTEM, 0, 1, [], [`path_${i}`]),
      );
    }
    expect(small.historyLength).toBe(3);
  });

  // ── USER 目标 ──

  it('操作 USER 路径', () => {
    userPaths.add('C:\\NewUserPath');
    mgr.push(
      makeRecord(OperationType.ADD, TargetType.USER, 1, 1, [], ['C:\\NewUserPath']),
    );

    mgr.undo(sysPaths, userPaths);
    expect(userPaths.toArray()).toEqual(['C:\\Users\\me\\AppData']);
    expect(sysPaths.toArray()).toEqual(['C:\\Windows', 'C:\\Program Files']);
  });
});
