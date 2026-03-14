# locklab

## 实现内容（可观测仿真系统）

- **任务脚本/DSL**（Acquire/Release/CondWait/Signal/Broadcast/Hold/Work）：`model.rs`
- **原语模型**（自旋锁/睡眠锁/条件变量）+ bug 注入开关：`primitives.rs`
- **RR 确定性调度仿真器 + 指标聚合**（锁竞争/平均持锁时间/上下文切换/最大等待/starvation）：`sim.rs`
- 条件变量语义采用 Mesa 风格：`CondWait` 原子释放锁并阻塞；被 `Signal`/`Broadcast` 唤醒后必须重新持锁才算 wait 完成（并用 `max_cond_wait` / `starvation` 统计这段等待）

## 对照失败测试 + 压力/对比测试

- **“能失败的对照测试”**（通过注入 bug 触发 Deadlock/Timeout）：
  - 自旋锁：`unlock` 不释放 → 超时
  - 睡眠锁：去掉 `wakeup` → 死锁
  - 条件变量：`signal` 不唤醒 → 死锁/超时  
    见 `tests/controls.rs`
- **正确版本压力测试 + 自旋睡眠对比断言**（高竞争下自旋锁更容易产生更多 `context_switches`）：见 `tests/stress.rs`

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