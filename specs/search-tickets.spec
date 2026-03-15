spec: task
name: "search-tickets"
inherits: project
tags: [feature, search]
---

## 意图

为 zendcli 添加 `search` 命令。接收关键词参数，通过 Zendesk Search API
查找相似工单并返回结构化 JSON 结果。zendcli 本身不包含 LLM 功能，
关键词由外部 LLM agent 生成后传入。典型调用链：
用户描述问题 → LLM 提取关键词 → 调用 `zend search "关键词"` → 返回相似工单。

## 已定决策

- 命令格式: `zend search "关键词"` 或 `zend search "关键词1 关键词2"`
- 智能 argv 路由: 不做自动路由，必须显式使用 `search` 子命令
- Zendesk 搜索: 使用 Zendesk Search API (`GET /api/v2/search.json?query=type:ticket {keywords}`)
- 搜索范围: 默认搜索 `type:ticket`
- 状态过滤: 可通过 `--status` 参数过滤工单状态（open, pending, solved 等）
- 结果数量: 默认返回 3 条，可通过 `--limit` 控制，上限 10
- 输出格式: JSON 数组，每条包含 `ticket_id`, `subject`, `status`, `created_at`, `description`(截断前200字符), `url`
- 完整描述: 可通过 `--full` 返回完整 description 内容
- 排序: 按相关度排序（Zendesk 默认）
- 错误输出: 与现有命令一致，结构化 JSON 错误到 stdout

## 边界

### 允许修改
- src/main.rs（添加 search 子命令）
- src/api.rs（添加搜索 API 调用）
- tests/integration_test.rs（添加 search 测试）

### 禁止做
- 不要修改现有命令（ticket, email, follower, comments, configure）的行为
- 不要在 zendcli 内嵌入任何 LLM 调用逻辑
- 不要添加 LLM 相关依赖

## 验收标准

场景: 搜索成功返回相似工单
  测试: test_search_returns_tickets
  假设 Zendesk 存在关于 "登录失败" 的历史工单
  当 用户执行 `zend search "登录 失败"`
  那么 CLI 使用关键词调用 Zendesk Search API
  并且 stdout 输出有效 JSON 数组
  并且 每条结果包含 `ticket_id`, `subject`, `status`, `url` 字段

场景: 搜索无结果
  测试: test_search_empty_results
  当 用户执行 `zend search "xyznonexistent123"`
  那么 stdout 输出空 JSON 数组 `[]`
  并且 exit code 为 0

场景: 缺少搜索关键词时报错
  测试: test_search_missing_keyword
  当 用户执行 `zend search` 不带参数
  那么 输出使用帮助信息
  并且 exit code 非 0

场景: 缺少 Zendesk 配置时报错
  测试: test_search_missing_config
  假设 未配置 Zendesk 凭证
  当 用户执行 `zend search "关键词"`
  那么 stderr 输出 "Not configured"
  并且 exit code 非 0

场景: Zendesk API 返回错误
  测试: test_search_api_error
  假设 Zendesk API 返回 401 未授权
  当 用户执行 `zend search "关键词"`
  那么 stdout 输出结构化 JSON 错误
  并且 error 字段为 "api_error"
  并且 exit code 非 0

场景: limit 参数控制结果数量
  测试: test_search_with_limit
  当 用户执行 `zend search "问题" --limit 3`
  那么 返回结果不超过 3 条

场景: limit 超出范围被拒绝
  测试: test_search_invalid_limit
  当 用户执行 `zend search "问题" --limit 20`
  那么 stdout 输出结构化 JSON 错误
  并且 exit code 非 0

场景: status 参数过滤工单状态
  测试: test_search_with_status_filter
  当 用户执行 `zend search "问题" --status open`
  那么 Zendesk 搜索 query 中包含 `status:open`

场景: full 参数返回完整描述
  测试: test_search_with_full_flag
  当 用户执行 `zend search "问题" --full`
  那么 返回结果中 description 字段为完整内容，不截断

场景: help 输出包含 search 命令
  测试: test_help_includes_search
  当 用户执行 `zend --help`
  那么 输出包含 "search"

## 排除范围

- LLM 关键词提取（由外部 agent 负责）
- 搜索结果的排序自定义（使用 Zendesk 默认排序）
- 搜索结果的缓存
- 非工单类型的搜索（用户、组织等）
