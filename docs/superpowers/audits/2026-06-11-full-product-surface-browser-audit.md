# 2026-06-11 FeroHa 全量产品面与浏览器模拟审查

## 结论摘要

FeroHa 的项目本质是一个 Dual-Track AI Note IDE：人类面负责最终写入、审查和知识整理，AI 面负责检索、分析、编排、生成提案和维护记忆结构。核心安全边界是：AI 不能直接越过人类面写入笔记，AI 输出应先进入 Bridge Review，形成具体文本变更后再进入 Diff Review。

本轮已完成：

- 前端 Vite 以 `http://127.0.0.1:1420/` 启动并通过浏览器脚本访问。
- 浏览器预览模式下，AI 面和人类面的主要页面、按钮、表单、canvas 和浮窗均已模拟打开或操作。
- Rust/Tauri 命令层通过 `cargo test` 覆盖验证。
- React/Vitest 单元测试和生产构建通过。
- 已创建 `.env` 占位文件，便于后续填写 API key。

本轮未完成真实外部 AI 调用：当前代码没有读取 `.env`，AI key 主要通过设置页/Tauri 配置保存。填入 `.env` 后，还需要把这些值同步到设置页或接线 dotenv 读取，才能做 live AI 调用。

## 用户行为与功能清单

### 全局壳层

- 在 AI 面和人类面之间切换。
- 展开/收起侧栏。
- 使用状态栏查看文件、行列、保存状态和当前控制面。
- 打开 CLI 浮窗，输入 `/agent ...`，支持关闭、最小化、Pin、历史导航。
- 使用快捷键进入编辑器、图谱/画布、任务/Diff、CLI、搜索等入口。

### 人类面

- 编辑器：编辑 Markdown、保存、预览、格式化、使用加粗/斜体/标题/链接/列表/代码/引用/删除线等工具栏动作。
- 大纲与元数据：切换大纲、查看笔记元数据。
- 向 AI 提任务：填写任务标题、任务类型、范围、期望输出和审查方式；提交给 AI Manager；在浏览器预览下生成 payload。
- 任务类型：研究、总结、验证、Dream、JSON-LD 索引、JSON-LD 读取、写作提案、外部导入、代码辅助。
- 任务范围：当前笔记、选中文本、文件夹、图谱焦点、自由上下文。
- 审查方式：人工 Bridge 审查、只读自动入队、仅生成草稿。
- 灵感画布：添加笔记、连接模式、删除模式、导出 Markdown、适配全部卡片；canvas 支持卡片和连接。
- Bridge Review：查看 AI 提案、刷新提案、审批/拒绝/归档/进入差异审查；浏览器预览下显示本地运行时不可用。
- Diff Review：查看待处理和历史、统一/并排视图、接受/拒绝 ghost/text block 变更；浏览器预览下为空状态。
- 设置：语言、主题、字号、字体、默认视图、自动保存、快捷键、LLM 提供商、API key、模型、Embedding 提供商。

### AI 面

- 编辑器：同样可编辑当前笔记，并通过底部 AI task strip 发起 AI 任务。
- 知识图谱：显示 Dream 三区图谱、搜索节点、聚焦视图、Dream 三区视图、导出 PNG；浏览器预览示例为 5 节点/4 连线。
- Agent 任务：刷新、清除、触发 Dream；显示 Manager/Scientist/Orchestrator 三主体、信任评分、数据流、任务队列、Dream 状态。
- 指令卡库：浏览 25 个指令卡，按类别/标签/排序筛选，创建、导入、导出、切换视图、收藏/使用卡片。
- 流程编辑器：运行、保存、导出、导入、撤销/重做；从内容/分析/格式/系统/Agent 指令卡构建流程；默认开始/结束 2 节点。
- 插件设置：刷新插件状态，查看插件后端、已发现/已启用插件数、插件目录和 `plugin_status` 命令。
- AI task strip：打开指令卡，选择任务类型，查看 Bridge 风险、写入策略和推荐工具。
- Orchestrator compact panel：显示编排中枢状态，等待 Tauri 运行时数据。

### 后端与 AI 能力面

- Vault/FS：打开 vault、读取/保存/创建/删除/重命名笔记、创建文件夹、模板、标签、资产和 watcher。
- Graph：获取图谱、反链、笔记链接、焦点图谱。
- AI commands：全文/向量搜索、提交任务、派发 Agent task、CLI 执行、任务审批/取消、trace、Dream、trust score、Manager snapshot、scheduler status、配置读写、ghost feedback、orchestrator 状态/事件/终止/恢复、workflow patch、output review、research translate、proposition graph verify、JSON-LD/MDT 相关能力。
- Bridge：提案列表、状态统计、审批/拒绝/归档等动作。
- Diff/Ghost：读取 diff blocks、ghost suggestion、接受/拒绝/审查历史。
- Snapshot：当前快照和快照差异。

## 浏览器模拟结果

| 区域 | 操作 | 结果 |
| --- | --- | --- |
| App shell | 打开 `http://127.0.0.1:1420/` | 成功，标题为 `Dual-Track Note IDE` |
| 编辑器 | 输入临时文本、切换预览/编辑、点击保存 | 编辑器可交互；浏览器预览无 Tauri 持久保存 |
| AI 面导航 | 编辑器、知识图谱、Agent 任务、指令卡、流程、插件、设置 | 均可打开 |
| 知识图谱 | 检查 canvas 与图谱文本 | canvas 存在，1228x617，网格采样非空；显示 5 节点/4 连线 |
| Agent 任务 | 打开面板 | 正确显示 Tauri 后端不可用、三主体和空任务状态 |
| 指令卡 | 打开库与底部指令卡弹层 | 25 张卡可见；弹层可通过 Escape 关闭 |
| 流程 | 打开流程编辑器 | 默认开始/结束节点、运行/保存/导入/导出控件可见 |
| 插件 | 打开插件设置 | 降级到 browser 后端，但暴露了原始 invoke 错误 |
| 人类任务入口 | 填写标题和期望输出，提交 | 显示“浏览器预览：任务 payload 已生成。” |
| 灵感画布 | 打开画布并检查 canvas | canvas 存在，1256x628，网格采样非空 |
| Bridge Review | 打开审查页 | 显示 0 待审查/0 已处理/0% 信任与本地运行时提示 |
| Diff Review | 打开差异页 | 显示浏览器预览无法读取真实差异和空状态 |
| 设置 | 打开人类面/AI 面设置 | 配置控件均可见，快捷键会随控制面变化 |
| CLI 浮窗 | 打开和关闭 | 浮窗出现，包含 `/agent ...` 输入和历史/快捷键提示 |

## 自动化验证结果

- `npm.cmd test`：通过，27 个测试文件，127 个测试。
- `npm.cmd run build`：通过，TypeScript 和 Vite build 成功。
- `cargo test`：通过，Rust 测试结果为 313 passed、0 failed。
- `curl.exe http://127.0.0.1:1420/`：HTTP 200，Vite 页面可访问。
- `npm.cmd run e2e`：未通过，原因是本机 Playwright cache 缺少 `chromium_headless_shell-1217/.../chrome-headless-shell.exe`。

## 发现的问题与风险

1. 自带 E2E 当前不可执行：Playwright headless shell 缓存损坏或不完整，`npx playwright install chromium` 没有恢复缺失的 `chrome-headless-shell.exe`。
2. E2E 选择器过时：`e2e/app.spec.ts` 仍查找 `button[title="Graph"]` 和 `button[title="Diff"]`，当前 UI 使用中文 title，例如 `知识图谱`、`差异审查`。
3. `.env` 未被应用读取：AI 配置目前通过设置页和 Tauri AppConfig 流转，不能只靠填写 `.env` 完成 live AI 测试。
4. 浏览器预览不是完整 Tauri IPC 环境：真实文件写入、vault、Bridge、Diff、AI live call 需要 Tauri webview 运行时。
5. 插件设置的浏览器降级文案泄露原始错误：`Cannot read properties of undefined (reading 'invoke')`，建议改为稳定的人类可读提示。
6. 控制台警告：`FeroHaIcon: icon "Edit" not found in lucide-react`，出现两次。
7. 设置页 API key 字段触发浏览器 verbose 警告：password field 不在 form 内，低风险。
8. DiffView 的主区域 `textContent` 包含 `@keyframes` CSS 文本，可能污染测试、搜索或辅助技术读取。
9. Build 警告：主 chunk 约 1.14 MB，超过 700 kB 警戒线。
10. Rust 测试有 dead_code/profile warning，功能不阻断，但需要在发布前整理。

## 后续 live AI 复测前置条件

1. 修复或重装 Playwright headless-shell 缓存，或把 Playwright 配置指向已存在的 `chromium-1217/chrome-win64/chrome.exe`。
2. 更新 E2E 选择器，使其匹配当前中文 UI 或使用稳定 `aria-label`/test id。
3. 决定 `.env` 策略：要么实现 dotenv -> AppConfig 接线，要么明确要求在设置页填写 API key。
4. 用 Tauri dev/build 启动真实 webview 后，对 vault/Bridge/Diff/AI IPC 做 live 测试。
5. 填入 API key 后，按“人类任务入口 -> AI Manager -> Bridge -> Diff -> 写入”的闭环复测权限边界。

## 2026-06-12 补充复测

用户已在 `.env` 填入 API。复测时只记录脱敏状态，不输出 API key 原文。

### 资源压力处理

- 清理 `node_modules/.vite`。
- 清理 8 个 Playwright 失败产物目录。
- 保留 `test-results/.last-run.json` 这类已跟踪文件。
- 避免再次并行跑 `npm test`、`npm run build`、`cargo test`，改为单 Vite + Codex 内部浏览器复测。

### `.env` 与 AI 配置

- `.env` 中 LLM key 已存在，长度为 35。
- `.env` 中 model 为 `deepseek-v4-flash`。
- `.env` 中 provider 字段疑似填入 endpoint/URL，而不是应用期望的枚举值；UI 测试中按模型名推断并选择 `deepseek`。
- 当前应用仍不会自动读取 `.env`；复测时通过设置页 UI 填入 provider/model/key。
- 测试结束后已通过设置页把浏览器 UI 中的 key 清空，确认 keyLength 为 0。

### Codex 内部浏览器结果

| 区域 | 操作 | 结果 |
| --- | --- | --- |
| 设置 | 从 `.env` 填入 provider/model/key | UI 显示 provider=`deepseek`，model=`deepseek-v4-flash`，key 仅记录长度 |
| 编辑器 | 预览/编辑往返 | 成功 |
| AI task strip | 切换到“验证”任务 | 成功，风险文案更新为低风险一致性验证 |
| 知识图谱 | 打开、搜索 `Dream`、切聚焦 | 成功，显示 5 节点/4 连线，canvas 存在 |
| Agent 面板 | 刷新、触发 Dream | 成功降级，显示 Tauri 后端不可用和空任务状态 |
| 指令卡 | 搜索“研究” | 成功，筛到 2 张卡 |
| 流程 | 重命名并运行 | 成功，toast 显示流程已完成 |
| 插件 | 刷新插件状态 | 成功降级，但仍显示原始 invoke 错误文案 |
| 人类任务入口 | 填写任务并提交 | 成功，生成浏览器预览 payload |
| 灵感画布 | 添加笔记、连接模式 | 成功，canvas 存在 |
| Bridge Review | 打开空状态 | 成功，显示本地运行时提示 |
| Diff Review | 待处理/历史往返 | 成功，浏览器预览空状态正常 |
| CLI 浮窗 | 打开并输入 `/agent verify current note` | 输入成功；最小化按钮在 Codex 内部浏览器中出现坐标翻译失败 |

### Live API 探针

- 使用 `.env` 中 endpoint/key/model 发起最小 DeepSeek chat completions 请求。
- API endpoint 和模型可连通，返回 `status=ok`，`modelReturned=deepseek-v4-flash`。
- 第二次探针返回 `finishReason=length`、`promptTokens=20`、`completionTokens=32`、`reasoningLength=114`、`contentLength=0`。
- 风险：当前应用若只读取 `message.content`，该模型/参数组合可能得到空输出；需要提高 `max_tokens`、处理 `reasoning_content`，或使用非 reasoning/更合适模型。

### 本轮阻塞与风险

- Codex 内部浏览器对 CLI 浮窗按钮的坐标翻译不稳定，点击“最小化/关闭”可能失败；功能本身已验证输入出现并可填写。
- Browser preview 仍不是 Tauri IPC 环境，真实 vault、Bridge、Diff、AI 写入闭环需要 Tauri webview。
- `npm.cmd test` 在资源压力下触发 Node worker OOM：`Zone Allocation failed - process out of memory`。本轮未继续重跑全量单元测试，避免压垮机器。

## 2026-06-12 修复后全量回归

用户更新目标后，保留 `http://127.0.0.1:1420/` dev server 运行，不再清掉 dev server。

### 已修复问题

- `.env` 接入 Tauri 初始配置：`AppConfig::from_env()` 会读取项目 `.env` 和进程环境变量，并把 `FEROHA_LLM_PROVIDER`、`FEROHA_LLM_API_KEY`、`FEROHA_LLM_MODEL`、`FEROHA_EMBEDDING_PROVIDER`、`FEROHA_EMBEDDING_API_KEY`、`FEROHA_OLLAMA_BASE_URL` 写入初始 `AppConfig`。当 provider 填的是 `https://api.deepseek.com` 这类 endpoint，且 model 包含 `deepseek` 时，会归一化为 `deepseek`。
- Tauri 启动时不再只用默认 AI 配置：`main.rs` 使用 env 配置初始化 `LlmRouter`、`EmbeddingPipeline` 和被管理的 `AppConfig`。
- `set_config` 统一复用 `AppConfig::to_router_config()`，避免运行期配置和启动期配置分叉。
- DeepSeek/OpenAI 兼容响应处理补充 `reasoning_content` 与 `finish_reason`：优先返回最终 `message.content`；若只有 reasoning、没有最终输出，则返回明确错误，提示增加 `max_tokens` 或更换模型，而不是静默返回空字符串。
- 插件设置页浏览器预览降级文案已改为稳定中文提示，不再暴露原始 Tauri invoke 错误。
- `FeroHaIcon` 增加 `Edit -> Pencil` 别名，切换预览/编辑时不再产生 `Edit` 图标缺失警告。
- Vitest 增加全局测试 setup，在每个用例后执行 React Testing Library `cleanup()`，修复单 worker 全量测试时 DOM 泄漏导致的重复元素失败。

### Codex 内部浏览器回归

| 区域 | 操作 | 结果 |
| --- | --- | --- |
| App shell | 打开 `http://127.0.0.1:1420/` | HTTP 200，标题为 `Dual-Track Note IDE` |
| 插件页 | 切到“插件”并读取页面状态 | 显示“插件运行状态”“浏览器预览”“插件后端只在 Tauri 应用中可用”，未泄漏 `not in tauri`/`invoke` 原始错误 |
| 编辑器 | 切回“编辑器”，点击“预览模式” | `编辑模式` 按钮出现，内容仍显示欢迎笔记 |
| 控制台 | 检查 warn/warning 日志 | `FeroHaIcon`/`icon "Edit"` 警告数量为 0 |

### `.env` live AI 复测

- 使用 `.env` 中 endpoint/key/model 发起 DeepSeek chat completions 请求；输出只记录脱敏结果，不输出 API key。
- endpoint host：`api.deepseek.com`。
- requested model：`deepseek-v4-flash`。
- returned model：`deepseek-v4-flash`。
- finishReason：`stop`。
- promptTokens：21。
- completionTokens：38。
- contentLength：36。
- reasoningLength：93。
- contentPreview：`{"ok":true,"source":"env-live-test"}`。
- 结论：`.env` 中的 API、endpoint、model 可真实连通；当 `max_tokens=256` 时，该模型返回最终 `message.content`，应用侧新增的 reasoning-only 错误分支仍覆盖低 token 场景。

### 自动化验证

- `cargo test --lib -- --test-threads=1`：通过，323 passed、0 failed。覆盖 Tauri/Rust 后端命令层、AI scheduler、Bridge、Diff/Ghost、Vault/FS、Graph、JSON-LD、MDT、插件、snapshot、orchestrator 等核心能力。
- `cargo build --bin feroha`：通过，Tauri 二进制入口、`main.rs`、context 和 invoke handler 可编译；仍有既有 dead_code/profile warning。
- `npx.cmd vitest run src/components/__tests__/VaultBrowser.test.tsx --pool=threads --poolOptions.threads.singleThread=true`：通过，7 passed。
- `npx.cmd vitest run --pool=threads --poolOptions.threads.singleThread=true`：通过，27 test files、128 tests。
- `npm.cmd run build`：通过，TypeScript 和 Vite build 成功；仍有既有 chunk size warning，最大 `index` chunk 约 1.14 MB。

### 仍需真实 Tauri webview 覆盖的边界

- Codex 内部浏览器可以完整模拟 Web UI，但不能提供 Tauri webview 中的 `__TAURI_INTERNALS__`/IPC 环境。因此真实 `invoke`、文件系统写入、Bridge/Diff 写入闭环和窗口级行为，需要在 Tauri dev/build webview 中补一轮人工或专门 harness 验证。
- 本轮通过 Rust 全量测试覆盖后端命令逻辑，通过 Codex 内部浏览器覆盖前端可见交互，通过 `.env` live 请求覆盖外部 AI 连通性；三者合起来是当前环境下最接近“前后端全启动”的验证组合。
## 2026-06-12 资源压力下继续回归

用户提示存在资源压力后，已清理可再生成测试缓存：

- 删除 `node_modules/.vite`。
- 删除 `test-results`。
- 未删除源码、依赖目录、`.env` 或用户数据。

后台 Vite dev server 在当前工具环境中无法稳定常驻监听 `1420`。为继续使用 Codex 内部浏览器测试，本轮采用“fresh build + 轻量 no-store 静态预览服务 + Codex in-app browser 新标签”的方式复测；该服务只读 `dist`，测试结束可随时关闭，避免遗留重型 Node/Vite 进程。

### 新发现并修复

- CLI 浮窗在浏览器预览中执行 `/agent ...` 时，曾暴露底层 `TypeError: Cannot read properties of undefined (reading 'invoke')`。
- 根因：`CliMiniWindow` 未接收 `isTauri`，始终调用 Tauri `execute_cli`；而底部 `CliBar` 已有浏览器降级分支。
- 修复：`CliMiniWindow` 新增 `isTauri` prop，`App` 传入现有运行时状态；非 Tauri 时直接输出“浏览器预览 / CLI 命令已模拟执行”，不再 import/call Tauri API。
- 新增回归测试：`src/components/__tests__/CliMiniWindow.test.tsx`，覆盖非 Tauri 模式不会调用 `invoke`、不会展示 raw TypeError。

### Codex 内部浏览器复测

| 区域 | 操作 | 最新结果 |
| --- | --- | --- |
| 初始 AI 面 | 新标签打开 `http://127.0.0.1:1420/?codexCacheBust=...` | 标题 `Dual-Track Note IDE`；模式按钮 title 为“切换到人类面”；按钮中心命中 SVG/button，不再落到 aside |
| 编辑器 | 预览模式 -> 编辑模式 | 成功往返，无 `FeroHaIcon/Edit` 警告 |
| AI 图谱 | 打开知识图谱 | 显示 Dream 三区，canvas 存在，5 节点/4 连线 |
| Agent 面板 | 打开 Agent 任务 | 浏览器预览下显示 Tauri 后端不可用的可读降级信息 |
| 指令卡 | 打开指令卡库 | 25 个指令卡可见，搜索/分类/排序入口存在 |
| 流程 | 打开流程编辑器 | 开始/结束节点、运行/保存/导入/导出等入口可见 |
| 插件 | 打开插件页 | 显示“插件后端只在 Tauri 应用中可用”，不再泄露 raw invoke 错误 |
| 人类任务 | 填写任务标题和期望输出，提交给 AI Manager | 显示“浏览器预览：任务 payload 已生成” |
| 灵感画布 | 添加笔记、连接模式、适配全部卡片 | canvas 存在；连接模式显示“从节点锚点拖到目标卡片”提示 |
| Bridge Review | 打开桥接审查 | 显示待审查/已处理/平均信任统计，以及本地运行时提示 |
| Diff Review | 打开差异审查 | 待处理/历史/统一视图/并排视图入口可见，浏览器预览空状态正常 |
| CLI 浮窗 | 输入 `/agent 浏览器回归检查` 并回车 | 显示“浏览器预览 / CLI 命令已模拟执行”；未出现 `Execution error`、`Cannot read properties...` 或 `reading 'invoke'` |
| 笔记库 | 打开侧栏笔记库 | 切换笔记库、新建笔记、新建文件夹、刷新、排序等入口可见 |

### 最新自动化验证

- `npx.cmd vitest run src/components/__tests__/CliMiniWindow.test.tsx --pool=threads --poolOptions.threads.singleThread=true`：通过，1 test。
- `npm.cmd run build`：通过，TypeScript 与 Vite build 成功；仍有既有 chunk size warning，最大 index chunk 约 1.14 MB。
- `npx.cmd vitest run --pool=threads --poolOptions.threads.singleThread=true`：通过，28 test files，130 tests。
- `.env` 脱敏扫描：keyLength=35；`src`、`src-tauri`、`docs` 中未发现原始 API key。

### 当前未完成边界

- Codex 内部浏览器仍不是 Tauri webview；真实 IPC、文件写入、Bridge -> Diff -> 落盘闭环还需要 Tauri webview 专项回归。
- 旧报告中关于“插件 raw invoke 错误”“CLI raw invoke 错误”“`.env` 不读取”的早期记录已经被后续修复覆盖；以本节和“修复后全量回归”为最新状态。

## 2026-06-13 真实 Tauri WebView 回归

本轮在前后端全启动状态下，用真实 Tauri 原生窗口和 WebView2 远程调试端口 `9222` 复测。Codex 内部浏览器继续覆盖普通 Web UI；真实 IPC 由 WebView2 CDP 覆盖。

### 真实 Tauri 启动修复

- 首次启动真实 Tauri 时崩溃在 `tauri-plugin-fs` 配置解析：`plugins.fs.scope` 是旧式配置，当前插件只接受 `requireLiteralLeadingDot`。已移除 `tauri.conf.json` 中旧式 `plugins.shell/fs/dialog` 配置，保留 Rust 侧插件注册。
- 修复后真实 Tauri 可启动，WebView2 调试目标为 `http://localhost:1420/`，页面内存在 `window.__TAURI_INTERNALS__` 和 Tauri global。
- 真实 IPC 快检通过：`ping=pong`，`get_config` 返回 `.env` 注入的 `deepseek / deepseek-v4-flash`，API key 只记录存在和长度，不记录原文。

### 真实 WebView 前端修复

- 真实 WebView 首次渲染出现空白根节点；控制台报 `Rendered more hooks than during the previous render`，根因是 `OrchestratorPanel` 在 `orchestratorStatus=null` 时早返回，状态到达后多执行一个 `useCallback`。
- 已新增回归测试 `keeps hook order stable when orchestrator status arrives after the empty state`。
- 修复后真实 WebView 显示“后端已连接：pong”，React 根节点正常，控制台无 Hook 顺序错误。

### `open_vault` 崩溃修复

- 真实 IPC 调用 `open_vault` 时崩溃：`there is no reactor running`，根因是 Tauri WebView 回调线程上使用裸 `tokio::spawn`。
- 已将文件 watcher worker、AI task worker、Scientist refine 后台任务、定时任务 scheduler 的 spawn 入口改为 `tauri::async_runtime::spawn`。
- 新增 Rust 回归测试 `start_uses_tauri_runtime_without_requiring_a_current_tokio_reactor`。
- 复测打开临时 vault 成功：`get_vault_path` 返回临时 vault，`list_notes` 读到 `seed.md`，`list_bridge_proposals` 正常返回空列表。

### 真实交互结果

| 区域 | 操作 | 结果 |
| --- | --- | --- |
| Tauri IPC | `ping`、`get_config`、`plugin_status`、`list_tasks` | 均可调用；`.env` key 脱敏确认已进入后端配置 |
| 人类任务入口 | 填写任务并提交默认人工 Bridge 审查 | UI 显示“任务已提交到 AI Manager”；后端生成 `Pending` 任务 |
| Bridge Review | 打开收件箱、查看 proposal、点击“拒绝” | proposal 从 `pending` 变为 `rejected`，对应任务变为 `Cancelled` |
| AI live | IPC 调用 `plan_research` | 约 3.2 秒返回 1 条研究步骤，证明真实 Tauri -> Rust -> LLM router -> `.env` 模型通路可用 |
| AI 面 CLI | 打开 CLI 浮窗，执行 `/agent research CLI 真实 IPC 冒烟任务` | 浮窗显示 `Task submitted`，后端生成待审任务和 Bridge proposal；随后通过 Bridge action 拒绝并取消任务 |
| 知识图谱 | 切回 AI 面并打开图谱 | 图谱面板正常渲染 Dream 三区示例，不再空白 |

### 仍需跟进

- 冷启动未打开 vault 时，人类任务入口仍允许提交 `requires_bridge=true` 的任务，但 Bridge store 未初始化，proposal 会被跳过；打开 vault 后正常。建议后续在 UI 上禁用 Bridge 提交流程或提示先打开 vault，或为冷启动提供明确的默认 Bridge store。
- 本轮未执行“批准 Bridge 后实际跑长任务并落盘 Diff”的高成本路径；为避免真实 AI/文件写入副作用，只验证到提交、生成 proposal、拒绝取消。

### 最终验证与资源清理

- `npx.cmd vitest run src/components/__tests__/OrchestratorPanel.test.tsx src/components/__tests__/CliMiniWindow.test.tsx src/components/__tests__/PluginSettings.test.tsx src/components/__tests__/FeroHaIcon.test.tsx src/components/__tests__/AppLayout.test.ts --pool=threads --poolOptions.threads.singleThread=true`：通过，5 个测试文件、21 个测试。
- `cargo test -p feroha --lib -- --test-threads=1`：通过，324 个 Rust 单元测试。
- `npm.cmd run build`：通过，TypeScript 与 Vite production build 成功；仍有既有主 chunk 体积提示。
- `cargo build -p feroha`：通过，Tauri/Rust 二进制入口可编译；仍有既有 dead_code/profile warning。
- `.env` 密钥脱敏扫描：`src`、`src-tauri`、`docs` 中未发现原始 API key。
- 资源清理：已删除本轮测试产生的 `.codex-e2e-vault`、`.codex-tauri-dev.pid`、`tauri-dev.out.log`、`tauri-dev.err.log` 与 `node_modules/.vite` 可再生缓存。
- 端口清理：`1420` 与 `9222` 无测试监听残留。

## 2026-06-13 Bridge 冷启动与审查模式补充回归

上一轮真实 WebView 发现：冷启动未打开 vault 时，人类任务入口可以提交 `manual_bridge` 任务，但后端 `BridgeProposalStore` 尚未初始化，导致任务进入 scheduler 后没有对应 proposal。此状态会让用户看到一个 pending 任务，却无法在 Bridge Inbox 审查。

### 本轮修复

- 后端新增 Bridge 预检：所有会走 Bridge pending 的入口在写入 scheduler 前检查 Bridge store；无 vault 时返回明确错误，而不是静默创建孤儿 pending task。
- 人类任务入口的 `review_mode` 现在参与后端策略：
  - `manual_bridge`：保持原有 Bridge 审查要求。
  - `read_only_auto_queue`：当策略没有写入根时降级为 `requires_bridge=false`。
  - `draft_only`：清空写入根、移除写入型工具，只保留安全草稿能力，降级为 `requires_bridge=false`。
- 前端 `HumanTaskIntake` 在真实 Tauri 且未打开 vault 时，会对默认人工 Bridge 审查显示“先打开笔记库”提示，并禁用提交按钮；用户可改用只读自动入队或仅生成草稿。

### 真实 WebView/IPC 补充测试

- Codex 内置浏览器打开 `http://localhost:1420/`：普通 Web UI 可见，浏览器预览模式正常。
- WebView2 CDP 连接真实 Tauri 页面：`window.__TAURI_INTERNALS__` 存在，标题为 `Dual-Track Note IDE`。
- 无 vault 状态：
  - `get_vault_path` 返回 `No vault open`。
  - `manual_bridge` 的 `dispatch_agent_task` 返回错误：需要先打开 vault。
  - `list_tasks` 未出现人工 Bridge 孤儿 pending task。
  - `read_only_auto_queue` 任务成功进入执行队列，策略为 `requires_bridge=false`、`write_roots=[]`。
  - `draft_only` 写作任务成功进入执行队列，策略为 `requires_bridge=false`、`write_roots=[]`、工具仅保留 `llm_complete`。
- 真实 UI 操作：切换到“人类面” -> “向 AI 提任务”，填写标题与期望输出后，页面显示“先打开笔记库后才能提交 Bridge 审查任务”，提交按钮仍禁用。
- 打开临时 vault 后：
  - `open_vault` 成功，`list_notes` 读到 `seed.md`。
  - `manual_bridge` 任务成功生成 pending Bridge proposal。
  - 执行 `reject` 后，proposal 变为 `rejected`，任务变为 `Cancelled`。

### 补充验证

- `npx.cmd vitest run src/components/__tests__/HumanTaskIntake.test.tsx src/components/__tests__/CliMiniWindow.test.tsx src/components/__tests__/OrchestratorPanel.test.tsx --pool=threads --poolOptions.threads.singleThread=true`：通过，3 个测试文件、11 个测试。
- `cargo test -p feroha task_intent_command_tests -- --nocapture`：通过，14 个目标测试。
- `cargo test -p feroha --lib -- --test-threads=1`：通过，326 个 Rust 单元测试。
- `npm.cmd run build`：通过，TypeScript 与 Vite production build 成功；仍有既有主 chunk 体积提示。
## 2026-06-13 Bridge 批准、AI 生成、Diff 接受闭环补充

本轮继续保留完整 Tauri dev 进程，并通过 WebView2 CDP 连接真实 Tauri 页面执行 IPC 自动化；普通 Codex 内浏览器仍用于页面可见交互检查。`.env` 中的 API key 只用于真实 AI 调用，报告与日志只记录脱敏结果。

### 新发现并修复

- `accept_diff` 在真实 Tauri 中触发 `process_file_event` 后曾导致 AppState 锁中毒，日志显示 panic：`Cannot block the current thread from within a runtime`。根因是 `SearchEngine` 在 Tokio runtime 内调用 `blocking_lock()`。已将 Tantivy `IndexWriter` 锁改为同步 `std::sync::Mutex`，并把锁中毒转换为普通错误。
- Diff 接受后，ghost 本身会变为 `accepted`，目标笔记也已写入，但对应 Bridge proposal 仍停留在 `pending`，导致 Bridge Inbox 与 Diff Review 状态不一致。已为 `BridgeProposalStore` 增加按 `source_ref` 更新状态的方法；当 ghost 全部接受时同步为 `applied`，全部拒绝时同步为 `rejected`，部分决策继续留待审。

### 真实 WebView/IPC 闭环

- 打开测试 vault：`D:/新项目仓库/贝叶斯笔记/.codex-e2e-vault`。
- 提交 `write_proposal` 人工 Bridge 任务，Bridge action 批准后真实调用 `.env` 中的 DeepSeek 配置。
- AI 返回短中文建议并生成 ghost；`get_diff_blocks` 返回 1 个待审文本块。
- 执行 `accept_diff` 后，`card-result.md` 成功追加内容，`read_note` 正常返回，无 `poisoned lock`。
- 新增状态同步复测：ghost `ghost_c37b5651ae1b463bad5c859fe5d2ef0f` 从 `pending` 变为 `accepted`，对应 Bridge proposal `bridge_b9a0aa6fbad34560bfe1bf49e710784f` 从 `pending` 变为 `applied`。
- 日志复查未发现新的 `panicked`、`Cannot block`、`poisoned` 或 Bridge 同步失败记录。

### 最新验证

- `cargo test -p feroha --lib -- --test-threads=1`：通过，330 个 Rust 单元测试全绿。
- `npx.cmd vitest run src/components/__tests__/HumanTaskIntake.test.tsx src/components/__tests__/BridgeInbox.test.tsx src/components/__tests__/CliMiniWindow.test.tsx src/components/__tests__/OrchestratorPanel.test.tsx --pool=threads --poolOptions.threads.singleThread=true`：通过，4 个测试文件、21 个测试全绿。
- `npm.cmd run build`：通过，TypeScript 与 Vite production build 成功；仍有既有主 chunk 体积 warning。
- `.env` 外密钥扫描：排除运行时锁定索引目录后结果为 `NO_SECRET_HITS`。

### 当前运行状态

- 完整 Tauri dev 保持运行，当前包装进程 PID 记录在 `.codex-tauri-dev.pid`。
- WebView2 CDP 端口 `9222` 可连接，真实 Tauri 页面为 `http://localhost:1420/`。
- `.codex-e2e-vault` 保留为本轮回归现场，里面包含 `seed.md`、Bridge/Ghost 审查记录和 `card-result.md` 的闭环结果。
