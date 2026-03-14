# locklab

locklab 是一个用 Rust 编写的可观测、可注入故障、可压力测试的同步原语实验系统。它通过确定性 RR 调度仿真器，对比**自旋读写锁**与**睡眠读写锁**的行为差异，并收集详细的性能与公平性指标。

## 实现内容（可观测 + 可失败对照 + 压测）

- **仿真系统**（确定性 RR 调度）：[`sim.rs`](sim.rs)
- **读写锁原语**（自旋 vs 睡眠）+ bug 注入开关：[`primitives.rs`](primitives.rs)
- **任务脚本/状态机**（读锁/写锁/释放/工作负载）：[`model.rs`](model.rs)

## 观测指标（Metrics）

所有指标通过 `Metrics` 结构体汇总，定义与聚合逻辑位于 `sim.rs`。

| 指标 | 描述 |
|------|------|
| `contentions` | 锁竞争次数 |
| `avg_hold_time()` | 平均持锁时间（总持锁时间 / 成功获取次数） |
| `avg_read_hold_time()` | 读锁平均持锁时间 |
| `avg_write_hold_time()` | 写锁平均持锁时间 |
| `context_switches` | 上下文切换次数（用于 sleep vs spin 差异对比） |
| `max_wait` | 全局最大等待时间 |
| `max_read_wait` | 读锁最大等待时间 |
| `max_write_wait` | 写锁最大等待时间 |
| `starvation` | 是否发生饥饿（任一任务等待时间超过 `starvation_threshold`） |

## 测试（符合“能失败的对照测试”要求）

- **对照失败测试**（注入 bug 后必然触发 deadlock/timeout/违规）：[`tests/controls.rs`](tests/controls.rs)  
  包含对自旋锁、睡眠锁、条件变量的错误变体测试（如 unlock 不释放、无唤醒、唤醒顺序错误等）。
- **压力测试 + sleep-vs-spin 差异断言**（高竞争下 sleep 锁的 `context_switches` 更少）：[`tests/stress.rs`](tests/stress.rs)
- **说明文档**（指标口径、入口、测试矩阵）：本文档 [`README.md`](README.md)
