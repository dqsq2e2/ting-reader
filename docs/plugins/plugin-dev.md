# 插件开发指南

本文以元数据搜索插件为例，介绍创建项目、填写清单、实现业务、构建、打包、安装联调和发布的完整流程。其他类型的插件使用对应模板，并按[能力声明](./capabilities.md)实现操作。

## 1. 准备工具

从 [Ting Reader 官网](https://www.tingreader.cn) 下载适合开发机系统和 CPU 架构的 `trpack`，解压后将工具所在目录加入 PATH，再确认命令：

```sh
trpack --version
trpack --help
trpack new --help
```

Windows 也可以在解压目录用 PowerShell 执行 `.\trpack.exe --help`；Linux/macOS 为解压出的文件添加执行权限后使用 `./trpack --help`。后文命令以工具已加入 PATH 为例。

JavaScript 开发按需要安装 Node.js 和项目构建工具；WASM、Native 开发安装 Rust 工具链，推荐 Rust 1.93 或更高版本。准备一个 Ting Reader 测试服务端，用于安装插件、配置和联调。

## 2. 创建项目

JavaScript 用于服务端脚本逻辑，入口采用 ESM；WASM 使用 `wasm32-wasip1`；Native 按服务器操作系统和 CPU 架构发布库文件。客户端 HTML 界面与服务端业务运行时分别构建，三种运行时都可以声明 UI 能力。

| 模板 | 能力 |
| --- | --- |
| `metadata` | 元数据搜索和刮削 |
| `format` | 声明式扩展格式识别、元数据提取及可选写回 |
| `ui` | 客户端页面或动作入口 |
| `route` | 插件 HTTP 路由 |
| `store` | 插件商店 |
| `content` | 内容处理 |
| `tool` | 带输入/输出 schema 的工具 |
| `task` | 任务处理 |
| `event` | 事件处理 |

选择一种运行时，在独立开发目录中创建项目。JavaScript：

```sh
trpack new my-plugin --template metadata --runtime javascript --id my-plugin --version 1.0.0
```

WASM：

```sh
trpack new my-plugin --template metadata --runtime wasm --id my-plugin --version 1.0.0
```

Native：

```sh
trpack new my-plugin --template metadata --runtime native --id my-plugin --version 1.0.0
```

编辑项目中的 `plugin.yml`。JavaScript 业务入口为 `plugin.js`，Rust 业务入口为 `src/lib.rs`。界面插件的页面、脚本和样式放到 `ui/`，入口路径与清单保持一致。

## 3. 填写清单

下面是 JavaScript 元数据插件的清单。WASM、Native 项目保留所选运行时的 `runtime` 和 `entry_point`，按实际业务调整能力和权限。

```yaml
id: my-plugin
name: My Plugin
version: 1.0.0
min_core_version: 2.0.0
author: Your Name
description: { zh: 元数据示例, en: Metadata example }
runtime: javascript
entry_point: plugin.js
capabilities:
  - id: metadata.search
    kind: metadata_provider
    operations: [search]
    auto_scrape: true
    search_fields:
      - key: title
        label: { zh: 书名, en: Title }
        type: text
        required: true
        default_from: book.title
    result_fields:
      - key: title
        label: { zh: 书名, en: Title }
permissions:
  - type: network_access
    domain: example.com
```

`version` 是插件自身版本；`min_core_version` 是最低主程序版本，至少为 `2.0.0`，实际使用更高版本的功能时相应提高。需要限制 Flutter 客户端版本时，可另外填写 `min_flutter_version`。

能力操作名称、输入输出和权限作用域按下方对应指南填写。工具能力使用固定 `invoke: invokeTool`；网络、文件、事件分别声明 `domain`、`path`、`event` 作用域。声明权限是访问条件之一，实际业务请求还受当前用户、实例和资源范围限制。

- [能力声明](./capabilities.md)：九类能力与固定操作。
- [HostGateway](./hostgateway.md)：宿主方法、权限和资源生命周期。
- [配置指南](./plugin-config.md)：配置 schema、表单和敏感字段。
- [界面与日志](./ui-logging-migration.md)：页面桥接、可运行示例和日志排查。

## 4. 实现业务

业务入口实现清单声明的操作。元数据 `search` 接收搜索条件，返回分页书籍结果；字段和限额见[元数据搜索接口](./capabilities.md#元数据搜索接口)。以下例子将搜索标题返回为一条结果，实际开发时替换为源站请求和解析逻辑。

访问宿主数据时，JavaScript 使用 `Ting.host.invoke(method, params)`，Rust 使用 `host.invoke(method, params)`，在清单中声明相应权限。媒体和 HTTP 正文通过宿主资源 ID 读写，使用完关闭资源。

### JavaScript

编辑 `my-plugin/plugin.js`：

```js
import { success, publishSearch } from './sdk.mjs';

export async function search(request) {
  const items = request.title ? [{ id: null, title: request.title }] : [];
  return success(publishSearch({
    items,
    total: null,
    has_more: null,
  }, request));
}
```

运行时支持签名包内的相对 `.js` / `.mjs` 导入。第三方依赖在开发机通过构建工具打包到包内；服务端执行已发布的模块。`trpack build` 收集入口的相对模块依赖，不执行 npm 安装、前端构建或 TypeScript 编译。

完整说明见 [JavaScript 运行时](./js_runtime_guide.md)。

### Rust 实现

WASM、Native 元数据项目的 `my-plugin/Cargo.toml`：

```toml
[dependencies]
ting-plugin-sdk = { git = "https://github.com/dqsq2e2/ting-plugin-sdk.git", tag = "v2.0.5" }
ting-scraper-sdk = { git = "https://github.com/dqsq2e2/ting-scraper-sdk.git", tag = "v2.0.5" }
serde_json = "1"
```

编辑 `my-plugin/src/lib.rs`：

```rust
use serde_json::{Value, json};
use ting_plugin_sdk::{Host, Plugin, Result, SdkError};
use ting_plugin_sdk::contract::protocol::PluginErrorCode;
use ting_plugin_sdk::contract::scraper::SearchRequest;

#[derive(Default)]
struct MyPlugin;

impl Plugin for MyPlugin {
    const ID: &'static str = "my-plugin";
    const OPERATIONS: &'static [&'static str] = &["search"];

    fn invoke(&mut self, operation: &str, input: Value, _host: &dyn Host) -> Result<Value> {
        if operation != "search" {
            return Err(SdkError::new(
                PluginErrorCode::UnsupportedOperation,
                "Unsupported operation",
            ));
        }
        let request: SearchRequest =
            serde_json::from_value(input.clone()).map_err(SdkError::parse)?;
        request.validate().map_err(SdkError::invalid)?;
        let items = request.title.as_deref()
            .filter(|title| !title.trim().is_empty())
            .map(|title| vec![json!({ "id": null, "title": title })])
            .unwrap_or_default();
        ting_scraper_sdk::publish_search_for_request(&input, json!({
            "items": items,
            "page": request.page,
            "page_size": request.page_size,
            "total": null,
            "has_more": null,
        }))
    }
}

ting_plugin_sdk::export_plugin!(MyPlugin);
```

`ID` 与清单的 `id` 一致，`OPERATIONS` 覆盖声明的操作。`invoke` 根据操作名称处理业务，`export_plugin!` 提供运行时入口。根据源站实际分页返回 `page`、`page_size`；未知总数和是否还有下一页使用 `null`。

## 5. 检查与构建

按所选运行时执行以下命令。

### JavaScript

```sh
node --check my-plugin/plugin.js
```

TypeScript、第三方依赖和界面项目按各自的构建命令生成发布文件，确保清单中的入口指向构建产物。

### WASM

```sh
cd my-plugin
rustup target add wasm32-wasip1
cargo check --target wasm32-wasip1
cargo clippy --target wasm32-wasip1 -- -D warnings
cargo build --release --target wasm32-wasip1
cd ..
```

将 `target/wasm32-wasip1/release/ting_plugin.wasm` 复制到项目根目录的 `ting_plugin.wasm`，与 `entry_point` 一致。PowerShell 使用：

```powershell
Copy-Item -LiteralPath .\my-plugin\target\wasm32-wasip1\release\ting_plugin.wasm -Destination .\my-plugin\ting_plugin.wasm
```

完整说明见 [WASM 运行时](./wasm_runtime_guide.md)。

### Native

```sh
cd my-plugin
cargo check
cargo clippy -- -D warnings
cargo build --release
cd ..
```

Windows 将 `target/release/ting_plugin.dll` 复制到根目录；Linux/macOS 分别复制 `libting_plugin.so` / `libting_plugin.dylib`。`entry_point` 应使用实际产物名称。跨平台分发时为每个服务器操作系统、CPU 架构构建对应包，`--platform-tag` 只是包命名参数，不会执行交叉编译。完整说明见 [Native 运行时](./native_runtime_guide.md)。

## 6. 签名、打包和检查

以下命令在各插件项目的上一级目录执行。先生成一次稳定的发布者密钥：

```sh
trpack keygen --key-id example-publisher --output keys/publisher.private.json --public-output keys/publisher.public.json
```

私钥不要放到插件目录、Git 或发布资产中。同一插件后续发布使用同一受控签名身份；密钥轮换需与部署端信任配置配合。

```sh
trpack validate my-plugin
trpack build my-plugin --output dist/my-plugin-1.0.0.tr --sign-key keys/publisher.private.json
trpack inspect dist/my-plugin-1.0.0.tr
trpack verify dist/my-plugin-1.0.0.tr
```

`trpack build` 将已有产物打包并签名，自动包含：

- 根目录规范化后的 `plugin.yml`；
- `entry_point` 指定的业务入口；
- JavaScript 入口中发现的相对模块导入；
- `web_container` HTML 入口及其所在目录。

其他业务资产用 `--include` 显式加入：

```sh
trpack build my-plugin --include assets --output dist/my-plugin-1.0.0.tr --sign-key keys/publisher.private.json
```

静态导入和字符串字面量动态导入会被收集；运行时计算出的模块路径及其他动态加载资产需要明确纳入发布包。`--include` 路径相对插件根目录。

不传 `--sign-key` 会生成一次性密钥，适合临时测试包；正式发布使用稳定私钥。`verify` 校验包结构、哈希和签名，发行者是否受信任仍由部署端决定。

检查包内资产可以解包到独立目录：

```sh
trpack unpack dist/my-plugin-1.0.0.tr --output unpacked/my-plugin
```

## 7. 安装与联调

1. 在测试服务端的插件管理页上传 `.tr` 包。安装是管理员操作，宿主检查结构、签名、发布者、最低版本、能力和入口。
2. 未受信任的发布者按客户端安装确认流程处理；安装后自动加载插件，完成配置并确认初始化成功。
3. 从实际入口触发能力，例如元数据搜索、工具、HTTP 路由或 UI 页面。验证成功结果、空结果、错误响应、未授权请求及资源释放。
4. 在插件日志中按插件、来源和操作定位。UI 插件分别验证 Web 和 Flutter，包括明暗主题、语言、书籍上下文和关闭重开。
5. 修改代码后重新构建产物、打包、验签、安装，再触发业务。安装目录中的签名文件不作为源码编辑目录。

“重新加载”会重新创建已安装插件的运行时；要部署新代码，应先安装新构建的包。当前 CLI 的联调方式为安装包、调用能力和查看宿主日志，不提供独立的宿主模拟器或 `dev/watch` 命令。

## 8. 发布流程

提升插件自己的 `version`，保持稳定 `id`；如果新增功能要求更高宿主版本，再提升 `min_core_version` 或 `min_flutter_version`。重新编译、签名并验证包，在独立插件仓库的 Release 中发布 `.tr` 资产。Native 包按平台分别发布。

CI 流程按运行时执行：检出插件源码 → 安装所需编译工具链和下载 `trpack` → 测试与构建 → `trpack validate` → 使用受控私钥打包 → `inspect` / `verify` → 上传资产。私钥通过 CI Secret 注入临时文件，结束后清理，不上传为 artifact。

发布包只包含插件运行所需的文件；构建缓存、测试密钥和签名私钥保留在发布包之外。
