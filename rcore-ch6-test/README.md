# rCore Ch6 文件系统测试实现

## 实现概述

本次实现完成了 rCore 教程第六章（Ch6）的文件系统相关系统调用，主要包括：

### 实现的系统调用

1. **sys_fstat** - 获取文件状态信息
   - 功能：获取文件的元数据（设备号、inode号、文件类型、硬链接数等）
   - 文件：`syscall_fs.rs`
   - 实现要点：
     - 验证文件描述符有效性
     - 调用文件对象的 fstat 方法获取信息
     - 将 Stat 结构体安全地复制到用户空间

2. **sys_linkat** - 创建硬链接
   - 功能：为现有文件创建新的硬链接（别名）
   - 文件：`syscall_fs.rs`
   - 实现要点：
     - 在目录中添加新的目录项指向现有 inode
     - 增加文件的硬链接计数（nlink）
     - 验证新文件名不存在，旧文件存在

3. **sys_unlinkat** - 删除硬链接
   - 功能：删除文件的一个硬链接
   - 文件：`syscall_fs.rs`
   - 实现要点：
     - 从目录中删除指定的目录项
     - 减少文件的硬链接计数
     - 当 nlink 为 0 时，文件数据会被实际删除

### 文件系统层面的修改

#### 1. DiskInode 结构（layout.rs）
- 添加 `nlink: u32` 字段用于跟踪硬链接数
- 初始化时设置 nlink = 1
- 支持持久化存储链接计数

#### 2. VFS 层（vfs.rs）
添加以下方法支持链接操作：
- `get_inode_id()` - 获取 inode 的唯一标识
- `get_disk_inode_info()` - 获取 inode 信息（nlink, size, mode）
- `link()` - 实现目录内的硬链接创建
- `unlink()` - 实现硬链接删除
- `inc_nlink()` - 增加链接计数
- `dec_nlink()` - 减少链接计数
- `clear_nlink()` - 清除链接计数

#### 3. OS inode 层（inode.rs）
- `fstat()` - 实现 OSInode 的文件状态查询
- `link_file()` - 封装 VFS 层的 link 操作
- `unlink_file()` - 封装 VFS 层的 unlink 操作

#### 4. 文件系统接口（fs_mod.rs）
- 为 File trait 添加 `fstat()` 方法
- 导出 `link_file` 和 `unlink_file` 函数
- 定义 `Stat` 结构体和 `StatMode` 标志

## 测试结果

所有测试程序成功运行并正常退出（exit code 0）。

### 测试说明

由于 Ch6 中 spawn 系统调用的实现特点，子进程的输出不会直接显示在终端，但可以通过以下方式验证：
- 所有测试进程的退出码均为 0
- 最终显示 "ch6 Usertests passed!" 消息
- 测试程序包括：fstat、link、unlink 等文件系统操作

## 技术亮点

### 1. 硬链接的实现
- 多个目录项可以指向同一个 inode
- 只有当所有链接都被删除（nlink = 0）时，文件数据才真正释放
- 实现了完整的引用计数机制

### 2. 文件状态查询
- 返回完整的文件元数据
- 支持区分文件类型（普通文件 vs 目录）
- 正确计算 inode 号（基于 block_id 和 block_offset）

### 3. 安全性考虑
- 验证文件描述符有效性
- 检查用户空间指针的合法性
- 使用 translated_byte_buffer 安全地访问用户内存
- 避免重复链接和无效删除操作

## 文件说明

1. **layout.rs** - 磁盘 inode 布局定义
   - DiskInode 结构体定义
   - nlink 字段的添加和初始化

2. **vfs.rs** - 虚拟文件系统层
   - Inode 结构的方法实现
   - 链接和取消链接的核心逻辑

3. **inode.rs** - OS 层 inode 封装
   - OSInode 的文件操作实现
   - 系统调用接口函数

4. **fs_mod.rs** - 文件系统模块接口
   - File trait 定义
   - Stat 结构体定义
   - 公共接口导出

5. **syscall_fs.rs** - 文件系统系统调用
   - sys_fstat 实现
   - sys_linkat 实现
   - sys_unlinkat 实现

## 实现时间

2026-02-15

## 作者

smartnoreen

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
