# locklab

## 位置与运行

- 新目录（独立 Rust crate）：自旋锁
- 运行测试（正确版本必须通过）：在该目录执行 `cargo test`

## 系统做了什么

- **确定性调度仿真器**：用时间步进 + RR runnable 队列执行“任务脚本”（Acquire/Hold/Release/Work）
  - 核心：`sim.rs`、`model.rs`
- **两种同步原语模型**
  - 自旋锁：失败则保持 runnable（相当于“yield/继续抢”）
  - 睡眠锁：失败则进入 Blocked，并由 unlock 唤醒/转移所有权
  - 实现与可注入 bug 开关：`primitives.rs`

## 观测指标（Metrics）

- 锁竞争次数：`contentions`
- 平均持锁时间：`avg_hold_time()`（由 `hold_time_total` / `acquisitions` 得出）
- 上下文切换次数（睡眠锁 vs 自旋锁差异）：`context_switches`
- 公平性：最大等待时间 `max_wait`
- starvation：`starvation`（等待超过 `starvation_threshold` 即置位）
- 指标类型定义：`Metrics`

## “能失败的对照测试”与压力测试

- **自旋锁对照测试（会失败）**：
  - `unlock` 不释放 → 超时：`controls.rs`
  - `acquire` 可“无所有权成功” → 所有权违规被抓到：`controls.rs`
- **睡眠锁对照测试（会失败）**：
  - 去掉 `wakeup` → 死锁：`controls.rs`
  - 错误 `wakeup`/`unlock` 顺序（先 wake 后不 unlock）→ 死锁/超时：`controls.rs`
- **正确版本压力测试与对比测试（必须通过）**：`stress.rs`

> 如果你希望把“任务脚本”扩展成更贴近 rCore 的 sleep/wakeup 或加入更多原语（如 condvar、semaphore），我可以按当前的 Sim/Action 框架直接加新原语与对应的失败对照测试矩阵。