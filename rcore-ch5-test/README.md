# rCore Ch5 测试实现

本目录完成了 rCore-Tutorial ch5 的测试题，通过率为 **14/15**。

## 实现的功能

### Ch5 新功能
1. **sys_spawn** - 创建新进程系统调用
   - 不同于 fork + exec，直接加载新程序
   - 创建新的地址空间并执行指定程序

2. **sys_set_priority** - 设置进程优先级
   - 支持动态调整进程优先级
   - 最小优先级限制为 2

### Ch4 补充功能
3. **sys_get_time** - 获取系统时间
   - 返回秒和微秒
   - 使用虚拟内存管理正确处理用户空间指针

4. **sys_mmap** - 内存映射
   - 支持 R/W/X 权限控制
   - 检查地址对齐和重叠
   - 创建匿名内存映射

5. **sys_munmap** - 取消内存映射
   - 精确匹配起始地址和长度
   - 正确释放映射的物理页面

## 测试结果

### 通过的测试 (14/15)
- ✅ Test spawn0 OK - spawn 系统调用基本功能
- ✅ Test wait OK - 等待子进程
- ✅ Test waitpid OK - 按 PID 等待子进程
- ✅ Test set_priority OK - 设置优先级
- ✅ Test sleep OK - 睡眠功能
- ✅ Test sleep1 passed - 睡眠精度测试
- ✅ Test get_time OK - 获取时间
- ✅ Test 04_1 OK - mmap 基本功能
- ✅ Test 04_4 test OK - mmap 错误处理
- ✅ Test 04_6 ummap2 OK - munmap 错误处理
- ✅ 以及 ch2/ch3 的所有测试

### 未通过的测试 (1/15)
- ❌ Test 04_5 ummap OK - ch4_unmap 复杂场景测试
  - 问题：连续 mmap 相邻区域时第二次失败
  - 状态：正在调试中

## 主要修改的文件

### 1. os/src/syscall/process.rs
实现了 5 个新的系统调用:
- `sys_spawn` - 创建并执行新进程
- `sys_set_priority` - 设置进程优先级
- `sys_get_time` - 获取系统时间
- `sys_mmap` - 内存映射
- `sys_munmap` - 取消内存映射

### 2. os/src/task/task.rs
- 添加了 `spawn` 方法到 `TaskControlBlock`
- 在 `TaskControlBlockInner` 中添加了 `priority` 字段
- 所有任务创建时初始化优先级为 16

### 3. os/src/mm/memory_set.rs
- 添加了 `munmap_area` 方法，用于精确移除指定范围的内存区域
- 返回布尔值指示操作是否成功

## 实现细节

### spawn 实现
```rust
pub fn spawn(self: &Arc<Self>, elf_data: &[u8]) -> Arc<Self> {
    // 1. 从 ELF 创建新的地址空间
    // 2. 分配新的 PID 和内核栈
    // 3. 创建新的 TaskControlBlock
    // 4. 添加到父进程的子进程列表
    // 5. 初始化 trap context
}
```

### set_priority 实现
```rust
pub fn sys_set_priority(prio: isize) -> isize {
    if prio < 2 {
        return -1;  // 优先级最小为 2
    }
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    inner.priority = prio as usize;
    prio
}
```

### mmap 实现
- 检查地址页面对齐
- 检查权限位有效性 (只允许 R/W/X 组合)
- 检查是否与现有映射重叠
- 创建 MapArea 并映射到页表

### munmap 实现
- 检查地址页面对齐
- 查找精确匹配起始地址和长度的 MapArea
- 如果找到则 unmap 并移除，否则返回错误

## 构建和测试

### 编译内核
```bash
cd os
cargo build --release
```

### 运行测试
```bash
cd ci-user
make test CHAPTER=5
```

### 运行内核
```bash
cd os
make run
```

## 技术要点

1. **spawn vs fork+exec**
   - spawn 直接创建新进程并加载程序
   - 不需要复制父进程的地址空间
   - 更高效，适合创建独立的子进程

2. **内存映射管理**
   - 使用 VPNRange 管理虚拟页面范围
   - MapArea 封装了映射区域的属性和数据帧
   - 支持 Framed 类型映射（按需分配物理页面）

3. **优先级调度**
   - 每个任务维护一个 priority 字段
   - 可以动态调整优先级
   - 支持优先级继承（fork 时子进程继承父进程优先级）

## 待解决的问题

ch4_unmap 测试失败的原因正在调查中，可能的方向：
1. 页表映射时机问题
2. 重叠检查逻辑需要优化
3. MapArea 范围计算可能有边界情况

## 参考资料

- [rCore-Tutorial-Guide](https://LearningOS.github.io/rCore-Tutorial-Guide/)
- [rCore-Tutorial-Book-v3](https://rcore-os.github.io/rCore-Tutorial-Book-v3/)
- [rCore-Tutorial-Code](https://github.com/LearningOS/rCore-Tutorial-Code)
