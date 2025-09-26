# wsl2-ssh-agent

Windows SSH Agent 桥接器，用于在 WSL2 中访问 Windows 系统的 SSH 密钥。

## 功能特性

* 双向通信桥接 WSL2 和 Windows SSH Agent

* 支持标准的 SSH 代理协议

* 可选的详细日志输出

* 基于 Tokio 的异步处理

* 自动重连机制

* 支持 Bitwarden SSH Agent 集成

## 快速开始

### 编译程序

```bash
cargo build --release
```

编译后的可执行文件位于 `target/release/wsl2-ssh-agent.exe`

### 设置启动脚本

将[ssh-agent.sh](ssh-agent.sh)脚本添加到你的 .bashrc 或 .zshrc 中：

```bash
source /path/to/ssh-agent.sh
```

自定义`wsl2-ssh-agent`路径：

```bash
source /path/to/ssh-agent.sh /path/to/wsl2-ssh-agent
```

自定义`wsl2-ssh-agent`参数：

```bash
source /path/to/ssh-agent.sh /path/to/wsl2-ssh-agent -v
```

### 确保 SSH Agent 服务运行

#### Windows OpenSSH Agent

在 Windows 管理员 PowerShell 中执行：

```PowerShell
# 启动 SSH Agent 服务
net start ssh-agent
```

设置服务为自动启动

```PowerShell
Set-Service ssh-agent -StartupType Automatic
```

#### Bitwarden 设置步骤

确保 Bitwarden SSH Agent 已启用：

在 Bitwarden 桌面应用中：设置 → SSH Agent → 启用 SSH Agent

### 在 WSL2 中测试

测试 SSH 连接

```bash
ssh -T <git@github.com>
```

列出已加载的密钥

```bash
ssh-add -l
```

1. 参数说明

```bash
SSH Agent Bridge - use Tokio forward stdin/stdout to Windows named pipe

Usage: wsl2-ssh-agent.exe [OPTIONS]

Options:
  -p, --pipe <PIPE>                Named pipe name [default: \\.\pipe\openssh-ssh-agent]
  -v, --verbose                    Enable verbose logging
  -r, --retries <RETRIES>          Connection retry count [default: 30]
      --retry-delay <RETRY_DELAY>  Retry delay (milliseconds) [default: 100]
  -h, --help                       Print help
  -V, --version                    Print version
```

## 故障排除

### 常见问题

#### 连接失败

启用详细日志诊断
wsl2-ssh-agent.exe --verbose

#### 管道不存在

检查 Windows 上的可用管道

```PowerShell
[System.IO.Directory]::GetFiles("\.\pipe") | Select-String "ssh"
```

#### Bitwarden 特定问题

确保 Bitwarden 桌面应用正在运行

确认 SSH Agent 功能已启用

检查 Bitwarden 中是否已添加 SSH 密钥

### 服务状态检查

#### 检查 OpenSSH Agent 服务状态

```PowerShell
Get-Service ssh-agent
```

#### 设置服务为自动启动

```PowerShell
Get-Process bitwarden
```

## 依赖要求

* WSL2

* Windows OpenSSH Agent 服务 或 Bitwarden 桌面应用

* socat（WSL2 中安装：sudo apt install socat）

## 许可证

[MIT License](LICENSE)
