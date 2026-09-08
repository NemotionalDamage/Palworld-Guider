# Palworld Guider（帕鲁游戏内只读向导）

> 英文版：[README.md](README.md)

一款只读、状态感知的《幻兽帕鲁》（Palworld）游戏内向导：在聊天框用 `!guide` 提问，即可获得基于版本化知识库、且可选结合玩家自身状态的简洁、可溯源回答——全程无需离开游戏。

项目已实现公开网页向导、离线 CLI 工具与只读游戏内 MOD。玩家在帕鲁聊天框输入 `!guide` 即可提问，并收到有据可查的简洁回答。

## 主要特性

- **确定性知识引擎** —— 每条游戏事实都来自经过审核、版本化、可溯源并纳入 Git 管理的 JSONL 数据，绝不来自 LLM 记忆。知识库从本地帕鲁 Steam 版提取，覆盖 1,520 个物品、299 只帕鲁与 1,805 条双语别名。
- **Rust 混合检索 + 确定性工具调用** —— 代理循环在有界 LLM 调用之外，组合精确标识符解析、结构化查询、Tantivy 词法检索与类型化 Rust 计算器；模型负责组织语言，但无法杜撰游戏事实或编造数量。
- **答案必须可溯源** —— 草稿中的数字、繁殖结果与实体断言会通过结构相邻性检查与工具证据比对；不合格草稿回退为基于证据的确定性回答，而非编造内容。
- **状态感知规划** —— 可选录入玩家状态快照，支持背包缺口分析、队伍工作缺口检测、当前可制作检查，以及带理由、需求与不确定性说明的 3–5 条优先级建议。
- **游戏内聊天界面** —— 可选的只读 UE4SS 适配器把帕鲁聊天框中的 `!guide` 问题送达向导服务器并返回简短回答，全程走经认证的回环 WebSocket。
- **版本与冲突感知** —— 版本不匹配、过期记录与来源冲突会被明确呈现，而非悄悄掩盖；维护 CLI 会审计来源、冲突与过期数据。
- **严格只读边界** —— 代码中不存在任何移动、战斗、采集、建造、物品变更或世界修改路径；Web 服务器与适配器网关只绑定 `127.0.0.1`。

## 架构

```text
                        ┌─────────────┐
  Player question ──── │ guide-agent │ ──── Grounded answer
        or !guide chat  │ (bounded   │      with provenance,
                         │  LLM loop) │      uncertainty, tools
                        └──────┬──────┘
                               │ typed tool calls
                    ┌──────────┼──────────┐
                    ▼          ▼          ▼
             ┌──────────┐ ┌─────────┐ ┌────────────┐
             │guide-core│ │knowledge│ │guide-planner│
             │(lookup + │ │-index   │ │(state-aware│
             │ calc)    │ │(Tantivy)│ │ planning)  │
             └────┬─────┘ └────┬────┘ └─────┬──────┘
                  │            │             │
                  ▼            ▼             ▼
             ┌──────────────────────────────────┐
             │         game-knowledge            │
             │  (JSONL store + validation)      │
             └──────────────────────────────────┘
                          │
                          ▼
                   data/reviewed/*.jsonl
             (1,520 items, 299 Pals, 1,805 aliases)
```

### 工作区 Crates（Rust 工作区成员）

| Crate | 用途 |
|---|---|
| `game-knowledge` | 类型化 schema、JSONL 存储加载、校验与本地构建导入 CLI |
| `knowledge-index` | 基于已审核摘要的内存 Tantivy 词法索引 |
| `guide-core` | 确定性查询、配方、材料、短缺与繁殖计算器 |
| `guide-tools` | 带 JSON schema、预算与结果封套的类型化工具注册表 |
| `provider` | OpenAI 兼容与 Ollama 聊天提供商（含超时与错误分类） |
| `guide-agent` | 带落地校验门、取消与回复上限的有界代理循环 |
| `state-snapshot` | 带新鲜度与完整性校验的版本化只读 `PlayerStateSnapshot` schema |
| `guide-planner` | 状态感知的进度规划器（输出排序后的下一步建议） |
| `guide-server` | 仅回环 Axum Web API、浏览器 UI 与可选适配器网关 |
| `game-gateway` | 面向 UE4SS 适配器的认证 schema-2 回环 WebSocket 网关 |
| `guide-adapter` | 游戏内聊天桥与适配器运行时（负责工具组合） |
| `guide-maintenance` | 版本检查、知识审计与批量校验 CLI |
| `guide-regression` | 42 个端到端回归测试（查询、计算、检索、落地与版本告警） |

## 快速开始

同一套有据可查的引擎，提供两种体验方式：

- **游戏内 MOD** —— 完整体验：直接在帕鲁聊天框用 `!guide` 提问。
- **仅 Web 网页端** —— 用浏览器获得同样的回答；无需 MOD，也无需运行游戏。

### 游戏内 MOD（完整体验）

只读游戏内 MOD 负责应答帕鲁聊天框中的 `!guide` 问题。整个过程全自动：脚本会定位 Steam 安装（无论游戏装在哪个盘、哪个目录）、找到 UE4SS 的 `Mods` 目录与最新存档、校验哈希，并且在完成存档备份并校验通过之前绝不写入游戏目录。

```powershell
git clone https://github.com/NemotionalDamage/Palworld-Guider.git
cd Palworld-Guider
cargo build --release
.\scripts\Setup-InGameGuide.ps1
.\scripts\Start-InGameGuide.ps1
```

#### 前置要求

- Windows 10/11 x64，PowerShell 5.1 及以上。
- 通过 Steam 安装《幻兽帕鲁》。游戏目录可以位于任意磁盘，脚本会从 Steam 库自动发现；游戏内聊天需要单机或私人会话。
- 游戏内已安装 UE4SS `3.0.1`：把 UE4SS 发布包解压到 `<游戏目录>\Pal\Binaries\Win64`，使 `UE4SS.dll` 与 `Palworld-Win64-Shipping.exe` 同目录（UE4SS 的标准安装方式）。预检会在任何操作前校验 DLL 哈希。
- Rust stable 1.98+（<https://rustup.rs>）。
- VS 2022 Build Tools，需包含“使用 C++ 的桌面开发”工作负载（自带 CMake 与 Ninja）。
- 在运行 `Start-InGameGuide.ps1` 的 PowerShell 会话中配置好模型服务（Provider）环境变量，见下文[配置](#配置)。

#### 安装（Setup）

`Setup-InGameGuide.ps1` 一条命令即可，无需任何路径参数：游戏根目录、UE4SS DLL 与最新存档都会自动发现（可选参数 `-GameRoot`、`-Ue4ssDll`、`-SaveDirectory`、`-OutputDirectory` 仅用于覆盖自动发现）。脚本会先做只读预检；若 `Mods\PalworldGuider` 已存在则拒绝继续（请先运行卸载脚本）；随后在 `.local\backups\palworld` 下为最新存档创建并校验备份；接着构建并暂存 MOD 包、校验暂存哈希清单；最后才安装 `Mods\PalworldGuider` 并在 `mods.txt` 中启用它，同时保留其它所有 MOD。加 `-WhatIf` 只报告计划执行的阶段、不做任何改动。任何预检失败都会在任何文件被写入之前停止。

#### 配置模型服务（Provider，仅需一次）

运行一次交互式配置，API Key 输入时会自动隐藏：

```powershell
.\scripts\Set-GuideProvider.ps1
```

配置会保存到 `.local\set-provider.ps1`（已被 Git 排除），之后
`Start-InGameGuide.ps1` 会自动加载。需要修改时加 `-Force` 重新运行即可。
你仍然可以在会话中手动设置 `GUIDE_PROVIDER`、`GUIDE_MODEL`、
`GUIDE_BASE_URL`、`OPENAI_API_KEY`；会话中的值优先于保存的文件。当
`GUIDE_PROVIDER=ollama` 时，需要先启动 Ollama（`ollama serve`），并确保
`GUIDE_MODEL` 指定的模型已拉取（`ollama list`）。

#### 启动（Start）

若帕鲁正在运行请先退出，然后执行：

```powershell
.\scripts\Start-InGameGuide.ps1
```

`Start-InGameGuide.ps1` 会重新校验已安装的包与哈希清单；在内存中生成随机网关令牌并绑定到当前 PowerShell 会话（绝不写入文件、注册表或命令行）；启动回环 guide-server；关闭残留的 Steam 客户端；再从同一会话通过 Steam 重新拉起帕鲁，使游戏进程继承该令牌。脚本只打印回环地址，绝不打印令牌：

```text
Web interface: http://127.0.0.1:8070/
Adapter gateway: 127.0.0.1:8071
```

如需更改回环端口，可使用 `-Port` 与 `-AdapterPort`。

#### 游戏内使用

进入单机或私人会话，在聊天框中输入：

```text
!guide ping
!guide <问题>
!guide retry
!guide new
```

- `!guide ping` 返回 `Pong: Palworld Guider adapter connected.`，不会调用模型服务。
- `!guide <问题>` 走与网页版相同的带落地校验的代理循环，可回答物品、材料、帕鲁、配方、繁殖、进度等各类问题，并附带知识库依据。
- 每个问题默认使用干净上下文，互不干扰；只有问题中明确指代上一轮回答
  （如 继续/刚才/continue/again）时才携带最近一轮作为追问上下文。
- `!guide retry` 清空上下文并重新提问上一个问题。
- `!guide new` 只清空对话记录，不重新提问。

之后在同一 Steam 窗口再次启动游戏会沿用本次会话令牌；如果 Steam 被完全关闭，请重新运行 `Start-InGameGuide.ps1`，让游戏继承新令牌。

若开启 UE4SS 后进存档崩溃（堆损坏 `0xc0000374`），需要按 [docs/troubleshooting.md](docs/troubleshooting.md) 第 12 条禁用 UE4SS 的世界加载钩子；本适配器不使用这些钩子。

#### 卸载

退出帕鲁后，仅移除本 MOD：

```powershell
.\scripts\Uninstall-Ue4ssAdapter.ps1 -ModsDirectory "<游戏目录>\Pal\Binaries\Win64\Mods"
```

`<游戏目录>` 即 Steam 中显示的帕鲁安装目录（库 → 管理 → 浏览本地文件）。卸载脚本只删除 `Mods\PalworldGuider` 及其在 `mods.txt` 中的那一行，其它 MOD 与 UE4SS 本身均不受影响。

### 仅体验 Web 网页端（无需 MOD、无需游戏）

如果只想体验 Web 网页端，不需要 UE4SS、不需要 MOD，也不需要运行游戏。克隆、构建并启动回环服务器即可：

```powershell
git clone https://github.com/NemotionalDamage/Palworld-Guider.git
cd Palworld-Guider
cargo build --release

.\scripts\Set-GuideProvider.ps1
.\scripts\Start-WebGuide.ps1
```

上面的 Provider 配置与 MOD 完全共用，整个克隆只需配置一次。

在浏览器打开 <http://127.0.0.1:8070/>。Web 端与 MOD 共用同一套有据可查的问答引擎：可以提出同样的问题、上传玩家状态快照进行状态感知规划、查看每条回答的依据，并在同一会话中多轮追问。在终端按 Ctrl+C 即可停止服务器。

## 命令行工具

### 确定性离线 CLI（无需 LLM）

完全离线运行，不依赖任何模型服务或游戏状态：

```powershell
cargo run -p guide-core -- lookup item Wood
cargo run -p guide-core -- recipe "Wooden Club"
cargo run -p guide-core -- materials 3 "Wooden Club"
cargo run -p guide-core -- shortage --inventory "Wood=6" 3 "Wooden Club"
cargo run -p guide-core -- craftable --inventory "Wood=14" "Wooden Club"
cargo run -p guide-core -- breeding Lamball Lamball
cargo run -p guide-core -- chain 4 Lamball Lamball
```

所有命令返回 JSON，包含 `status`、`data`、`provenance`、`version`、`uncertainty` 与 `errors` 字段。退出码：`0` = 成功，`1` = 未知/有歧义，`2` = 出错。

### 自然语言问答（使用 LLM）

```powershell
# Ollama（本地）
cargo run -p guide-agent -- ask "How do I get Wood?" --provider ollama --model llama3.2

# OpenAI 兼容服务
$env:OPENAI_API_KEY = "your-key"
cargo run -p guide-agent -- ask "Materials for 3 Wooden Clubs?" --provider openai --model gpt-4o-mini
```

### 知识库维护

```powershell
cargo run -p guide-maintenance -- version-check --data data/reviewed --game-version 1.0.3
cargo run -p guide-maintenance -- audit-sources --data data/reviewed
cargo run -p guide-maintenance -- audit-conflicts --data data/reviewed
cargo run -p guide-maintenance -- audit-stale --data data/reviewed --game-version 1.0.3
cargo run -p guide-maintenance -- validate-batch path/to/candidates.jsonl
```

## 配置

所有配置只通过环境变量提供。`Set-GuideProvider.ps1` 会把 Provider 配置保存到被 Git 忽略的 `.local\set-provider.ps1`，凭据不会出现在命令行或 Git 历史中。

两个启动脚本默认最多等待模型 300 秒。如果你的模型明显更快，可以传入更小的值，例如 `-ProviderTimeoutSeconds 120`。

| 变量 | 使用方 | 说明 |
|---|---|---|
| `GUIDE_PROVIDER` | `guide-server` | `openai` 或 `ollama`（必填） |
| `GUIDE_MODEL` | `guide-server` | 模型名，例如 `gpt-4o-mini` 或 `llama3.2`（必填） |
| `OPENAI_API_KEY` | `guide-server`、`guide-agent` | OpenAI 兼容服务的 API 密钥（仅从环境变量读取，绝不写入日志） |
| `GUIDE_BASE_URL` | `guide-server` | 覆盖模型服务地址 |
| `GUIDE_DISABLE_REASONING` | `guide-server`、`guide-agent` | 设为 `1` 可关闭推理 token |
| `OLLAMA_BASE_URL` | `guide-agent` | 覆盖 Ollama 地址（默认为 `http://localhost:11434`） |
| `PALWORLD_GUIDER_GATEWAY_TOKEN` | `guide-server`、游戏内适配器 | 回环适配器网关的 Bearer 令牌（16–4096 字符，绝不记录日志）；`Start-InGameGuide.ps1` 会绑定到启动它的会话 |
| `PALWORLD_GUIDER_GATEWAY_PORT` | 游戏内适配器 | 适配器读取的回环网关端口；`Start-InGameGuide.ps1` 将其设为 `-AdapterPort`（默认 `8071`） |
| `PALWORLD_GUIDER_CHAT_DEBUG_LOG` | `guide-server`（可选） | 聊天调试 JSONL 日志路径（位于被 git 忽略的目录内） |

完整的 CLI 参数、服务器与载荷限制、日志轮转等见 [docs/configuration.md](docs/configuration.md)。

## 项目结构

```text
adapter/
  read-only/ue4ss/          # UE4SS Lua/C++ 混合适配器（只读）

crates/                     # 13 个 Rust 工作区 crate
  game-knowledge/           # schema、JSONL 存储、校验、导入 CLI
  knowledge-index/          # Tantivy 词法索引
  guide-core/               # 确定性查询与计算 CLI
  guide-tools/              # 类型化工具注册表
  provider/                 # OpenAI 兼容与 Ollama 提供商
  guide-agent/              # 带落地校验门的有界代理循环
  state-snapshot/           # 玩家状态 schema
  guide-planner/            # 状态感知进度规划器
  guide-server/             # 回环 Axum Web API + 浏览器 UI
  game-gateway/             # 认证回环 WebSocket 网关
  guide-adapter/            # 游戏内聊天桥
  guide-maintenance/        # 版本检查与审计 CLI
  guide-regression/         # 回答回归测试集（42 个测试）

data/
  reviewed/                 # 规范化的已审核 JSONL 知识库
    sources.jsonl           #   3 个已登记来源
    items.jsonl             #   1,520 个物品
    pals.jsonl              #   299 只帕鲁
    aliases.jsonl           #   1,805 条双语别名
    facts.jsonl             #   38 条种子事实与进度边

docs/                       # 完整项目文档
  mod-rollout-plan.md
  installation.md
  configuration.md
  troubleshooting.md
  data-updates.md
  deployment.md
  operations.md
  knowledge-source-policy.md
  reference-data/
  schemas/

scripts/                  # UE4SS 预检、构建、备份、安装、安装向导、启动、卸载
  Test-InGameGuide.ps1
  Build-Ue4ssAdapter.ps1
  Backup-PalworldSave.ps1
  Install-Ue4ssAdapter.ps1
  Uninstall-Ue4ssAdapter.ps1
  Setup-InGameGuide.ps1
  Start-InGameGuide.ps1
```

## 知识库

知识库是游戏事实的唯一权威来源，具备以下特性：

- **经过审核** —— 每条事实都从本地帕鲁 Steam 版提取（使用 FModel 与 PalworldModding/UsefulFiles 映射），在晋级前经过逐字段人工审核。
- **版本化** —— 每条记录都带有 `applicable_game_version`、来源 ID、提取日期、审核状态与置信度。
- **可溯源** —— 来源登记在 `docs/reference-data/source-log.md`，优先级为：本地目标版本数据 > 官方文档 > 经审核的二手资料 > 社区资料。
- **冲突可见** —— 来源之间的分歧会记录为 `ConflictRecord`，绝不悄悄掩盖。
- **可 diff** —— 以 JSONL 存储，便于人工审核与 Git 差异比较。

来源优先级：

| 优先级 | 来源 | 置信度 |
|---|---|---|
| 1 | 本地目标版本数据 | `verified-target` |
| 2 | 官方补丁说明/文档 | `official` |
| 3 | 用户审核的二手资料 | `reviewed-secondary` |
| 4 | 社区维基/数据库 | `community` |
| 5 | LLM 推断假设 | 永不固化为事实 |

完整的知识更新流程见 [docs/data-updates.md](docs/data-updates.md)。

## 测试

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

工作区共 13 个 crate、327 个测试，包括：

- schema、引用完整性、版本、别名与冲突测试
- 配方树、短缺、可制作数量、繁殖与循环检测等计算器测试
- Mock 提供商下的代理循环与落地校验门回归测试
- 状态快照校验与规划器排序测试
- 42 个端到端回归测试：查询准确性、计算正确性、检索命中率、幻觉率与版本告警行为

## 安全与只读边界

- Web 服务器与适配器网关**只**绑定 `127.0.0.1`，`--host` 参数被显式拒绝。
- 适配器只读取玩家位置与当前出战帕鲁（Otomo）的标识/位置；不读取背包、队伍、生命、耐力、存档或附近单位。
- 模型可见的工具白名单在编译期固定：`get_player_status` 与 `get_active_pal_status`；`send_chat_message` 为内部工具，模型永远无法调用。
- API 暴露的状态快照会裁剪为 schema 版本、来源类型、游戏版本、新鲜度与缺失字段；原始快照永不进入模型提示词。
- 不存在任何修改路径：没有移动、战斗、采集、建造、物品变更或世界写入。

## 文档

| 文档 | 内容 |
|---|---|
| [docs/installation.md](docs/installation.md) | 构建、数据布局、首次运行与验证命令 |
| [docs/configuration.md](docs/configuration.md) | 环境变量、CLI 参数、服务器/载荷限制、日志轮转 |
| [docs/deployment.md](docs/deployment.md) | 游戏内 MOD 安装/启动细节、纯网页版部署、API 端点与安全检查 |
| [docs/troubleshooting.md](docs/troubleshooting.md) | 12 个常见问题：原因与解决办法 |
| [docs/data-updates.md](docs/data-updates.md) | 知识更新流程、来源日志、冲突处理与版本追踪 |
| [docs/operations.md](docs/operations.md) | 性能预算、崩溃恢复、备份、密钥脱敏与监控 |
| [docs/mod-rollout-plan.md](docs/mod-rollout-plan.md) | 游戏内 MOD 源码路径与安装包发布路径的推进计划 |

## 许可

本项目为私有向导工具，只存储经过审核与转换的事实与来源元数据，不含游戏资产、受版权保护的文本或整页转储。数据与版权政策见 [docs/knowledge-source-policy.md](docs/knowledge-source-policy.md)。
