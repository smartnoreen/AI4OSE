# locklab

## 位置与运行

- 新 crate：`biglabA5/睡眠锁`（包名 `sleep_lock_lab`）`Cargo.toml`
- 运行：在该目录执行 `cargo test`（已验证通过）

## 可观测指标（Metrics）

- 锁竞争次数：`contentions`
- 平均持锁时间：`avg_hold_time()`（由 `hold_time_total` / `acquisitions`）
- 上下文切换次数（对比差异）：`context_switches`
- 公平性：最大等待时间 `max_wait`
- 是否出现 starvation：`starvation`（等待超过 `starvation_threshold`）
- 指标定义与聚合逻辑：`sim.rs`

## “能失败的对照测试”与压力测试

- **自旋锁（故意注入 bug）**：
  - `unlock` 不释放 → 超时：见 `controls.rs`
  - “无所有权也算 acquire 成功” → 所有权违规被抓到：见 `controls.rs`
- **睡眠锁（故意注入 bug）**：
  - 去掉 `wakeup` → 死锁：见 `controls.rs`
  - 错误 `wakeup`/`unlock` 顺序（先 wake 后不 release）→ 死锁/超时：见 `controls.rs`
- **正确版本压力测试 + sleep-vs-spin 差异断言**（高竞争下 sleep 的 `context_switches` 更少）：见 `stress.rs`

## 核心实现入口

- 任务脚本/状态机：`model.rs`
- 两类锁模型 + bug 注入开关：`primitives.rs`
- 仿真调度器 + 指标聚合 + 实验生成：`sim.rs`