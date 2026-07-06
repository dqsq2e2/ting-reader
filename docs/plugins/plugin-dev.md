# 插件开发指南

Ting Reader 插件开发的关键是：在 `plugin.yml` 声明 capability 和 permissions，在插件代码里通过 HostGateway 调用系统能力，再按 JavaScript、WASM、Native 的桥接差异完成入口和打包。开发者最常写的是“插件如何安全读取书籍、进度、媒体、库文件、缓存和任务”，而不是绕过宿主直接读数据库或拼路径。

先区分两个方向：

- Capability：注册插件能力，决定系统什么时候调用插件。
- HostGateway：插件访问宿主系统，决定插件怎么安全读取或操作系统数据。

## 1. 插件如何调用系统能力

插件不能直接读数据库、拼媒体路径或绕过用户权限。需要读取书籍、章节、进度、媒体地址、存储库文件、缓存或创建任务时，统一调用 HostGateway。HostGateway 会检查 manifest 权限、当前登录用户、用户能访问的书籍或存储库、管理员写权限，以及文件路径是否仍在存储库根目录内。

不同运行时调用的是同一组系统能力，只是桥接方式不同：

1. JavaScript 后台方法使用 `Ting.host.invoke(method, params)`。
2. `web_container` UI 使用 `postMessage` 发送 `method: "host.invoke"`，客户端再转发到 `/api/v1/plugin-host/invoke`。
3. WASM 使用 `ting_env.host_invoke`，再通过 `host_response_size` 和 `host_read_body` 读取 JSON 结果。
4. Native 动态库通过 `plugin_set_host_api` 接收 Host API，再调用 `host_invoke(method, params_json, result_json)`。

## 2. 最小但完整的 manifest

```yaml
id: assistant-tools
name: Assistant Tools
version: 1.0.0
min_core_version: 1.4.8
runtime: javascript
entry_point: plugin.js
author: Your Name
description:
  zh: 提供客户端入口、工具和后台任务
  en: Provides client entries, tools, and background tasks

capabilities:
  - id: assistant.panel
    kind: ui_extension
    invoke: openAssistant
    slot: global.floating_action
    title: { zh: AI 助手, en: AI Assistant }
    icon: message-circle
    render:
      mode: web_container
      entry: ui/index.html

  - id: books.tools
    kind: tool_provider
    invoke: invokeTool
    tools:
      - name: books.search
        description: Search books the current user can access

  - id: batch.summarize
    kind: task_handler
    invoke: runTask
    task_types: [book.summarize]

permissions:
  - type: books_read
  - type: progress_read
  - type: task_create
  - type: cache_read
  - type: cache_write
```

`id@version` 会成为运行时实例 id，例如 `assistant-tools@1.0.0`。如果没有显式写 `runtime`，后端会根据 `entry_point` 扩展名推断：`.js` 是 JavaScript，`.wasm` 是 WASM，`.dll/.so/.dylib` 是 Native。

## 3. Capability 与 HostGateway 速览

主指南只保留概念入口：

- Capability 写在 `capabilities` 中，声明插件提供什么能力、由谁调用、调用哪个运行时函数、是否显示客户端 UI。
- HostGateway 是插件访问宿主数据的唯一安全入口，例如读取书籍、章节、进度、媒体地址、库文件、缓存、播放列表或用户设置。
- 运行时文件不决定插件类型，`capabilities[].kind` 才决定插件能做什么。
- 所有 HostGateway 调用都需要在 `permissions` 中声明对应权限；缺权限、缺用户上下文或越权访问都会被拒绝。

详细 kind、slot、render mode、HTTP route、任务、事件和 UI 图标写法，请看：[插件能力声明](./capabilities.md)。HostGateway 方法参数、响应格式、Web 容器桥接、WASM/Native 错误码和权限表，请看：[HostGateway 能力调用详解](./hostgateway.md)。

## 4. 选择运行时

- JavaScript 运行时适合快速实现 API 接入、元数据处理、工具、插件商店源和轻量 UI 后端逻辑。详见：[JavaScript 运行时](./js_runtime_guide.md)。
- WASM 运行时适合跨平台 Rust 逻辑、内容处理和计算型任务。详见：[WASM 运行时](./wasm_runtime_guide.md)。
- Native 运行时适合格式处理、流式解密、系统库调用和平台二进制工具供应。详见：[Native 运行时](./native_runtime_guide.md)。

## 5. 插件配置

需要 API key、接口地址、模型名、开关、枚举选项或数值参数时，在 `plugin.yml` 里声明 `config_schema`。配置表单、敏感字段加密、默认值、运行时读取方式和常见坑请看：[插件配置 `config_schema`](./plugin-config.md)。

## 6. trpack 打包

官网 public 目录已经提供 `trpack` 二进制下载：

- Windows x86: [`trpack-1.0.2-windows-amd64`](https://www.tingreader.cn/trpack/trpack-1.0.2-windows-amd64)
- Linux x86: [`trpack-1.0.2-linux-amd64`](https://www.tingreader.cn/trpack/trpack-1.0.2-linux-amd64)
- Linux ARM: [`trpack-1.0.2-linux-arm64`](https://www.tingreader.cn/trpack/trpack-1.0.2-linux-arm64)
- Mac Intel: [`trpack-1.0.2-darwin-amd64`](https://www.tingreader.cn/trpack/trpack-1.0.2-darwin-amd64)
- Mac M系列: [`trpack-1.0.2-darwin-arm64`](https://www.tingreader.cn/trpack/trpack-1.0.2-darwin-arm64)

下载后建议重命名为 `trpack` 或 `trpack.exe`，并放到 PATH 中。Linux 和 macOS 需要先执行 `chmod +x trpack`。

`trpack` 的常用能力：

| 命令 | 用途 |
| --- | --- |
| `init` | 按模板创建插件项目，模板包括 `metadata`、`format`、`ui`、`route`、`content`、`tool` |
| `validate` | 校验插件目录和 `plugin.yml/plugin.yaml` |
| `build` / `pack` | 构建 `.tr` 包，可用 `--include` 添加额外文件、`--json` 输出机器可读摘要 |
| `keygen` | 生成 Ed25519 发布密钥，保持后续升级的发布者身份稳定 |
| `sign` | 给已有 `.tr` 包重新签名 |
| `inspect` | 查看 `.tr` 包元数据、文件表和签名摘要，支持 `--json` |
| `verify` | 校验 `.tr` 包结构、manifest、文件表和签名状态 |
| `unpack` | 解包到目录，便于调试包内文件 |
| `is-tr` | 快速判断文件是否是 `.tr` 插件包 |

```bash
trpack init my-plugin --template ui --id my-plugin --name "My Plugin"
trpack validate my-plugin
trpack build my-plugin --output dist/my-plugin.tr --json
trpack inspect dist/my-plugin.tr --json
trpack unpack dist/my-plugin.tr --output unpacked
trpack is-tr dist/my-plugin.tr
```

插件项目可以独立于主仓库维护。发布前至少执行创建、校验、打包和验证：

```bash
trpack init my-plugin --template tool --id my-plugin --name "My Plugin"
trpack validate my-plugin
trpack build my-plugin --output dist/my-plugin.tr
trpack verify dist/my-plugin.tr
```

公开发布插件时建议使用稳定签名密钥，确保后续升级保持同一发布者身份：

```bash
trpack keygen --key-id my-plugin-release --output keys/private.json --public-output keys/public.json
trpack build . --output dist/my-plugin.tr --sign-key keys/private.json
trpack sign dist/my-plugin.tr --key keys/private.json --output dist/my-plugin.signed.tr
trpack verify dist/my-plugin.signed.tr
```

安装器会检查 `.tr` 包格式、manifest、文件表、签名元数据、服务端版本要求、依赖插件和发布者身份。当前安装路径只接受有效 `.tr` 包；未签名包会被拒绝，未受信但签名有效的包需要用户确认后才能安装。同一个插件 id 如果来自不同发布者身份，需要先卸载旧插件再安装。

不要把源码目录直接复制到服务端插件安装目录作为发布方式。安装 `.tr` 时宿主会写入 `.trpack/package.json` 和 `.trpack/signature.json`；启动发现插件时会重新校验这些元数据、文件大小和 sha256。直接改安装目录里的文件可能导致签名校验失败或启动时被跳过。开发调试后应重新 `trpack build`/`sign` 并通过插件管理页重装或升级。
