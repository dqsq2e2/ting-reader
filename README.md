# Ting Reader

Ting Reader 是一个轻量级的自托管有声书平台，支持自动刮削元数据、多端播放进度同步以及多架构 Docker 部署。

![License](https://img.shields.io/github/license/dqsq2e2/ting-reader)
![Docker Pulls](https://img.shields.io/docker/pulls/dqsq2e2/ting-reader)

## 📸 界面展示

| 桌面端首页 | 移动端首页 |
| :---: | :---: |
| ![Desktop Home](https://image.sjcnas.xyz/i/2026/02/02/12ro2xh.png) | ![Mobile Home](https://image.sjcnas.xyz/i/2026/02/02/12s6bcx.png) |

## ✨ 功能特性

- 📚 **自动刮削**：集成喜马拉雅元数据刮削，自动获取书名、作者、演播者、简介及标签。
- 🎨 **自适应主题**：根据书籍封面**自动提取主色调**并实时调整书籍详情页背景与按钮颜色，视觉体验极致沉浸。
- ☁️ **多源支持**：支持 WebDAV（如 Alist、PikPak）远程存储及本地目录挂载，轻松管理海量有声书资源。
- 🎧 **沉浸播放**：支持跳过片头/片尾，支持播放速度调节及进度记忆。
- 🏷️ **智能标签**：支持标签筛选，标签云横向滚动展示，交互体验佳。
- 🌓 **深色模式**：完美的深色模式适配，夜间听书更护眼。
- 🐳 **Docker 部署**：支持 amd64 和 arm64 多架构构建，一键启动。
- 🔐 **权限管理**：完善的登录系统与管理员后台。

## 🚀 快速开始

### 使用 Docker Compose (推荐)

创建 `docker-compose.yml` 文件：

```yaml
version: '3'
services:
  ting-reader:
    image: dqsq2e2/ting-reader:latest
    container_name: ting-reader
    ports:
      - "3000:3000"
    volumes:
      - ./data:/app/data
      - ./storage:/app/storage
      - ./cache:/app/cache
    restart: always
```

启动容器：

```bash
docker-compose up -d
```

访问 `http://localhost:3000` 即可开始使用。

## 🛠️ 开发指南

### 环境要求
- Node.js 20+
- SQLite3

### 安装步骤

1. 克隆仓库：
   ```bash
   git clone https://github.com/dqsq2e2/ting-reader.git
   cd ting-reader
   ```

2. 安装后端依赖：
   ```bash
   cd ting-reader-backend
   npm install
   npm start
   ```

3. 安装前端依赖：
   ```bash
   cd ../ting-reader-frontend
   npm install
   npm run dev
   ```

## 📄 开源协议

本项目采用 [MIT License](LICENSE) 协议。

## 🙏 致谢

本项目参考或使用了以下优秀开源项目，在此表示衷心的感谢：

- [Abs-Ximalaya](https://github.com/search?q=Abs-Ximalaya&type=repositories): 喜马拉雅刮削与下载参考。
- [xm_decryptor](https://github.com/jupitergao18/xm_decryptor): 喜马拉雅 xm 文件解密核心逻辑参考。

## 🤝 贡献指南

欢迎提交 Issue 或 Pull Request！请参考 [CONTRIBUTING.md](CONTRIBUTING.md) 了解更多细节。
