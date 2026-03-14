spec: task
name: "refactor-by-rust"
inherits: project
tags: [rewrite, parity, rust]
---

## 意图

将 zendcli（当前为 TypeScript/Node.js 实现）完整重写为 Rust，在同一仓库内进行。
所有开发在 `refactor-rust` 分支上完成，不破坏 main 分支。目标是 branch 上的 Rust 实现完全通过测试，
行为与 main 分支的 TS 版完全一致。二进制名保持 `zend`，配置文件路径保持 `~/.zendcli/config.json`，
所有命令的 JSON 输出格式与 TS 版完全一致，确保现有 agent/脚本无缝迁移。
验证通过后通过 PR 合并到 main，再通过 GitHub Actions workflow 触发发版。

## 已定决策

- 分支策略: 在 `refactor-rust` 分支上进行所有重构工作，不直接修改 main 分支；branch 上必须完全通过测试且行为与 main 一致后，才能通过 PR 合并
- 语言: Rust，在当前 repo 根目录重构，替换 TS 源码
- 二进制名: `zend`，与 TS 版相同
- 配置路径: `~/.zendcli/config.json`，格式与 TS 版完全兼容
- 配置优先级: 环境变量 (`ZENDESK_SUBDOMAIN`, `ZENDESK_EMAIL`, `ZENDESK_API_TOKEN`) > 文件配置
- 文件权限: 目录 0700, 配置文件 0600
- 认证: HTTP Basic Auth, `{email}/token:{api_token}` Base64 编码
- CLI 框架: `clap`
- HTTP 客户端: `reqwest` (异步, tokio 运行时)
- JSON: `serde` + `serde_json`
- 智能 argv 路由: 数字参数自动路由到 ticket 命令，邮箱格式自动路由到 email 命令
- 输出: JSON-only，错误也输出结构化 JSON 到 stdout
- 发布: GitHub Actions workflow 触发 npm 发版，与当前 CI/CD 流程一致
- [JS-only] 不适用: 原 TS 版使用 `esbuild` 打包和 `commander.js`，Rust 版不需要这些

## 边界

### 允许修改
- src/**
- Cargo.toml
- Cargo.lock
- .github/workflows/**
- package.json (适配 npm 发版)
- tsconfig.json (移除)
- dist/** (移除)

### 禁止做
- 不要直接在 main 分支上修改，所有改动必须在 `refactor-rust` 分支进行
- 不要修改 `~/.zendcli/config.json` 的格式，必须与 TS 版互相兼容
- 不要改变任何命令的 JSON 输出结构
- 不要引入写操作（创建/修改/删除工单）
- 不要移除 `skill/SKILL.md`

## 完成条件

场景: 单工单查询返回精简 JSON
  测试:
    包: zendcli
    过滤: test_ticket_get_slim_output
    层级: integration
    替身: local_http_stub
    命中: src/cli.rs, src/api.rs
  假设 Zendesk API 返回工单 "12345" 的完整数据
  当 用户执行 `zend 12345`
  那么 stdout 输出精简 JSON，包含 id、subject、description、status、priority、tags、assignee_id、requester_id
  并且 不包含完整 API 响应中的冗余字段

场景: 单工单查询 --raw 返回完整响应
  测试:
    包: zendcli
    过滤: test_ticket_get_raw_output
    层级: integration
    替身: local_http_stub
    命中: src/cli.rs, src/api.rs
  假设 Zendesk API 返回工单 "12345" 的完整数据
  当 用户执行 `zend 12345 --raw`
  那么 stdout 输出完整 API 响应 JSON

场景: 邮箱参数自动路由到 assignee 查询
  测试:
    包: zendcli
    过滤: test_email_argv_routing
    层级: cli
    替身: local_http_stub
    命中: src/cli.rs
  假设 Zendesk API 返回 assignee 为 "user@example.com" 的工单列表
  当 用户执行 `zend user@example.com`
  那么 等价于 `zend email user@example.com`
  并且 stdout 输出工单列表 JSON

场景: 数字参数自动路由到 ticket 查询
  测试:
    包: zendcli
    过滤: test_numeric_argv_routing
    层级: cli
    替身: local_http_stub
    命中: src/cli.rs
  假设 Zendesk API 返回工单 "99999"
  当 用户执行 `zend 99999`
  那么 等价于 `zend ticket 99999`

场景: assignee 工单搜索支持 status 和 limit 过滤
  测试:
    包: zendcli
    过滤: test_email_search_with_filters
    层级: integration
    替身: local_http_stub
    命中: src/cli.rs, src/api.rs
  假设 Zendesk API 对 assignee 搜索返回多条结果
  当 用户执行 `zend user@example.com --status open --limit 10 --sort asc`
  那么 请求包含正确的 ZQL 查询参数
  并且 stdout 输出过滤后的工单列表 JSON

场景: follower 查询排除 assignee
  测试:
    包: zendcli
    过滤: test_follower_excludes_assignee
    层级: integration
    替身: local_http_stub
    命中: src/cli.rs, src/api.rs
  假设 用户 "user@example.com" 同时是工单 A 的 follower 和工单 B 的 assignee+follower
  当 用户执行 `zend follower user@example.com`
  那么 结果包含工单 A
  但是 不包含工单 B

场景: comments 输出精简格式
  测试:
    包: zendcli
    过滤: test_comments_slim_output
    层级: integration
    替身: local_http_stub
    命中: src/cli.rs, src/api.rs
  假设 工单 "12345" 有 3 条评论（2 条 public, 1 条 private）
  当 用户执行 `zend comments 12345`
  那么 stdout 输出 3 条评论，每条仅包含 author、time、visibility、body 四个字段

场景: comments visibility 过滤
  测试:
    包: zendcli
    过滤: test_comments_visibility_filter
    层级: integration
    替身: local_http_stub
    命中: src/cli.rs
  假设 工单 "12345" 有 public 和 private 评论
  当 用户执行 `zend comments 12345 --visibility public`
  那么 stdout 仅包含 public 评论

场景: configure 交互式配置写入
  测试:
    包: zendcli
    过滤: test_configure_writes_config
    层级: integration
    替身: temp_home_dir
    命中: src/config.rs
  假设 `~/.zendcli/config.json` 不存在
  当 用户执行 `zend configure` 并输入 subdomain、email、api_token
  那么 `~/.zendcli/config.json` 被创建，包含输入的三个字段
  并且 文件权限为 0600

场景: 环境变量覆盖文件配置
  测试:
    包: zendcli
    过滤: test_env_overrides_file_config
    层级: unit
    命中: src/config.rs
  假设 `~/.zendcli/config.json` 中 subdomain 为 "file-sub"
  并且 环境变量 `ZENDESK_SUBDOMAIN` 设置为 "env-sub"
  当 加载配置
  那么 subdomain 的值为 "env-sub"

场景: 缺少认证配置时报错
  测试:
    包: zendcli
    过滤: test_missing_config_error
    层级: cli
    替身: temp_home_dir
    命中: src/config.rs, src/cli.rs
  假设 `~/.zendcli/config.json` 不存在且环境变量未设置
  当 用户执行 `zend 12345`
  那么 stdout 输出结构化错误 JSON，包含配置缺失的提示
  并且 进程退出码非 0

场景: API 请求失败返回结构化错误
  测试:
    包: zendcli
    过滤: test_api_error_structured_output
    层级: integration
    替身: local_http_stub
    命中: src/api.rs, src/cli.rs
  假设 Zendesk API 返回 401 Unauthorized
  当 用户执行 `zend 12345`
  那么 stdout 输出结构化错误 JSON，包含 error 和 message 字段

场景: 工单不存在返回结构化错误
  测试:
    包: zendcli
    过滤: test_ticket_not_found_error
    层级: integration
    替身: local_http_stub
    命中: src/api.rs, src/cli.rs
  假设 Zendesk API 返回 404 Not Found
  当 用户执行 `zend 99999`
  那么 stdout 输出结构化错误 JSON

场景: refactor-rust 分支测试全部通过
  测试:
    包: zendcli
    过滤: test_branch_all_tests_pass
    层级: ci
    命中: .github/workflows/**
  假设 `refactor-rust` 分支已完成所有 Rust 代码编写
  当 在 `refactor-rust` 分支上运行 `cargo test`
  那么 所有测试通过，退出码为 0

场景: refactor-rust 分支行为与 main 一致
  测试:
    包: zendcli
    过滤: test_branch_parity_with_main
    层级: integration
    替身: local_http_stub
    命中: src/cli.rs, src/api.rs, src/config.rs
  假设 对同一组 Zendesk API mock 数据
  当 分别在 main 分支（TS 版）和 `refactor-rust` 分支（Rust 版）执行相同命令
  那么 两个分支的 stdout JSON 输出完全一致

## 排除范围

- 写操作（创建/修改/删除工单、评论）
- OAuth 认证方式
- 非 JSON 输出格式（human-readable 表格等）
- MCP server 功能
- Windows 平台支持
