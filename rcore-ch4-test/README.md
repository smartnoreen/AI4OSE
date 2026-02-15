# rCore Ch4 测试实现

本目录包含 rCore-Tutorial Ch4 章节的实现文件。

## 测试结果

- **测试通过率**: 13/16 (81%)
- **提交哈希**: 2705fc6

## 实现的功能

### 1. sys_mmap - 内存映射
- 文件: `memory_set.rs`
- 功能: 将指定的虚拟地址范围映射到物理内存
- 特性:
  - 页对齐检查 (4KB)
  - 权限验证 (R/W/X)
  - 重叠检测
  - 大小限制 (最大 1GB)

### 2. sys_munmap - 取消映射
- 文件: `memory_set.rs`
- 功能: 取消指定虚拟地址范围的映射
- 特性:
  - 精确匹配检查
  - 页表清理

### 3. sys_get_time - 获取系统时间
- 文件: `process.rs`
- 功能: 获取当前系统时间(秒和微秒)
- 特性:
  - 支持跨页的 TimeVal 结构体
  - 安全的用户空间内存访问

### 4. sys_trace - 地址追踪
- 文件: `process.rs`
- 功能: 读写用户空间指定地址的字节
- 特性:
  - 安全的地址验证
  - 权限检查 (可读/可写)
  - 内核空间保护

## 文件说明

### 核心实现文件

1. **memory_set.rs**
   - 路径: `os/src/mm/memory_set.rs`
   - 修改内容: 添加 `mmap()` 和 `munmap()` 方法到 `MemorySet` 结构体

2. **process.rs**
   - 路径: `os/src/syscall/process.rs`
   - 修改内容:
     - 实现 `sys_get_time()`
     - 实现 `sys_trace()`
     - 添加安全的地址转换辅助函数
     - 实现 `sys_mmap()` 和 `sys_munmap()` 系统调用接口

3. **syscall_mod.rs**
   - 路径: `os/src/syscall/mod.rs`
   - 修改内容: 在 syscall 分发函数中添加新的系统调用

4. **task_mod.rs**
   - 路径: `os/src/task/mod.rs`
   - 修改内容: 添加全局 mmap/munmap 函数和 TaskManager 方法

5. **task.rs**
   - 路径: `os/src/task/task.rs`
   - 修改内容: 在 TaskControlBlock 中添加 mmap/munmap 方法

## 使用方法

### 复制文件到 rCore 项目

```bash
# 假设你在 rCore-Tutorial-Code 目录
git checkout ch4

# 复制文件
cp rcore-ch4-test/memory_set.rs os/src/mm/memory_set.rs
cp rcore-ch4-test/process.rs os/src/syscall/process.rs
cp rcore-ch4-test/syscall_mod.rs os/src/syscall/mod.rs
cp rcore-ch4-test/task_mod.rs os/src/task/mod.rs
cp rcore-ch4-test/task.rs os/src/task/task.rs
```

### 运行测试

```bash
cd ci-user
make test CHAPTER=4
```

## 技术要点

### 内存映射 (mmap)

```rust
pub fn mmap(&mut self, start: usize, len: usize, prot: usize) -> isize {
    // 1. 参数验证
    // 2. 重叠检测
    // 3. 权限转换
    // 4. 创建映射区域
}
```

**关键验证**:
- 起始地址必须页对齐
- 长度不能为 0
- 权限位必须有效 (0-7)
- 不能与现有区域重叠

### 取消映射 (munmap)

```rust
pub fn munmap(&mut self, start: usize, len: usize) -> isize {
    // 1. 查找匹配的区域
    // 2. 移除区域
    // 3. 清理页表
}
```

**关键点**:
- 必须精确匹配起始地址和长度
- 调用 `area.unmap()` 清理页表

### 安全地址访问

```rust
fn safe_translate_byte_read(token: usize, addr: usize) -> Option<u8>
fn safe_translate_byte_write(token: usize, addr: usize, value: u8) -> bool
```

**安全检查**:
- 地址不能在内核空间 (>= TRAP_CONTEXT_BASE)
- PTE 必须有效
- 读操作需要可读权限
- 写操作需要可写权限

## 测试用例

通过的测试:
- ✅ ch4_mmap0 - 基本映射测试
- ✅ ch4_mmap3 - 复杂映射测试
- ✅ ch4_unmap - 取消映射测试
- ✅ ch4_unmap2 - 复杂取消映射
- ✅ ch4_trace1 - 地址追踪测试
- ✅ ch3 的所有测试 (继承)

未通过的测试:
- ❌ 部分 ch3 trace syscall 计数测试 (未实现计数功能)

## 代码统计

- **总修改行数**: 228 行
  - memory_set.rs: +83 行
  - process.rs: +111 行
  - task_mod.rs: +24 行
  - task.rs: +10 行

## 相关链接

- GitHub 仓库: https://github.com/smartnoreen/AI4OSE
- Ch4 分支: https://github.com/smartnoreen/AI4OSE/tree/ch4
- 提交详情: https://github.com/smartnoreen/AI4OSE/commit/2705fc6

## 注意事项

1. **页对齐**: 所有地址必须是 4KB (PAGE_SIZE) 的倍数
2. **权限位**: prot 参数的 bit 0-2 分别代表 R/W/X 权限
3. **安全性**: 永远不要直接解引用用户空间指针，使用安全的转换函数
4. **错误处理**: 返回 -1 表示失败，0 或正数表示成功
