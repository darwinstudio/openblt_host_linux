# OpenBLT 烧录工具

基于 [OpenBLT](https://www.openblt.org/) 的桌面固件烧录工具，支持 RS232 串口和 USB 两种通信方式，通过 XCP 协议烧录 Motorola S-record 格式固件到目标板。

## 功能

- **RS232 串口烧录** — 支持自定义串口设备路径和波特率
- **USB 烧录** — 使用固定 VID/PID `0x1D50/0x60AC`，无需额外配置
- **固件解析预览** — 选择固件文件后展示段数、总大小、地址范围等信息
- **自动重试连接** — 下位机复位后自动等待 backdoor 窗口并重连，无需手动反复操作
- **实时进度反馈** — 烧录进度条 + 日志输出
- **设置持久化** — 通道、串口、波特率等参数自动保存，下次启动无需重填

## 技术栈

| 层级 | 技术 |
|------|------|
| 前端 | Vue 3 + TypeScript + Naive UI |
| 后端 | Rust (Tauri 2) |
| FFI | 预编译 `libopenblt.so`（OpenBLT 官方库） |
| 包管理 | pnpm (前端) / Cargo (Rust) |

## 项目结构

```
.
├── src/
│   ├── App.vue              # 前端主界面（单文件组件）
│   └── main.ts              # Vue 入口
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs           # Tauri 命令（version / program / firmware_info）
│   │   └── openblt.rs       # LibOpenBLT FFI 绑定
│   ├── build.rs             # 链接 libopenblt.so，设置 rpath
│   ├── libopenblt.so        # 预编译共享库（非源码构建）
│   └── tauri.conf.json      # Tauri 配置
├── package.json
└── README.md
```

## 前置条件

- [Node.js](https://nodejs.org/) (>= 18) + pnpm
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- Linux 系统

## 快速开始

```bash
# 安装前端依赖
pnpm install

# 开发模式
pnpm tauri dev

# 构建 .deb 安装包
pnpm tauri build
```

## 使用说明

1. 启动应用后，点击「设置」配置通信通道（RS232 或 USB）
2. 点击「选择固件」加载 `.s19` / `.s28` / `.s37` / `.srec` / `.mot` 格式的 S-record 文件
3. 主界面会展示固件段数、总大小、起止地址等概览信息
4. 点击「烧录」开始，日志区会输出连接和烧录进度

## 相关链接

- [OpenBLT 官网](https://www.feaser.com/openblt/doku.php?id=homepage)
- [OpenBLT GitHub](https://github.com/feaser/openblt)
- [Tauri 2 文档](https://v2.tauri.app/)
- [Naive UI](https://www.naiveui.com/)

## 作者

shenzan & Hy3
