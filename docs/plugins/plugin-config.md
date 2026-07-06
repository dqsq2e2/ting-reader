# 插件配置 `config_schema`

插件需要让管理员填写 API 密钥、接口地址、模型名、开关、枚举选项或数值参数时，在 `plugin.yml` 里声明 `config_schema`。Ting Reader 会根据这个 schema：

- 在插件管理页显示“配置”按钮和表单；
- 从 `default` 提取初始配置；
- 保存配置到后端插件配置目录；
- 按 schema 校验配置；
- 自动加密敏感字段；
- 在插件运行时把解密后的配置注入 `Ting.config`。

## 1. 完整写法

推荐使用 JSON Schema object 形式：

```yaml
config_schema:
  type: object
  properties:
    api_base_url:
      type: string
      title:
        zh: API 地址
        en: API endpoint
      description:
        zh: OpenAI 兼容接口地址，可以填写服务根地址或完整 /v1/chat/completions 地址。
        en: OpenAI-compatible endpoint. A service root or full /v1/chat/completions URL is accepted.
      placeholder:
        zh: https://api.example.com/v1/chat/completions
        en: https://api.example.com/v1/chat/completions
      default: https://api.openai.com/v1/chat/completions

    api_key:
      type: string
      format: secret
      x-encrypted: true
      title:
        zh: API 密钥
        en: API key
      description:
        zh: 后端会加密保存，插件运行时通过 Ting.config.api_key 读取明文值。
        en: Stored encrypted by the backend. The plugin reads the plaintext value from Ting.config.api_key at runtime.

    model:
      type: string
      title:
        zh: 模型
        en: Model
      default: gpt-4.1-mini

    mode:
      type: string
      title:
        zh: 处理模式
        en: Processing mode
      default: balanced
      enum:
        - fast
        - balanced
        - accurate
      enum_labels:
        fast:
          zh: 快速
          en: Fast
        balanced:
          zh: 平衡
          en: Balanced
        accurate:
          zh: 准确
          en: Accurate

    temperature:
      type: number
      title:
        zh: 温度
        en: Temperature
      default: 0.2
      minimum: 0
      maximum: 1

    max_candidates:
      type: integer
      title:
        zh: 最大候选数
        en: Max candidates
      default: 8
      minimum: 1
      maximum: 20

    enabled:
      type: boolean
      title:
        zh: 启用增强处理
        en: Enable enhanced processing
      default: true
```

## 2. 扁平简写

后端也接受扁平写法，会自动包装成 `type: object` + `properties`。适合简单插件：

```yaml
config_schema:
  api_key:
    type: string
    format: secret
    x-encrypted: true
    title: API Key
  source_url:
    type: string
    title: Source URL
    default: https://example.com/plugins.json
```

## 3. 支持的表单字段

插件管理页当前按以下规则渲染配置表单：

| schema | 后台表单 |
| --- | --- |
| `type: string` | 文本输入框 |
| `type: string` + `enum` | 下拉选择框 |
| `type: number` | 数字输入框 |
| `type: integer` | 数字输入框，保存为数字 |
| `type: boolean` | 复选框 |
| `format: password` / `format: secret` | 密码输入框，并按敏感字段处理 |
| `x-encrypted: true` / `encrypted: true` | 密码输入框，并加密保存 |

`title`、`description`、`placeholder` 可以写字符串，也可以写中英文对象。`enum_labels` 可以按枚举值提供多语言显示名。

```yaml
title:
  zh: API 地址
  en: API endpoint
description:
  zh: 用于访问远程服务。
  en: Used to access the remote service.
placeholder:
  zh: 请输入地址
  en: Enter endpoint
enum_labels:
  clean:
    zh: 使用内置清洗
    en: Use built-in cleanup
```

支持 i18n 的常见字段包括顶层 `description`、搜索字段 `label/placeholder`、结果字段 `label`、配置项 `title/description/placeholder`、UI capability 的 `title/label`。

## 4. 敏感字段和加密

敏感字段建议同时写 `format: secret` 和 `x-encrypted: true`：

```yaml
api_key:
  type: string
  format: secret
  x-encrypted: true
  title:
    zh: API 密钥
    en: API key
```

后端识别以下任一标记后都会加密保存：

- `x-encrypted: true`
- `encrypted: true`
- `format: password`
- `format: secret`

插件管理页读取配置时，敏感字段会显示为空或占位提示，不会把明文回显给浏览器。用户保存时如果保持密钥不变，前端会用内部占位符保留旧值；插件作者不需要处理这个占位符。

## 5. 运行时读取配置

JavaScript 运行时通过 `Ting.config` 读取配置：

```js
function readConfig() {
  const config = Ting.config || {};
  return {
    apiBaseUrl: String(config.api_base_url || "https://api.openai.com/v1/chat/completions"),
    apiKey: String(config.api_key || ""),
    model: String(config.model || "gpt-4.1-mini"),
    enabled: config.enabled !== false,
    maxCandidates: Number(config.max_candidates || 8),
  };
}

async function search(args) {
  const config = readConfig();
  if (!config.apiKey) {
    Ting.log?.warn?.("api_key is empty; returning fallback result.");
  }
}

globalThis.search = search;
```

WASM 和 Native 运行时由宿主在调用时注入对应运行时上下文。需要配置时，优先通过运行时提供的插件配置上下文读取；不要直接读取后端配置文件，也不要假设配置文件路径。

## 6. 保存和更新配置

插件管理页使用以下接口保存配置：

```http
GET /api/v1/plugins/:id/config
PUT /api/v1/plugins/:id/config
Content-Type: application/json

{
  "config": {
    "api_key": "sk-...",
    "model": "gpt-4.1-mini",
    "enabled": true
  }
}
```

保存后后端会校验 schema、加密敏感字段并通知插件管理器。已运行的插件需要重新加载或由宿主热更新后才能拿到新配置；配置类问题排查时优先执行“保存配置”后再“重新加载插件”。

## 7. 常见坑

- `config_schema` 只负责插件自身配置，不是存储库的刮削源配置；存储库配置仍写在 library 的 `scraper_config`。
- `default` 只用于初始化和补齐缺失字段，不会覆盖用户已经保存过的值。
- 加密字段不要写在普通 `description` 或日志里；插件日志也不要打印 API key。
- 当前插件管理页不渲染嵌套 object、array 和复杂表单；需要复杂配置时建议拆成多个简单字段，或提供 `ui_extension` 自定义配置面板。
- 修改 `plugin.yml` 里的 `config_schema` 后，需要重新打包/重装或重新加载插件，后台才会看到新的表单。
