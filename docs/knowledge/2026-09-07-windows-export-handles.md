---
status: canonical
content_stage: implementation-backed
retrieved: 2026-09-07
freshness_class: normal
valid_until: 2026-09-17
confidence: high
---

# Windows 普通导出的清理权限与文件验证

CI `34052877839` 的 Windows 全产品任务在 Tauri 单测阶段为 689 passed / 1 failed；唯一失败是普通输出清理，`SetFileInformationByHandle(FileDispositionInfo)` 返回 Access denied。普通输出打开函数没有申请 DELETE 权限，而保留到工程的输出打开函数已有该权限。

[Microsoft 文档](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle)明确要求设置删除 disposition 的句柄在 CreateFile 时申请 DELETE；文件删除在相关句柄关闭后完成。当前修复为普通输出申请 GENERIC_READ / GENERIC_WRITE / DELETE，同时继续拒绝 FILE_SHARE_DELETE，不给外部句柄新增替换文件名的能力。

输出验证改用已有的 `probe_file`，经继承的普通文件句柄和 `fd:` 协议读取同一份输出，避免路径重开与 Windows CRT 共享模式冲突。[FFmpeg 6.1.1 file 协议源码](https://raw.githubusercontent.com/FFmpeg/FFmpeg/n6.1.1/libavformat/file.c)包含 fd 协议；项目锁定 Windows FFmpeg/ffprobe 6.1.1，macOS ARM64 为 7.0。没有升级 sidecar 或扩大文件共享权限。

普通和 reserved 输出都保留验证所需的 File；`enabled` 继续决定谁负责删除，reserved 输出仍由外层 `ProjectMediaOutput` 管理。回归使用实际生产 opener，断言清理先把仍被 encoder clone 持有的文件截为零，再在最后 clone 关闭后确认文件不存在；另有真实 WAV 的保留句柄 probe/cleanup 用例。Windows 原生验证结果写入当日总验收，不能用 macOS 通过代替。
