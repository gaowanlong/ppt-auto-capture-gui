# PPTX Office Compatibility Design

## Goal

修复 PPT Auto Capture 生成的 PPTX 在 Microsoft PowerPoint 打开时提示需要修复、修复后继续提示删除无法读取内容的问题，同时建立能够阻止相同缺陷回归的自动化测试。

## Scope

本次修改只处理 PPTX 生成器的 OOXML 包结构和兼容模板，不改变：

- 截屏行为和截屏图片编码；
- 每张截屏对应一页幻灯片的模型；
- 页面比例和图片适配逻辑；
- Windows 与 macOS 的捕获后端；
- 现有输出目录和文件命名规则。

PowerPoint 修复后的用户样本作为兼容结构基线：

- `/Users/allen/Documents/PPT Auto Capture/ppt-capture-20260731-174405  -  已修复.pptx`

测试断言 OOXML 行为和结构，不断言 ZIP 字节、XML 空白、节点排序或 PowerPoint 写入的非必要元数据。

## Root Cause

当前生成器同时存在三类结构缺陷：

1. `ppt/presProps.xml`、`ppt/viewProps.xml`、`ppt/theme/theme1.xml` 和 `ppt/tableStyles.xml` 被写入 ZIP，但未由 `ppt/_rels/presentation.xml.rels` 引用，形成孤立部件。
2. `ppt/viewProps.xml` 中 `p:restoredLeft` 和 `p:restoredTop` 使用了无效的 `cx`、`cy` 属性，而 PowerPoint 期望 `sz`。
3. 当前 Theme、Slide Master 和 Presentation 默认结构被过度精简。Theme 字体集合及样式矩阵不完整，PowerPoint 打开时会重写这些结构。

现有测试只验证部件存在、XML 可解析和少量关系存在，无法验证完整 OPC 关系图或 OOXML Schema 关键约束。

## Chosen Approach

采用“完整兼容模板 + 动态关系生成”：

- 从 PowerPoint 修复后的样本提取并整理稳定的 Theme、Slide Master 和 Presentation 默认结构；
- 保留当前动态生成幻灯片、媒体和页面尺寸的代码；
- 动态生成所有 Presentation 级关系，关系 ID 必须随幻灯片数量变化且保持唯一；
- 修正 View Properties 的属性；
- 不引入新的 PPTX 生成依赖。

不采用只补关系的最小修复，因为它无法消除精简 Theme 带来的后续兼容风险；不引入第三方库，因为这会扩大重构和发行风险。

## Package Structure

生成器必须写入并关联以下 Presentation 级部件：

| Relationship type | Target |
| --- | --- |
| `slideMaster` | `slideMasters/slideMaster1.xml` |
| `slide` | `slides/slideN.xml`，每页一个 |
| `presProps` | `presProps.xml` |
| `viewProps` | `viewProps.xml` |
| `theme` | `theme/theme1.xml` |
| `tableStyles` | `tableStyles.xml` |

关系 ID 可以是任意合法且唯一的 XML ID。实现应在幻灯片关系之后动态分配支持部件关系 ID，避免固定的 `rId5` 等 ID 在四页以上时冲突。

每个内部关系目标必须解析到 ZIP 中存在的部件；外部关系不在本次范围内。

## XML Compatibility Baseline

### View Properties

`p:normalViewPr` 中使用：

```xml
<p:restoredLeft sz="15611"/>
<p:restoredTop sz="94660"/>
```

不得重新出现 `cx` 或 `cy`。

### Theme

Theme 至少包含：

- 完整的颜色方案；
- `majorFont` 和 `minorFont` 中的 `latin`、`ea`、`cs`；
- PowerPoint 兼容的填充、线条、效果和背景填充样式矩阵；
- 合法的 `themeElements` 节点顺序。

### Presentation and Slide Master

Presentation 保留动态页面尺寸，并增加与修复后样本一致的默认文本样式。16:9 页面设置相应的 `type="screen16x9"`；其他受支持比例不写入错误的类型值。

Slide Master 使用完整的背景、颜色映射、布局列表和文本样式。现有空白布局及其到母版的关系保持不变。

## Error Handling

PPTX 生成继续使用现有错误返回机制。模板为编译期常量，因此不增加运行时模板读取失败路径。

测试中的关系图验证器遇到以下情况必须失败并给出具体部件：

- 重复关系 ID；
- 内部目标不存在；
- 必需关系缺失；
- 关系目标越出 PPTX 包根目录；
- XML 无法解析。

## Test Strategy

严格按测试驱动方式实施，每项生产修改前先观察对应测试失败。

### Unit Tests

- Presentation 关系包含四个支持部件。
- 0、1、3、100 页情况下所有关系 ID 唯一。
- `viewProps` 使用 `sz`，且不存在 `cx`、`cy`。
- Theme 包含完整字体集合和至少满足兼容基线的样式矩阵。
- Presentation 和 Slide Master 包含兼容性所需的默认结构。

### Package Integration Tests

通过真实 `PptxWriter` 生成 PPTX 后验证：

- ZIP 完整且所有预期部件存在；
- 所有 XML 均可解析；
- 从每个 `.rels` 文件解析出的内部 Target 都对应实际 ZIP 部件；
- 每页 Slide、Slide Relationship 和媒体文件一一对应；
- 输入 PNG 字节在 PPTX 媒体部件中保持一致；
- 多页生成不会造成关系 ID 冲突。

### Regression Fixture

用户修复后样本只用于确定兼容结构，不直接加入仓库，也不做整文件 golden comparison，避免泄露用户内容并避免 PowerPoint 版本导致的无意义字节差异。

## Success Criteria

- 新增测试在旧实现上以预期原因失败；
- 修复后新增测试和现有测试全部通过；
- `cargo fmt --check`、`cargo clippy` 和完整测试套件通过；
- 新生成 PPTX 的关系图完整、关键 XML 结构符合兼容基线；
- Windows 和 macOS 截屏及图片嵌入行为不发生回归；
- 修改提交并推送到当前 GitHub 分支 `codex/macos-native-capture`。

