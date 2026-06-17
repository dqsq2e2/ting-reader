# Ting Reader 外挂组件 (Widget) 集成指南

Ting Reader 提供了强大的外挂播放器组件（Widget），允许您将有声书播放功能无缝集成到个人博客、Notion、Obsidian 或任何支持 HTML/Iframe 的网页中。

## 1. 基础集成

### 获取嵌入代码
前往 Ting Reader 的 **“个性化设置” -> “外挂组件”** 页面，您可以选择两种模式：

*   **私有模式 (免登录)**: URL 中包含您的访问 Token。
    *   *适用场景*: 个人仪表盘、私有 Notion 页面。
    *   *注意*: 请勿在公开网页泄露此代码，否则他人可使用您的账号播放。
*   **公开模式 (需登录)**: URL 不含 Token。
    *   *适用场景*: 公开博客、网站侧边栏。
    *   *行为*: 访客首次访问需输入账号密码登录。

---

## 2. 布局模式 (开箱即用的 HTML 代码)

为了方便集成，我们提供了包裹好的 HTML 代码。您可以直接复制粘贴到您的网站 HTML 中。

### 2.1 标准嵌入 (文档流)
默认方式，Widget 就像一张图片一样插入在文章或页面的指定位置。

```html
<iframe 
  src="http://your-ting-reader.com/widget?token=YOUR_TOKEN" 
  width="100%" 
  height="150" 
  frameborder="0" 
  allow="autoplay; fullscreen">
</iframe>
```

### 2.2 吸底模式 (Fixed Bottom)
像网易云音乐网页版一样，固定在屏幕底部，不随页面滚动。

```html
<div style="position: fixed; bottom: 0; left: 0; width: 100%; z-index: 9999;">
  <iframe 
    src="http://your-ting-reader.com/widget?token=YOUR_TOKEN" 
    width="100%" 
    height="150" 
    frameborder="0" 
    allow="autoplay; fullscreen">
  </iframe>
</div>
```

### 2.3 悬浮右下 (Floating Right)
像聊天窗口一样悬浮在屏幕右下角。当屏幕较窄时，Widget 会自动切换为垂直紧凑布局。

```html
<div style="position: fixed; bottom: 20px; right: 20px; width: 350px; height: 150px; z-index: 9999; border-radius: 16px; overflow: hidden; box-shadow: 0 4px 20px rgba(0,0,0,0.15);">
  <iframe 
    src="http://your-ting-reader.com/widget?token=YOUR_TOKEN" 
    width="100%" 
    height="100%" 
    frameborder="0" 
    allow="autoplay; fullscreen">
  </iframe>
</div>
```

> **注意**: 请将代码中的 `http://your-ting-reader.com` 替换为您的实际部署地址，`YOUR_TOKEN` 替换为您的真实 Token（如果是私有模式）。推荐直接在设置页面复制生成好的代码。

---

## 3. 自定义 CSS 注入 (Widget 内部样式)

如果您想修改 Widget **内部**的样式（例如去掉白色背景、改变字体颜色），请使用 **“个性化设置” -> “自定义 CSS 注入”** 功能。

**重要提示**: 这里的 CSS 只会影响 Widget **内部**，无法控制 Widget 在网页中的位置（位置由上面的 HTML 代码控制）。

### 常用修改示例

#### 3.1 背景完全透明 (配合悬浮模式使用最佳)
如果您希望 Widget 没有背景色，直接浮在您的网页背景上。

```css
/* 移除背景色、边框和阴影 */
.widget-mode, 
.widget-mode > div, 
.widget-mode .bg-white\/95, 
.widget-mode .dark\:bg-slate-900\/95 {
    background: transparent !important;
    box-shadow: none !important;
    border: none !important;
    backdrop-filter: none !important;
}
```

#### 3.2 极简模式
隐藏左上角的“返回列表”按钮，只允许播放当前专辑。

```css
/* 隐藏左上角的返回箭头 */
.widget-mode > button.absolute {
    display: none !important;
}
```

#### 3.3 自定义字体样式
修改标题颜色以匹配您的博客主题。

```css
/* 标题改为品牌色 */
.widget-mode h4 {
    color: #ff4757 !important; 
    font-family: 'Georgia', serif !important;
}
```

#### 3.4 直角风格
去除所有圆角，打造硬朗风格。

```css
.widget-mode * {
    border-radius: 0 !important;
}
```

---

## 4. 常见问题 (FAQ)

**Q: 为什么我自己写的 CSS 定位不起作用？**
A: 请检查您是把 CSS 写在哪里了。
*   控制 **位置** (position/right/bottom) 的 CSS 必须写在您的 **网站 HTML** 中（包裹 iframe 的 div 上）。
*   控制 **颜色/字体** 的 CSS 必须写在 Ting Reader 的 **“自定义 CSS 注入”** 设置里。

**Q: 为什么点击封面不能全屏？**
A: 请检查您的 `iframe` 标签是否包含了 `allow="fullscreen"` 属性。这是浏览器安全策略要求的。

**Q: Widget 在手机上显示不全？**
A: Widget 具有响应式设计。当宽度小于 380px 时，它会自动切换为垂直布局以适应窄屏。请确保 iframe 高度至少为 `150px`。

