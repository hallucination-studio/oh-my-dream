# oh-my-dream 设计文档

> 版本：草案 v0.1 · 最后更新：2026-07-07

本文件定义 oh-my-dream 的架构、数据结构与需求拆分。它是活文档，随实现推进修订。

---

## 1. 定位与边界

oh-my-dream 是一个**本地桌面 AI 创作客户端**，用可视化节点工作流串联生成能力，首要链路是 **文生图 → 图生视频**。

设计参考 ComfyUI 的节点工作流思想，**只借鉴、不照搬**。

### 明确的产品决策

| 决策 | 取值 | 影响 |
|---|---|---|
| 产品形态 | 本地桌面客户端 | 规划走 Tauri，单二进制分发 |
| 账号体系 | 无需登录 | 不做用户系统、云同步 |
| 推理位置 | **本机不跑显卡** | 全部走云 API；本地推理留接口不实现 |
| 推理来源 | **多云厂商 API** | 需统一参数模型 + 每家一个 adapter |
| 功能范围 | 做减法 | 只做核心链路必需项 |

### 首批功能

1. 文生图（Text → Image）
2. 图生视频（Image → Video）
3. 可视化节点工作台
4. 资产库

---

## 2. 为什么用 Rust（以及它的价值边界）

纯云 API 编排的本质是 **async HTTP + 任务轮询 + 状态管理 + 本地文件/DB**，Rust 在这类工作上稳定、并发好、资源占用低，配合 Tauri 可得到轻量的桌面客户端。

需要清醒认识：生成任务的耗时几乎全在云端 GPU，**语言性能不是本产品的瓶颈**。Rust 的价值在于客户端体验、引擎的健壮性与可维护性、单二进制分发，而非"算得快"。

---

## 3. 分层架构

```
┌─────────────────────────────────────────────┐
│  ui/            前端节点画布（拖拽/连线/参数/进度）  │
├─────────────────────────────────────────────┤
│  src-tauri/     桌面壳，桥接前端与 Rust crates      │
├─────────────────────────────────────────────┤
│  crates/engine    工作流引擎（纯逻辑，无 UI/无网络）  │
│  crates/nodes     具体节点实现                      │
│  crates/backends  云 API 适配层（可插拔，多厂商）     │
│  crates/assets    资产库（SQLite + 文件 + 缩略图）    │
└─────────────────────────────────────────────┘
```

分层纪律：`engine` 不依赖 UI / 网络 / 文件系统 / 具体厂商；推理后端一律经 trait 抽象。

### 目录结构（规划）

```
oh-my-dream/
├── crates/
│   ├── engine/          # 图模型、拓扑排序、调度、缓存、节点 trait、类型系统、注册表
│   ├── nodes/           # TextPrompt / TextToImage / ImageToVideo / SaveAsset
│   ├── backends/        # InferenceBackend trait + fal / replicate / ... / local(占位)
│   └── assets/          # 资产库 store + thumbnail
├── src-tauri/           # Tauri 后端
├── ui/                  # 前端画布
└── docs/                # 本文档等
```

---

## 4. 工作流引擎（crates/engine）

对标 ComfyUI 的 `execution.py`，用 Rust 重写为纯逻辑层。

### 4.1 职责

- **图模型**：节点、连线的数据表示。
- **拓扑排序**：按依赖决定执行顺序，检测环。
- **调度执行**：依序执行节点，收集输出，向上层推送进度/错误。
- **缓存**：节点输入 hash 不变则复用上次输出。云 API 调用 = 花钱，缓存直接省钱，优先级高。
- **类型系统**：连线时校验输出类型与输入类型一致。

### 4.2 节点抽象

借鉴 ComfyUI 的 `INPUT_TYPES / RETURN_TYPES / FUNCTION` 三要素，但用 Rust trait 表达：

```rust
// 示意，非最终签名
trait Node {
    fn type_id(&self) -> &str;
    fn input_types(&self) -> &[PortSpec];   // 名称 + 类型 + 是否必填 + 默认
    fn output_types(&self) -> &[PortSpec];
    async fn run(&self, ctx: &mut RunContext, inputs: Inputs) -> Result<Outputs>;
}
```

节点通过**注册表**按 `type_id` 注册与查找，工作流反序列化时据此实例化。

### 4.3 数据类型（连线类型）

MVP 需要：`STRING`、`IMAGE`、`VIDEO`、`MODEL`（模型标识）、`INT`、`FLOAT`。
仅同类型端口可连；类型不匹配在连线期即报错，不等到执行。

---

## 5. Workflow 数据格式

**参考 ComfyUI 但重新设计。** 明确改进两点：

1. **逻辑与布局分离**：`position` 等 UI 信息与执行逻辑解耦。
2. **命名端口而非数组下标**：`inputs` 引用来源输出用**名字**，不用 ComfyUI 的数字下标，抗节点改版。

```jsonc
{
  "version": "1.0",
  "nodes": [
    {
      "id": "n1",
      "type": "TextPrompt",
      "params": { "text": "a cat" },
      "position": [100, 200]
    },
    {
      "id": "n2",
      "type": "TextToImage",
      "params": { "model": "flux-1", "steps": 28, "seed": 42 },
      "inputs": { "prompt": ["n1", "text"] }   // [来源节点id, 来源输出名]
    },
    {
      "id": "n3",
      "type": "ImageToVideo",
      "params": { "duration": 4, "fps": 24 },
      "inputs": { "image": ["n2", "image"] }
    }
  ]
}
```

从 ComfyUI 保留的好设计：强类型连线、结果缓存、把生成参数快照嵌入产物以便资产反查工作流。
砍掉的包袱：litegraph 冗长 JSON、`widgets_values` 按下标对齐的脆弱设计。

---

## 6. 推理后端适配层（crates/backends）

各云厂商 API 差异大（同步返图 vs 提交后轮询 vs webhook，参数名与取值各异），因此：

- **统一中间参数模型**：引擎只认 `T2IRequest` / `I2VRequest` 等中立结构。
- **每厂商一个 adapter**：负责把中立参数翻译为该厂商请求，并把响应归一化。

```rust
#[async_trait]
trait InferenceBackend {
    async fn text_to_image(&self, req: T2IRequest) -> Result<TaskHandle>;
    async fn image_to_video(&self, req: I2VRequest) -> Result<TaskHandle>;
    async fn poll(&self, handle: &TaskHandle) -> Result<TaskStatus>;
}
```

- 首批实现一个真实厂商（fal.ai 或 Replicate，二选一先落地）。
- `LocalBackend` 作为占位，兑现"为本地推理保留接口"的承诺，暂不实现。
- 任务需支持 **进度回调** 与 **取消**（对标 ComfyUI 的 `/interrupt`）。
- 密钥来自环境变量 / 本地配置，**绝不入库**。

---

## 7. 资产库（crates/assets）

- **存储**：产物（图片/视频）落本地目录；元数据入 SQLite。
- **字段**：`id / type / 文件路径 / 缩略图 / 生成参数(workflow 快照) / 来源节点 / 创建时间 / 标签`。
- **功能**：网格浏览、按类型筛选、**从资产反查生成它的工作流参数**、缩略图（视频抽首帧）。

---

## 8. 里程碑

每步都能独立验证，逻辑先行、UI 最后。

1. **引擎骨架**：节点 trait + 拓扑排序 + 类型校验 + 缓存，用假节点（如 `TextPrompt → UpperCase → Print`）跑通调度，`cargo test` 可验证。
2. **接第一个真实推理**：只做文生图，接一个云厂商，命令行能出图。
3. **资产库**：产物自动入库，可浏览、可看生成参数。
4. **Tauri + 画布 UI**：把前三步接入界面，可视化拖拽节点。
5. **图生视频节点**：串成 文生图 → 图生视频 完整链路。

---

## 9. 待定问题

- 首批接哪家云厂商（fal.ai / Replicate / 其他）？
- 前端画布库选型（reactflow / rete / litegraph）？
- 是否需要与 ComfyUI workflow 做导入/导出互通？
