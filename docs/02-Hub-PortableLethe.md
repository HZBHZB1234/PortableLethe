# Hub 设计 — PortableLethe（Tauri 2，纯 Rust 内核）

> 仓库：https://github.com/HZBHZB1234/PortableLethe （空仓库起步）
> 硬约束：**内部不引入 C# / Python 运行时或代码**；外部 `dotnet publish` 只是工具链调用。

---

## 1. 分层结构

```
┌──────────────────────────────────────────────────────────────┐
│ React UI (src/)                                              │
│  仪表盘 | 版本 | 资源 | 配置 | 实例 | 客户端包 | 日志          │
└───────────────────────┬──────────────────────────────────────┘
                        │ Tauri command（薄封装，只做参数/事件转发）
┌───────────────────────▼──────────────────────────────────────┐
│ Rust Core (src-tauri/)                                       │
│  version │ config │ process │ resources │ storage │ package  │
└───────┬───────────┬───────────┬───────────┬──────────┬───────┘
        │           │           │           │          │
   reqwest/zip  serde_json  std::process  catalog_parser  git2
   sha2/reflink  config-schema  tauri-plugin-shell  unityfs  twox-hash
        │           │           │           │          │
        └───────────┴───────────┴───────────┴──────────┘
                       外部进程（仅工具链）
              dotnet SDK (publish)  |  git CLI (可选)
```

**换栈策略**：UI 与 Rust core 之间只走 Tauri command契约（`commands.rs` 里的 `#[tauri::command]`），UI 层可整体替换；core 内部各模块以 trait + 事件总线解耦，未来重构底层不影响 command 签名。

---

## 2. Crates 选型

| 用途 | crate | 备注 |
|---|---|---|
| HTTP / 断点续传 | `reqwest`（stream, rustls-tls-native-roots） | Range + ETag |
| 解压 | `zip` | 预编译包 / 客户端包 |
| 校验 | `sha2` | 官方发布哈希 / 下载完整性 |
| Git | `git2` | 免装 git；`https://gh-proxy.com/https://github.com/...` 前缀 |
| Unity 资源 | `unityfs = 0.1.4` | **已实测**，见 04 |
| xxhash | `twox-hash` | launcher 缓存格式一致 |
| 配置 | `serde` / `serde_json` / `rust-ini` | schema + Lethe.cfg 编辑 |
| 硬链接/reflink | `reflink` + std::fs::hard_link | 存储去重 |
| 日志 | `tauri-plugin-log` | UI 日志面板 |
| 进程 | `std::process` + `tauri-plugin-shell` | 服务端进程管理 |
| 本地元数据（可选） | `rusqlite` | 也可用 JSON 状态文件，v1 建议 JSON |

---

## 3. 模块一：版本管理

- **预编译通道（默认）**：从 `PortableOpenLethe` GitHub Releases（或可配置 URL）拉取 `latest.json` → `download_url` → 经 `gh-proxy.com` 加速下载 zip → SHA256 校验 → 解压到 `hub/versions/<version>/`。
- **源码编译通道（进阶）**：
  1. `git2` clone/fetch `PortableOpenLethe`（可配 gh-proxy 前缀）；
  2. 若本机无 SDK，从官方 dotnet-install 下载 portable SDK 到 `hub/tools/dotnet`（约 200MB，UI 明确提示体积）；
  3. `dotnet publish -c Release -r win-x64 --self-contained -o <staging>`；
  4. **静态数据不再通过 `-p:StaticDataSourceDir` 嵌入**（已废弃该机制），发布后把 `hub/resources/official/current/` 软链或复制到 `<staging>/static/`；
  5. 校验产物关键文件存在 → 注册为新版本。
- **切换**：Windows 文件锁决定必须先停服 → 把 `current` junction 指向新版本 → 启动。保留最近 N 个版本，一键回滚（同样 stop→switch→start）。
- **原子性**：下载/解压全部在 `<version>.tmp` 目录，成功后 rename；中途失败不影响 `current`。

---

## 4. 模块二：配置托管

- 读取服务端 `config-schema.json`（语言无关），生成表单；字段标注 `owner: hub|server`、`requiresRestart`、`secret`。
- Hub 只写自己拥有的键到 `data/instances/<id>/server.bootstrap.json`：端口、JWT 密钥/有效期、限流、超时、请求体上限、CORS、MOD_FILES_DIR、STATIC_DATA_DIR、DB 路径、插件开关/目录、实例 ID。
- 密钥存 Windows 凭证管理器 / DPAPI，UI 脱敏显示。
- 写入用「临时文件 → 原子 rename」，写坏时保留上一版备份；保存后通过 `/hub/request-restart` 或进程管理重启实例。
- Hub 拥有的配置**不写进服务端面板可编辑区**（面板显示"请在 LetheHub 修改"）。

---

## 5. 模块三：进程管理

- `Process::new(版本目录/exe)` + 当前工作目录设为实例目录；stdout/stderr 重定向到 `instances/<id>/logs/server.log`，UI 实时尾部显示。
- 启动后轮询 `http://127.0.0.1:<port>/health`；超时/非 200 视为失败。
- 崩溃退避重启（1s→2s→4s→…→30s 上限）；手动停止时不再自动拉起。
- 响应服务端 `RestartRequested`：stop → start。
- 多实例：每实例独立端口、进程句柄、日志、状态机（`Stopped/Starting/Running/Unhealthy/Updating`）。

---

## 6. 模块四：资源同步

详见 `04-资源同步与静态数据.md`。Hub 在此模块实现：
- 官方站点镜像：modfiles（`/Lethe.dll`、`/ModularSkillScripts.dll`、`/motions.dll`、`/limbus-manifest.txt`、`/noticeMeta.json`、`/news.json`）；
- 官服静态数据：`catalog_S1.bin` 解析 + `unityfs` 提取 TextAsset → `hub/resources/official/current/StaticData/` 与 `Localize/`；
- 官服 bundle 差异同步：Rust 版 `sync_server_bundles.py` 逻辑；
- 全部只写内容寻址仓库，实例通过 junction 引用；更新后标记"需重启"并触发实例重启。

---

## 7. 模块五：多实例与存储优化

目录布局：
```
hub/
  blobs/sha256/<hash>                 # 内容寻址，只存一份
  versions/<version>/                 # 服务端发布版（可从 blobs 硬链）
  resources/official/<version>/        # 官方资源镜像
  resources/current -> official/<ver>  # junction
  instances/<id>/
    server -> ../../versions/<ver>     # junction
    static -> ../../resources/current  # junction
    data/                              # 实例私有：SQLite/配置/插件/脚本/日志
  clientpkg/
```
- 相同内容优先 `hard_link`（NTFS 同卷）；跨卷回退 `reflink` 或普通复制。
- 实例删除只删 overlay `data/` 与 junction，不动全局仓库。
- 端口分配：从配置区间自动分配并检测占用。
- 并发更新：全局仓库写操作加文件锁；一个实例更新期间其他实例继续运行。

---

## 8. 模块六：客户端发布包导出

**只分发预编译 launcher**（不编译 Rust）。

包内容（用户确认的边界）：
```
PortableLetheClient-<ver>.zip
  LimbusCompany.exe            # PortableLetheLauncher 预编译产物
  BepInEx/core/*               # 私服 profile 基线（来自 LetheLauncher-Distribution-7）
  BepInEx/doorstop/*           # doorstop
  BepInEx/interop/*            # Il2CppInterop
  BepInEx/patchers/*, unity-libs/*
  dotnet/*                     # CoreCLR 运行库
  BepInEx/plugins/             # Lethe.dll / ModularSkillScripts.dll / motions.dll（来自实例 modfiles）
  BepInEx/config/Lethe.cfg     # ServerRoot 指向目标实例；IgnoreLauncherToken 按模式设置
  doorstop_config.ini
  lethe-launcher.ini           # 预置 ServerUrl / ProfileId（由 launcher 读取）
  README.txt                   # 安装/切换说明
```
**不包含**：游戏本体、Steam 文件、Addressables bundle、ZylarNex。
导出时从实例读取当前端口/地址与 modfiles，自动写入配置；生成 zip 并记录 SHA256。

---

## 9. IPC 契约（换栈边界）

Tauri commands（稳定签名）：
`hub_status`、`hub_config_schema`、`hub_config_get/set`、`instance_list/create/start/stop/restart/delete`、`version_list/install/switch/rollback`、`resource_sync/status`、`clientpkg_export`、`log_tail`。

事件：`instance-state`、`sync-progress`、`download-progress`、`log-line`、`restart-requested`。

所有 command 的参数/返回值用 serde 定义的结构体，前端由 TS 类型镜像；**UI 不直接访问文件系统或进程**。

---

## 10. 安全

- Hub 自身不暴露网络监听（纯本地 GUI）；若未来加远端 API，绑 loopback + token。
- 下载必须校验 SHA256；manifest/官方页面内容按不可信数据处理。
- 插件/脚本包从 Hub 推送到服务端时走管理 token + 审计日志。
- Windows 长路径与文件锁：所有路径操作前规范化；切换版本前强制停服。

---

## 11. 验收标准

1. Hub 可在无 SDK、无 Python、无 Docker 的 Windows 机器上运行；
2. 下载预编译版 → 启动 → `/health` 200 → 停止 → 回滚，全链路 UI 可操作；
3. 源码编译通道能下载 portable SDK 并产出可运行版本；
4. 资源同步后静态数据对预编译服务端生效；
5. 两个实例同时运行，资源只存一份（硬链接），端口/数据隔离；
6. 导出的客户端包在干净游戏目录解压后，launcher 能连接对应实例。
