# Code Switcher — 功能扩展 Todo

## 功能二：可用模型列表查询

- [x] P2.1 Rust: 添加 reqwest 依赖到 Cargo.toml
- [x] P2.2 Rust: 实现 `fetch_available_models` 命令 (支持 OpenAI 兼容 + Anthropic 格式)
- [x] P2.3 Rust: 在 lib.rs 注册新命令
- [x] P2.4 前端: 新增 ModelSelector 组件 (Fetch Models 按钮 + 下拉列表)
- [x] P2.5 前端: 集成到 ClaudeConfigEditor / CodexConfigEditor / GenericConfigEditor

## 功能三：高级配置编辑 (settings.json / config.toml)

- [x] P3.1 Rust: toml 依赖已在 Cargo.toml 中
- [x] P3.2 Rust: 实现 `read_claude_settings` / `write_claude_settings` 命令
- [x] P3.3 Rust: 实现 `read_codex_config` / `write_codex_config` 命令
- [x] P3.4 Rust: 在 lib.rs 注册新命令
- [x] P3.5 前端: 新增 ClaudeAdvancedPanel 组件 (thinking, performance, model overrides)
- [x] P3.6 前端: 新增 CodexAdvancedPanel 组件 (reasoning, sandbox, web search, features, analytics)
- [x] P3.7 前端: 在 ConfigEditor 中添加 Advanced 标签页

## 本次修复

- [x] ClaudeProfile 添加 `hasCompletedOnboarding: true` 默认值
- [x] Qwen 普通版 brand 改为 codex (支持 Responses API)
- [x] Qwen CP 版 brand 改为 claude (仅支持 Anthropic & OpenAI Completions)
- [x] CodexAdvancedPanel 增加 CSW.md 配置项: Memories, Undo, Analytics, Hide/Show Reasoning
- [x] 移除 ghost 占位文字
- [x] 布局优化: 侧边栏渐变背景、主内容区大气光晕、头部精简

## 验证

- [ ] V.1 Fetch Models: 输入 Deepseek key 返回模型列表
- [ ] V.2 Fetch Models: 输入 DashScope key 返回 Qwen 模型列表
- [ ] V.3 Claude Advanced: 读取/编辑 settings.json
- [ ] V.4 Codex Advanced: 读取/编辑 config.toml
- [x] V.5 `npm run build` 通过
- [ ] V.6 Tauri 应用正常启动

## UI 美化 & 布局优化

- [x] VendorSelector 恢复网格布局 (2-3列, 带图标+描述)
- [x] 去掉品牌过滤,所有 vendor 对所有 brand 可见
- [x] 添加 OpenaiIcon 包装组件
- [x] 添加 VendorInfoCard 组件 (显示选中 vendor 的图标+名称+描述)
- [x] ClaudeConfigEditor 改为 max-w-5xl 左右布局 (lg:grid-cols-2)
- [x] CodexConfigEditor 同样改为左右布局
- [x] Auth 卡片使用 OpenAI logo 作为图标
- [x] auth.json / config.toml 改为可折叠

## 1M 上下文

- [x] ClaudeProfile 添加 `use1MContext: boolean` 字段
- [x] generateConfigJson 中所有模型名自动追加 `[1m]` 后缀
- [x] Options 区域添加 "1M Context" 开关

## UI 交互优化

- [x] VendorSelector 图标放大 (h-10 w-10, size=28)
- [x] ToggleRow 启用时显示品牌 accent 色背景
- [x] ToggleRow 条目放大 (text-[13px], py-1.5)
- [x] ToggleRow hover 气泡提示 (显示功能说明)
- [x] 所有 ToggleRow 调用添加 accent 和 description 参数
- [x] vendorId 配置写入映射确认 (select→fill baseUrl→save→disk, deselect→clear vendorId→save→disk)

## 本次修复：卡片 Save 激活落盘

- [x] GitNexus 分析当前保存/激活链路：
  - 前端底部 `Save` 原先只调用 `activate_provider_profile`。
  - 后端 `activate_provider_profile_with_paths` 要求目标 provider JSON 已存在。
  - 前端失败时会吞掉错误并保留 UI 乐观激活，导致 APP 内看起来激活，但本地文件可能没有写入。
- [x] Rust 新增 `save_and_activate_provider_profile` Tauri 命令：
  - 先把当前卡片完整 provider JSON 保存到 `~/.csw/config/<brand>/<id>.json`。
  - 再激活该 provider，写入 `isActive` 状态。
  - Codex 激活时继续沿用原逻辑写入 runtime `~/.codex/auth.json` 和 `~/.codex/config.toml`。
- [x] 前端底部 `Save` 改为调用 `save_and_activate_provider_profile`：
  - Claude / Codex / Gemini 都走同一条保存并激活链路。
  - 取消失败时的本地乐观激活，避免 UI 和磁盘状态不一致。
- [x] 添加 Rust 回归测试：
  - `save_and_activate_provider_profile_writes_active_json_for_all_brands`
  - 覆盖 Claude、Codex、Gemini 三个 brand 的 provider JSON 写入和 `isActive` 落盘。
- [x] 修复 `cargo fmt --check` 失败：
  - 失败点确认在模型查询函数的格式换行。
  - 已运行 `cargo fmt`，当前 `cargo fmt --check` 通过。
- [x] 修复 Claude activate 没写 runtime 配置：
  - Claude 激活现在写入 `~/.claude/settings.json`。
  - 写入内容来自 provider profile 的 env 映射，和前端预览保持一致。
  - 会保留 `settings.json` 中非 provider 管理的配置，只替换受控 env key。
- [x] 激活失败写入日志：
  - `save_and_activate_provider_profile` 遇到错误会写入 SQLite `~/.csw/data/logs.db`。
  - 当前表为 `app_logs`，字段包含时间戳、level、operation、message。
- [x] 新增 Rust 回归测试：
  - `save_and_activate_claude_provider_writes_settings_json`
  - `save_and_activate_provider_profile_logs_activation_errors`
- [x] 修复切换 provider / 刷新时页面和高亮边框闪动：
  - 根因：`ProfileGrid` 使用 ``key={`grid-${activeBrand}`}``，切换 Claude / Codex / Gemini 时会强制重挂载整块 main 区。
  - 根因：卡片和 sidebar 有初始入场动画，刷新或切换时会重新执行 opacity / 位移动画。
  - 根因：选中卡片边框使用共享 `layoutId="selected-border"`，跨卡片/跨 brand 切换时会触发 Framer Motion 共享布局动画。
  - 已改为稳定 `key="grid"`，切换 brand 时不重挂载 main。
  - 已关闭 grid 卡片和 sidebar 的启动入场动画，保留 hover、active、selected 的正常视觉状态。
  - 已移除 selected card 边框的共享 `layoutId`，避免高亮框跨卡片闪动。
  - 已把 sidebar 点击后的浏览器默认焦点框改成受控 `focus-visible` 样式，避免点击时出现突兀外框。
  - 二次修复：profile card 的 React key 从 `p.id` 改为 `${brand.id}-${p.id}`，避免 Claude/Codex 下同名 provider 复用同一个 DOM 节点，导致颜色沿用上一 brand。
  - 二次修复：Add Profile card key 改为 `${brand.id}` 维度，避免切换 provider 后保留旧 Motion 状态。
  - 二次修复：移除 profile card 点击时的 Framer Motion `opacity` 动画和逐卡片 `delay`，点击卡片只更新 class/style，不再触发延迟淡入淡出。
  - 已用 Node 源码断言做红绿检查，并通过浏览器切换 Claude / Codex 做人工验证。
- [x] 增加 Linux 打包能力：
  - 计划书：`docs/superpowers/plans/2026-04-29-linux-packaging.md`。
  - 已新增 `linux:build`：在 Linux 环境内执行 `tauri build --bundles deb,rpm,appimage`。
  - 已新增 `linux:build:docker`：从 macOS 通过 Docker builder 生成 Linux 测试包。
  - 已新增 Docker builder：`packaging/linux/Dockerfile` 和 `packaging/linux/build-linux.sh`，产物复制到 `target/linux-release/`。
  - 已将根目录 `target/` 加入 `.gitignore`，本地 release 产物保留给测试但不进入版本库。
  - 已新增 GitHub Actions `Linux Build` workflow：手动触发或推送 `v*` tag 时上传 `code-switcher-linux` artifact。
  - 已补充 Tauri `bundle.icon` 配置，解决 AppImage 打包时找不到方形 icon 的问题。
  - 已用 OrbStack Ubuntu amd64 实测打包成功，当前产物：
    - `target/linux-release/code-switcher_0.1.0_amd64.deb`，约 5.6M。
    - `target/linux-release/code-switcher-0.1.0-1.x86_64.rpm`，约 5.6M。
    - `target/linux-release/code-switcher_0.1.0_amd64.AppImage`，约 75M。
  - Linux 构建依赖 Node 20+；Docker、CI、OrbStack 实测环境使用 Node 22。
  - Linux release 继续使用 app-managed local paths：`~/.csw`、`~/.codex`、`~/.claude`、`~/.gemini`。
  - 当前不包含签名、自动更新和发行页自动发布，仍属于测试版 release artifact。
