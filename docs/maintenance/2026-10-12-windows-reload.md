# Windows 一体启动脚本与热重载信号

后续：Quick Dev v1.0.2 已增加软件内的标准 Uvicorn 入口自动兼容，无需逐项目添加本页描述的专用启动器。支持范围和软件层回归见 [1.0.2 说明](../releases/1.0.2.md)。下文保留为先前项目脚本修复的诊断记录。

## 诊断范围

艾必模的 dev:stack 使用 PowerShell 同时管理 Uvicorn 和 npm/Vite。Uvicorn 0.52.1 的 BaseReload.restart 在 Windows 上向服务子进程发送 CTRL_C_EVENT。原启动脚本通过 Start-Process -NoNewWindow 启动 Python，会共用控制台；npm 意外结束后 finally 清理 API，是报告中的退出路径。没有完整事故日志，不能将历史事故判定为百分之百复现。

Quick Dev 当前源码使用 CREATE_NO_WINDOW 启动外层 Shell，进程执行前加入 Windows Job Object；停止通过 TerminateJobObject 回收受管进程树。Job 的进程归属与控制台是否共享是两件事，独立控制台不需要脱离进程树。

## 本地修复

艾必模 scripts/quickdev-start.ps1 使用 scripts/lib/start-isolated-python.ps1 和 isolated-python.py：

- 后端解释器运行在 CREATE_NEW_CONSOLE + SW_HIDE 的独立控制台中，不使用 CREATE_BREAKAWAY_FROM_JOB。
- 保留虚拟环境，包装进程返回真实后端退出码，保留父子进程关系。
- stdout/stderr 经并行读取转发到外层输出，统一 UTF-8，保留 Quick Dev 日志。
- 重定向日志时，Uvicorn 的控制台唤醒写入同时到达独立 CONOUT$，否则隔离试验出现重载等待超时。

原 npm、就绪检查、路由指纹检查、端口处理和停止逻辑保留。本次没有修改 Quick Dev 应用二进制，没有自动启停真实项目，没有提交或发布艾必模的混合工作区。

## 验证与使用

Quick Dev 项目内 _tasks/2026-10-12-console-reload 保存临时 ASGI 工程、运行器、stdout/stderr 与 evidence.json。使用项目实际 Python 虚拟环境和 Uvicorn，动态分配独立端口，验证三次真实热重载、前端模拟进程存活、日志转发和进程树停止后端口释放。不是订单业务验收，也不是已安装 Quick Dev 的原生 UI 操作验收。

修改只对下一次 dev:stack 启动生效，当前运行进程不会自动变化。若改用 Quick Dev 原生多服务方案，可将网页前端和后端分别配置为两个服务，让软件独立管理启停、就绪与日志；不要同时运行一体脚本和同样的拆分服务。

Win32 原理参见 [Process Creation Flags](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags)。
