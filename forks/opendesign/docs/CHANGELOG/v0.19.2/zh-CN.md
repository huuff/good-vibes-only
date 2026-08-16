---
title: Open Design 0.19.2 — DeepSeek Harness, One Command Away
description: 用一条命令安装 DeepSeek Harness，导出可离线使用的 HTML，并让长时间 Agent 运行保持流畅。
---

### 🌟 Codename: *DeepSeek Harness, One Command Away*

🧰 **28 个 PR · 9 位贡献者 · 2 天** — **0.19.2 缩短了从安装 DeepSeek
Harness 到用它完成工作的路径。** 一条命令即可准备兼容的 Harness 工具链；Windows
路径包含空格时，Open Design 的连接组件也能正常安装；长时间流式输出不再拖慢聊天。
本次更新还带来可离线使用的 HTML 导出和结构化 Design System 3.0 runtime。

## 🔥 亮点

- 🧰 **用一条命令安装 DeepSeek Harness。** 新增的 bootstrap 脚本覆盖 macOS、
  Linux、Windows PowerShell 和 Windows CMD。脚本会复用兼容的本地工具，或安装一套
  用户级隔离的 Node.js 24 工具链与固定版本 Harness，然后打开 `dsh web` 配置 API
  key。Open Design 现在会通过 shell-safe 的 profile 路径安装连接组件，即使 Windows
  应用路径或 `DSH_HOME` 含有空格也可以完成安装。 (#6900, #6905)

- 📦 **HTML 导出移动后仍能正常打开。** 下载 HTML 时，Open Design 会把项目内的
  图片、样式、字体、module、worker 和嵌套文档收进一个可离线使用的文件。依赖缺失
  或体积超限时会返回明确错误，不再生成损坏的导出文件。CLI 也可以通过
  `od export --format html` 生成相同结果。 (#6855)

- ⚡ **长时间 Agent 运行保持流畅。** Thinking 与正文仍会持续出现在聊天中，
  Open Design 会在后台合并持久化和 React 更新，并在运行结束时写入最后一批缓存内容。
  `od export` 现在只需 project ID；项目已经保存 workspace 绑定时，Agent 不必再传
  workspace 或 member 参数。 (#6915)

- 🎨 **结构化 Design System 现在会参与生成和校验。** Design System 3.0 package
  为 Agent 提供 intent map、精确的组件实现、token 与规则。Agent 可以先解析目标组件，
  只加载声明过的文件，再检查组件复用、必要状态、token 使用和 fallback 行为。
  `od tools design-systems` 也提供同一套流程。 (#6805)

## ✨ 新增

- DeepSeek Harness 设计指南新增配套插件合集，收录 13 个经过核验的设计插件，按视觉
  输入、生成式 UI、设计工作流和预览分类，并提供本地化详情页与来自上游文档的安装
  命令。 (#6864, #6897)
- 打包应用发现本地 Agent CLI 时，会搜索 nix-darwin、NixOS、系统级和用户级的标准
  Nix profile 目录。 (#6885)

## 🔁 变更

- Home 创建入口会优先展示常用项目类型；DeepSeek Pro 与 Flash 活动文案也在产品和
  多语言落地页中保持一致，活动时间延续至 8 月 27 日。 (#6852, #6863, #6876,
  #6892)
- Landing page 生产部署完成后会清理当前 host 的缓存，并为 HTML 使用较短的 edge
  TTL，减少发布后继续看到旧页面的时间。 (#6552)

## 🐛 修复

### 🧠 DeepSeek Harness 与 Agent

- 取消 Harness run 后，运行只会以 canceled 状态结束一次，profile 进程会退出，
  provider 错误详情会保留，诊断信息也会记录正确的 provider 与 model。取消后的消息
  不会再显示为 Working、Done 或 Run failed。 (#6880)
- 缺少 DeepSeek 凭据时，界面会给出 `dsh web`、Settings → Models 和
  `DEEPSEEK_API_KEY` 的配置步骤，不再显示 `[object Object]`。 (#6890)
- 较长的结构化事件流不会在有效内容已经出现后继续让聊天保持 busy；项目导出也不会
  因为缺少重复的 workspace context 而反复失败。 (#6915)

### 🖼️ 预览与 Workspace UI

- Chromium 中止 `about:srcdoc` 导航后，生成的演示文稿会自动恢复；powered preview
  可以在反向代理后工作；comment bridge 也会等已提交的 preview URL 生效后再报告
  ready。 (#5592, #6797, #6882)
- 移动端评论预览关闭时不再闪烁，deck 预览会填满 viewer，move-to-team dialog 也会
  完整遮住下方 composer。 (#6785, #6829, #6854)
- 展开 Message Center 消息后，会重新显示配置过的 HTTP(S) 操作按钮；不安全的 URL
  仍会隐藏。 (#6894)

### 🖥️ 桌面端与交付

- Sidecar 会接受配置过的内部 host allowlist，打包部署可以在获准的内部 host 后正常
  工作。 (#6802)
- Team 套餐的落地页文案现在与每席位 5 美元的价格目录一致。 (#6834)

## 🙏 感谢每一位参与 0.19.2 的贡献者

@AmyShang-alt · @CVE-Hunter-Leo · @jlbeard84 · @joeylee12629-star ·
@kiuber · @lefarcen · @lorenzozanee · @mrcfps · @Siri-Ray
