//! 统计聚合。
//!
//! `daily_stats` 是**派生表**：从 `events` / `snapshots` / `scan_runs` 重算，
//! 可随时重建。

use super::Db;
use crate::domain::error::CoreError;
use rusqlite::params;

/// 单日聚合点。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPoint {
    /// `YYYY-MM-DD`
    pub day: String,
    /// 当日快照的 PATH 条目数（取当日最后一条快照）
    pub path_items: i64,
    /// 当日快照的环境变量数
    pub env_items: i64,
    /// 当日事件数
    pub events: i64,
    /// 当日冲突扫描次数
    pub conflicts: i64,
}

/// 统计面板数据。
///
/// **口径说明**：`days` 是按天聚合的**历史**序列，某一天的 `path_items`
/// 是「那天最后一次保存时的条目数」，不是「现在有多少条」。
/// 「现在」单独由 `current_path_items` / `current_avg_len` 给出，
/// 取自 `app_state.current_snapshot_id`（与 GUI 的 `READ_SNAPSHOT_CACHE` 同源）。
/// UI 上必须把这两个概念分开显示，否则用户会把「今天最后一次保存」误读成「当前」。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    /// 按日期升序的时间序列
    pub days: Vec<DailyPoint>,
    /// 最常改变的变量
    pub top_targets: Vec<TargetCount>,
    /// 配置应用次数
    pub profile_applies: Vec<ProfileApply>,
    /// **当前**快照的 PATH 平均长度
    pub current_avg_len: f64,
    /// **当前**快照的 PATH 条目数
    pub current_path_items: i64,
}

/// 变量被修改的次数。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetCount {
    /// 变量名（`events.target`）
    pub target: String,
    /// 出现次数
    pub count: i64,
}

/// 配置的应用统计。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileApply {
    /// 配置名
    pub name: String,
    /// 应用次数
    pub apply_count: i64,
    /// 最近一次应用时间（Unix 毫秒；从未应用为 `None`）
    pub last_applied_at: Option<i64>,
}

/// 全量重建 `daily_stats`，返回写入的天数。
///
/// ：只观测，不设阈值。：当前 `events` 规模下全表扫描是毫秒级，
/// 定一个现在跑不到的阈值等于埋一条永远不会被测到的分支。
/// 等真有启动瓶颈时，日志会提供决策所需的基线数据。
pub fn rebuild_daily(db: &mut Db) -> Result<usize, CoreError> {
    let started = std::time::Instant::now();
    let n = db.with_transaction(|tx| {
        tx.execute("DELETE FROM daily_stats", [])
            .map_err(|e| super::db_err("clear_daily", e))?;

        // 三个来源（events / 快照 / 扫描）先各自按天聚合，再 FULL OUTER JOIN。
        //
        // **为什么不用 `FULL OUTER JOIN`**：它要求 SQLite ≥ 3.39，且与聚合 CTE
        // 混用时语义容易读错。这里改用「UNION ALL 出 (day, 维度) 长表再 pivot」，
        // 语义完全相同，且不依赖任何可选特性—— 不引入新的版本约束。
        //
        // 每个来源只贡献**自己那一列**，其余列由别的分支补 0。
        tx.execute(
            "INSERT INTO daily_stats (day, path_items, env_items, events, conflicts)
             WITH ev AS (
               SELECT date(created_at / 1000, 'unixepoch', 'localtime') AS day, count(*) AS c
                 FROM events GROUP BY 1
             ),
             last_snap AS (
               SELECT date(s.created_at / 1000, 'unixepoch', 'localtime') AS day,
                      s.id AS sid,
                      ROW_NUMBER() OVER (
                        PARTITION BY date(s.created_at / 1000, 'unixepoch', 'localtime')
                        ORDER BY s.created_at DESC, s.id DESC
                      ) AS rn
                 FROM snapshots s WHERE s.deleted_at IS NULL
             ),
             snap_counts AS (
               SELECT l.day,
                      (SELECT count(*) FROM path_entries p WHERE p.snapshot_id = l.sid) AS paths,
                      (SELECT count(*) FROM env_values e WHERE e.snapshot_id = l.sid) AS envs
                 FROM last_snap l WHERE l.rn = 1
             ),
             scans AS (
               SELECT date(created_at / 1000, 'unixepoch', 'localtime') AS day, count(*) AS c
                 FROM scan_runs GROUP BY 1
             ),
             -- 把三个来源摊成 (day, path_items, env_items, events, conflicts) 的长表
             merged AS (
               SELECT day, paths AS path_items, envs AS env_items, 0 AS events, 0 AS conflicts FROM snap_counts
               UNION ALL
               SELECT day, 0, 0, c, 0 FROM ev
               UNION ALL
               SELECT day, 0, 0, 0, c FROM scans
             )
             SELECT day,
                    SUM(path_items),
                    SUM(env_items),
                    SUM(events),
                    SUM(conflicts)
               FROM merged
              GROUP BY day",
            [],
        )
        .map_err(|e| super::db_err("rebuild_daily", e))?;

        let n: i64 = tx
            .query_row("SELECT count(*) FROM daily_stats", [], |r| r.get(0))
            .map_err(|e| super::db_err("count_daily", e))?;
        Ok(n as usize)
    })?;
    log::info!(
        "daily_stats 重建完成: {} 天, 耗时 {:?}",
        n,
        started.elapsed()
    );
    Ok(n)
}

/// 读取最近 `days` 天的统计。
///
/// 返回两部分（口径不同，UI 必须分开显示，见 [`Dashboard`] 的文档注释）：
/// - `days`：按天的**历史**序列；
/// - `current_*`：**现在**的状态，取自 `app_state.current_snapshot_id`。
pub fn dashboard(db: &Db, days: u32) -> Result<Dashboard, CoreError> {
    let days = days.clamp(1, 365) as i64;
    let mut stmt = db.prepare(
        "SELECT day, path_items, env_items, events, conflicts
           FROM daily_stats ORDER BY day DESC LIMIT ?1",
    )?;
    let mut rows = stmt
        .query_map(params![days], |r| {
            Ok(DailyPoint {
                day: r.get(0)?,
                path_items: r.get(1)?,
                env_items: r.get(2)?,
                events: r.get(3)?,
                conflicts: r.get(4)?,
            })
        })
        .map_err(|e| super::db_err("dashboard", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("dashboard", e))?;
    rows.reverse(); // 前端按时间升序画图

    // 最常改变的变量：从 events.target 聚合
    let mut stmt = db.prepare(
        "SELECT target, count(*) AS c FROM events
          WHERE target IS NOT NULL AND target <> ''
          GROUP BY target ORDER BY c DESC, target LIMIT 10",
    )?;
    let top_targets = stmt
        .query_map([], |r| {
            Ok(TargetCount {
                target: r.get(0)?,
                count: r.get(1)?,
            })
        })
        .map_err(|e| super::db_err("dashboard_targets", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("dashboard_targets", e))?;

    // 配置应用次数：直接读 profiles
    let mut stmt = db.prepare(
        "SELECT name, apply_count, last_applied_at FROM profiles
          WHERE deleted_at IS NULL ORDER BY apply_count DESC, name LIMIT 10",
    )?;
    let profile_applies = stmt
        .query_map([], |r| {
            Ok(ProfileApply {
                name: r.get(0)?,
                apply_count: r.get(1)?,
                last_applied_at: r.get(2)?,
            })
        })
        .map_err(|e| super::db_err("dashboard_profiles", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| super::db_err("dashboard_profiles", e))?;

    // 当前快照的条目数与平均长度（B-2：daily_stats 无长度列，这里直接查 path_entries）。
    // 注意：`query_row_with` 已经把错误包成 `CoreError`，不能再套 `db_err`
    // （后者只接受 `rusqlite::Error`）。
    let (current_path_items, current_avg_len): (i64, f64) = db
        .query_row_with(
            "SELECT count(*), COALESCE(AVG(length(path)), 0.0)
               FROM path_entries
              WHERE snapshot_id = (SELECT current_snapshot_id FROM app_state WHERE id = 1)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| CoreError::new(e.code, "dashboard_current", e.message))?;

    Ok(Dashboard {
        days: rows,
        top_targets,
        profile_applies,
        current_avg_len,
        current_path_items,
    })
}
