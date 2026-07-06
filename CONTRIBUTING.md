# 贡献指南 (Contributing Guide)

感谢你对 **Ting Reader** 的关注！这是一个开源项目，我们非常欢迎任何形式的贡献，无论是修复 Bug、增加新功能、改进文档，还是开发新的插件。

## 🏗️ 架构概览

在开始贡献之前，请先了解项目的整体架构：

*   **后端 (`/backend`)**: 使用 **Rust** 编写，基于 `Axum` 框架。负责核心业务逻辑、数据库管理、音频流处理、插件运行时及 API 服务。
*   **前端 (`/frontend`)**: 使用 **React + TypeScript + Tailwind CSS** 编写。负责用户界面交互和状态管理。
*   **插件 (`/plugins`)**: 包含官方维护的 JS/WASM/Native 插件。

## 🛠️ 环境准备

要开始开发，你需要安装以下工具：

1.  **Node.js**: v22 或更高版本 (用于前端开发)
2.  **Rust**: v1.75 或更高版本 (用于后端开发)
3.  **SQLite3**: 用于数据库管理 (可选，Rust 包含编译时绑定)
4.  **Git**: 版本控制

## 🚀 本地开发流程

### 1. 克隆仓库

```bash
git clone https://github.com/dqsq2e2/ting-reader.git
cd ting-reader
```

### 2. 启动后端 (Rust)

后端服务默认运行在 `http://localhost:3000`。

```bash
cd backend

# (可选) 复制示例配置文件
# cp config.example.toml config.toml

# 运行开发服务器 (支持热重载建议使用 cargo-watch: cargo install cargo-watch)
cargo run
# 或者使用 cargo-watch:
# cargo watch -x run
```

### 3. 启动前端 (React)

前端开发服务器默认运行在 `http://localhost:5173`，并会自动代理 API 请求到后端。

```bash
cd frontend

# 安装依赖
npm install

# 启动开发服务器
npm run dev
```

打开浏览器访问 `http://localhost:5173` 即可开始开发。

## 🔌 插件开发

如果你想开发新的插件能力，请参考 [插件开发文档](docs/plugins/plugin-dev.md)：
- [JavaScript 运行时指南](docs/plugins/js_runtime_guide.md) (推荐初学者)
- [WASM 运行时指南](docs/plugins/wasm_runtime_guide.md) (高性能需求)
- [Native 运行时指南](docs/plugins/native_runtime_guide.md) (底层/加密格式)

## 📐 代码规范

### Rust 后端
- 遵循标准的 Rust 格式化规范 (`rustfmt`)。
- 在提交前请运行 `cargo fmt` 和 `cargo clippy` 检查代码。
- 尽量编写单元测试覆盖核心逻辑。

### React 前端
- 使用 TypeScript 编写，尽量避免使用 `any`。
- 组件采用函数式写法 (Functional Components) + Hooks。
- 样式优先使用 Tailwind CSS 类名。
- 提交前请运行 `npm run lint`。

## 📨 提交 Pull Request

1.  **Fork** 本仓库到你的 GitHub 账户。
2.  创建你的功能分支 (`git checkout -b feature/AmazingFeature`)。
3.  提交你的更改 (`git commit -m 'feat: add some amazing feature'`)。
    *   推荐使用 [Conventional Commits](https://www.conventionalcommits.org/) 规范。
4.  推送到分支 (`git push origin feature/AmazingFeature`)。
5.  在 GitHub 上开启一个 **Pull Request**。

## 🐛 提交 Issue

如果你发现了 Bug 或者有功能建议，请通过 GitHub Issue 提交。
- **Bug 报告**：请提供复现步骤、错误日志和环境信息。
- **功能建议**：请描述使用场景和期望的解决方案。

感谢你的每一份贡献！❤️
