---
name: patheditor-scoop-release
description: PathEditor 发布后同步 scoop bucket 的收尾流程。当用户说「更新 scoop」「bucket 更新一下」「发版后同步 scoop」，或在 PathEditor 完成发版（推送 v* tag、Release 已发布）后要求处理包管理器时使用。覆盖确认发版状态 → 核对产物命名与校验值 → 更新/新增 manifest → 更新 README → 本地验证 → 提交推送 → 汇报本机影响。
---

# PathEditor 发版后同步 scoop bucket

**触发时机**：PathEditor 完成发版（tag 已推、GitHub Release 已发布）之后。scoop 的 manifest 依赖 Release 上的产物，**发版没完成就先做这个会指向不存在的文件**。

bucket 仓库位置：`D:\Code\doing_exercises\programs\scoop-bucket`（`git@github.com:LHY0125/scoop-bucket.git`，分支 `master`）。它同时作为本地 bucket 挂在 `D:\settings\settings\Scoop\buckets\lhy`。

---

## ⓪ 前置检查（缺一不可）

1. **Release 已发布且产物齐全**。先跑：

   ```bash
   gh release view v<VERSION> --json assets -q '.assets[] | "\(.name)  |  \(.size)  |  \(.digest)"'
   ```

   预期三个产物（缺任何一个都要先回 PathEditor 处理，不要继续）：

   | 产物                              | 说明                            |
   | --------------------------------- | ------------------------------- |
   | `PathEditor_<V>_x64-setup.exe`    | NSIS 安装包                     |
   | `PathEditor_<V>_x64-portable.zip` | 免安装 zip（GUI manifest 的源） |
   | `patheditor-cli_<V>_x64.exe`      | CLI 单文件                      |

2. **产物名必须与 CI 命名规则一致**。`release.yml` 的重命名规则是 `patheditor-cli_<V>_x64.exe`；`autoupdate` 模板依赖它。**若 Release 上是 `patheditor.exe` 之类的原名，说明是手动上传的**（CI 未走完），此时 manifest 的 `autoupdate` 会 404。处理方式：补传标准命名的文件并删掉不规范的重名文件，**补传前先核对 SHA256 一致**，确认是同一个二进制。

3. **确认 bucket 仓库干净**：`git status --short` 应为空。

---

## ① 取校验值

三个产物各自的 sha256 都要取。**不要手算，用命令**：

```bash
gh api repos/LHY0125/PathEditor/releases/tags/v<VERSION> \
  --jq '.assets[] | "\(.name)  \(.digest)"'
```

`digest` 字段实测可用（不必担心 immutable releases 的问题）。若为空，退回本地 `sha256sum` 或 `Get-FileHash`。

---

## ② 写 manifest

两个 manifest，**名字必须成对且无歧义**：

| 文件                         | 安装命令                           | 源产物                            |
| ---------------------------- | ---------------------------------- | --------------------------------- |
| `bucket/patheditor-gui.json` | `scoop install lhy/patheditor-gui` | `PathEditor_<V>_x64-portable.zip` |
| `bucket/patheditor-cli.json` | `scoop install lhy/patheditor-cli` | `patheditor-cli_<V>_x64.exe`      |

> **命名红线**：`extras` bucket 里有一个无关的第三方项目占用了 `patheditor` 这个名字（`patheditor2` 1.0，CodePlex 归档）。**GUI 的 manifest 必须叫 `patheditor-gui`**，绝不能叫 `patheditor`，否则 `scoop install patheditor` 会解析到 extras 并产生歧义。

### GUI manifest（zip 版，推荐形态）

```json
{
  "version": "<V>",
  "description": "Graphical editor for the Windows PATH and environment variables",
  "homepage": "https://github.com/LHY0125/PathEditor",
  "license": "MIT",
  "architecture": {
    "64bit": {
      "url": "https://github.com/LHY0125/PathEditor/releases/download/v<V>/PathEditor_<V>_x64-portable.zip",
      "hash": "sha256:<上面取到的 digest>"
    }
  },
  "shortcuts": [["patheditor.exe", "PathEditor"]],
  "checkver": {
    "url": "https://api.github.com/repos/LHY0125/PathEditor/releases/latest",
    "jsonpath": "$.tag_name",
    "regex": "v?([\\d.]+(?:-[0-9A-Za-z.-]+)?)"
  },
  "autoupdate": {
    "architecture": {
      "64bit": {
        "url": "https://github.com/LHY0125/PathEditor/releases/download/v$version/PathEditor_$version_x64-portable.zip",
        "hash": {
          "url": "https://api.github.com/repos/LHY0125/PathEditor/releases/tags/v$version",
          "jsonpath": "$.assets[?(@.name == 'PathEditor_$version_x64-portable.zip')].digest"
        }
      }
    }
  }
}
```

**为什么用 zip 而不是 NSIS 安装包**：Tauri 的 `--bundles` 只支持 `msi`/`nsis`，zip 由 `release.yml` 手工 `Compress-Archive` 产出（内容是 `patheditor.exe` + `WebView2Loader.dll` 两个文件）。zip 解压即用，符合 scoop 的常规做法；而用 `7z` 解 NSIS 安装包的方案在实际机器上失败过（scoop 未能解包，只留下空壳安装目录），**不要回退到那个方案**。

### CLI manifest

结构与 GUI 相同，差异只有三处：`url` 用 `patheditor-cli_<V>_x64.exe#/patheditor.exe`（`#` 后是重命名）、加 `"bin": "patheditor.exe"`、去掉 `shortcuts`。

---

## ③ 更新 README

`README.md` 的 Manifests 表要与实际文件同步，并保留 extras 冲突的提醒：

````markdown
## Usage

```powershell
scoop bucket add lhy https://github.com/LHY0125/scoop-bucket
scoop install lhy/patheditor-gui    # 图形界面
scoop install lhy/patheditor-cli    # 命令行
```
````

````

表里两行（`patheditor-gui` / `patheditor-cli`），并保留这句：

> 注意：`extras` bucket 中另有一个无关项目占用了 `patheditor` 这个名字，请勿安装 `extras/patheditor`。

---

## ④ 验证

### 4.1 沙箱内能做的（必做）

```bash
node -e "JSON.parse(require('fs').readFileSync('bucket/patheditor-gui.json','utf8'));console.log('OK')"
powershell -NoProfile -Command "scoop info ./bucket/patheditor-gui.json"
````

`scoop info` 应正确显示 `Name` / `Version` / `Shortcuts`。

**⚠️ 沙箱验证的假阴性**：沙箱里 `powershell -NoProfile` 会话缺 `Microsoft.PowerShell.Utility` / `Security` 模块，`Get-FileHash`、`Get-Acl`、`Test-Path` 会报 `CommandNotFoundException`，导致 `scoop install` **必然在 hash 校验阶段失败**（`ERROR Hash check failed! ... Actual: <空>`）。**这不是 manifest 的问题**，不要因此去改 manifest 或怀疑 hash。认准这个特征：报错里出现 `Get-FileHash : The term ... is not recognized`。

### 4.2 必须交给用户在真实终端执行

```powershell
scoop uninstall patheditor-gui    # 若之前装过
scoop install lhy/patheditor-gui
```

要用户确认两点：**开始菜单出现 `PathEditor` 快捷方式**、**GUI 能启动**。在拿到这个确认之前，**不要声称端到端验证通过**。

---

## ⑤ 提交推送

```bash
git add -A
git commit -m "feat: update patheditor-gui to <V>, ..."   # 或 chore: / fix:
git push origin master
```

推送后核对 `git status -sb` 无 ahead/behind。

---

## ⑥ 汇报本机影响（用户会问这一条，主动讲）

用户明确在意「是否只是推送仓库、有没有改动我本地电脑的配置」。**每次都要把两类改动分开讲**：

| 类别                       | 内容                                            |
| -------------------------- | ----------------------------------------------- |
| **仓库改动**（低风险）     | bucket 的哪些文件、哪个提交、是否已推送         |
| **本机环境改动**（高风险） | 是否安装/卸载过东西、是否有残留、是否有缓存变动 |

若在验证过程中装过东西，**事后必须清理残留**（`apps/<app>/` 空壳目录、`cache/` 失效文件、失败记录），并确认用户的正常安装在清理中未受影响。

### 高危红线

**绝不擅自执行 `scoop cache rm * -f` 之类的通配破坏性命令。** 历史事故：探测校验工具时执行该命令，删掉了用户 **581 MB / 37 个包的下载缓存**（已装程序不受影响，但下次升级要重新下载）。要清理前先用 `scoop cache show` 看范围，或先问用户。

同理，`scoop uninstall` / `Remove-Item -Recurse` 前先确认目标是不是自己造成的残留（**不要误删用户的正常安装**，例如 `shims/patheditor.exe` 虽名为 `patheditor` 但可能是 `patheditor-cli` 的入口，删前先 `cat` 对应的 `.shim` 文件确认指向）。

---

## 越界即停

需要执行 `scoop uninstall`、`Remove-Item -Recurse`、`cache rm` 等改动用户本机环境的操作；或 Release 上找不到符合 CI 命名的产物；或发现 bucket 命名冲突需要改 manifest 名——**停下来先说明影响面并征得同意**。
