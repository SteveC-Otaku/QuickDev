from PIL import Image, ImageDraw, ImageFont
import json
from pathlib import Path

root = Path.cwd()
version = json.loads((root / 'package.json').read_text(encoding='utf-8'))['version']
scale = 2
canvas = Image.new('RGB', (1200 * scale, 630 * scale), '#0b0b0a')
draw = ImageDraw.Draw(canvas)
def font(size, bold=False):
    return ImageFont.truetype('C:/Windows/Fonts/msyhbd.ttc' if bold else 'C:/Windows/Fonts/msyh.ttc', size * scale)
def text(x, y, value, size, color='#f2efe6', bold=False):
    draw.text((x * scale, y * scale), value, font=font(size, bold), fill=color)
def line(box, fill, width=1):
    draw.line(tuple(v * scale for v in box), fill=fill, width=width * scale)
for x in range(0, 1200, 40):
    line((x, 0, x, 630), '#141413')
for y in range(0, 630, 40):
    line((0, y, 1200, y), '#141413')
draw.rectangle((56*scale, 54*scale, 65*scale, 66*scale), fill='#f0c14a')
text(82, 44, '即栈 · QUICK DEV', 21, bold=True)
text(957, 44, 'v' + version, 21, '#f0c14a', True)
text(56, 134, '多项目 dev，', 68, bold=True)
text(56, 224, '一处即栈。', 68, '#f0c14a', True)
text(60, 340, '前端 / 后端 / 桌面 App，一套方案启动', 27, '#b7b1a3')
line((60, 415, 1140, 415), '#39372e')
items = [('01', '热重载自动隔离', '标准 Uvicorn 入口无需修改脚本'), ('02', '前后端持续运行', '隔离后端信号，保留实时日志'), ('03', '统一停止回收', '后端与重载子进程保持受管')]
for index, (number, title, caption) in enumerate(items):
    x = 60 + index * 367
    text(x, 445, number, 16, '#f0c14a', True)
    text(x, 480, title, 25, bold=True)
    text(x, 526, caption, 16, '#a59f91')
text(60, 590, 'WINDOWS x64 / ARM64  ·  免费开源  ·  本地优先', 13, '#9a9588')
canvas.resize((1200, 630), Image.Resampling.LANCZOS).save(root / 'website/img' / f'release-{version}.png', optimize=True)
