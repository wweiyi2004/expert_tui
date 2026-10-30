# 终端宠物包

启动页宠物可以替换。让其他 agent 制作新宠物时，可以直接把 `PET-AUTHORING-PROMPT.md` 交给它。

一个宠物包就是一个 JSON 文件，包含调色板、像素帧、待机循环、点击动作和台词。惠惠（`megumin.json`）是内置的默认包。

## 使用

- 自定义包放在配置目录的 `pets/<名称>.json`（与 `tui.json` 同级，例如 `%APPDATA%/expert-chat/pets/cat.json`）。
- `/pet cat` 切换并写入配置；`/pet list` 列出可用包；`/pet off` / `/pet on` 关闭或恢复当前包。
- 配置 `pet` 也可以直接写一个 `.json` 路径。包不合法时会提示原因并改用惠惠。

## 格式

```json
{
 "name": "小猫",
 "palette": {"o": "#30242e", "y": "#f7dc7a"},
 "frames": {
  "sit":  ["..oo..", ".oyyo.", "oyyyyo"],
  "wave": ["o.oo..", ".oyyo.", "oyyyyo"]
 },
 "idle": {"frames": [["sit", 900]], "blink": null},
 "clicks": ["hello"],
 "actions": [
  {"id": "hello", "frames": [["wave", 200], ["sit", 200]], "duration": 1200,
   "quotes": [[0, "「喵。」"]]}
 ],
 "quote_ms": 2500
}
```

| 字段 | 说明 |
|---|---|
| `palette` | 单个 ASCII 字符 → `#RRGGBB`。`.` 固定表示透明，不能作键 |
| `frames` | 帧名 → 像素行。所有帧尺寸相同（1–256 像素），只能用 `.` 和调色板键。每个终端字符格画上下两个像素，宽高比约 4:5 时显示最好（惠惠是 64×80） |
| `idle.frames` | `[帧名, 毫秒]` 循环播放；第一帧决定尺寸，减少动态效果时只显示第一帧 |
| `idle.blink` | 可选，随机眨眼帧（每 3–6 秒一次，停 150ms） |
| `clicks` | 每次点击依次启动的动作 id，循环使用 |
| `actions[].frames` | `[帧名, 毫秒]`。默认播放一遍，停在最后一帧直到结束 |
| `actions[].duration` | 可选，循环播放这些帧直到该毫秒数 |
| `actions[].hold` | 为 `true` 时停在动作上，直到再次点击才回到待机 |
| `actions[].next` | 可选，结束后接着播放的动作 |
| `actions[].reduced` | 可选，减少动态效果时改播的动作 |
| `actions[].quotes` | `[开始毫秒, 台词]`，显示最近开始的一句 |
| `actions[].release_quote` | 可选，`hold` 动作被点击解除时说的话 |
| `quote_ms` | 台词从动作开始起显示多久，默认 2500 |

动作播放中（`hold` 除外）点击无效。

## 惠惠

`megumin-approved-poses.png` 是用户确认的六姿态设计稿。`megumin.json` 由 `python tool/build_simple_megumin.py` 生成：在 32×40 网格上手工摆放帽子、脸、披风、裙子、法杖等部件，再放大 2 倍成 64×80，并写入动作与台词。共 11 帧：待机、呼吸、眨眼、自我介绍（睁眼 / 得意闭眼）、举杖咏唱 ×2、施法起手、爆炸、烟尘、力竭倒地。调色板在原 8 色平涂的基础上加了火焰橙、火光黄和烟灰三种效果色。请修改脚本后重新生成，不要直接手改 JSON。

`megumin-detailed.json` 保留较复杂的版本；`tool/import_megumin_sprite.py` 可由确认稿重建该版本，勿覆盖当前版本。

`megumin-legacy.json` 保留替换前的数据供回溯。
