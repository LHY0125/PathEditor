/** PATH 路径条目 — 包含路径值和启用状态 */
export interface PathEntry {
  path: string;
  enabled: boolean;
}

/** 系统 PATH 与用户 PATH 的完整有序快照。 */
export interface PathSnapshot {
  system: PathEntry[];
  user: PathEntry[];
}
