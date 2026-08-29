#!/usr/bin/env python3
"""从中文模板生成 12 语言 README 与 SVG 图表。

用法: python3 scripts/gen_i18n_docs.py
输入: README.md, docs/*.zh.svg（中文源模板）
输出: docs/i18n/README.{code}.md, docs/{name}.{code}.svg
"""
import importlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"
I18N = DOCS / "i18n"
SRC_DIR = Path(__file__).resolve().parent / "i18n"

LANGS = [
    ("en", "English"), ("ko", "한국어"), ("ru", "Русский"), ("de", "Deutsch"),
    ("fr", "Français"), ("es", "Español"), ("pt", "Português"), ("hi", "हिन्दी"),
    ("ar", "العربية"), ("bn", "বাংলা"), ("id", "Bahasa Indonesia"), ("ja", "日本語"),
]
NATIVE = {"zh": "中文", **dict(LANGS)}
SVGS = ["architecture", "flow", "features", "lifecycle", "security"]

TEXT_RE = re.compile(r"(<text[^>]*>)(.*?)(</text>)", re.S)
ATTR_RE = re.compile(r'\b(alt|title)="([^"]*)"')
MD_IMG_RE = re.compile(r"!\[([^\]]*)\]\(([^)]*)\)")
BAR_RE = re.compile(r"^\[中文\]\(README\.md\) \| \[English\]\([^)]*\).*$", re.M)
SVG_PATH_RE = re.compile(r"docs/([\w.-]+)\.zh\.svg")
DOCS_PATH_RE = re.compile(r"docs/")

# 图片 alt/title 覆盖表：短键与 SVG <text> 标题共用 dict 键会冲突，故单独定义
ALT_OVERRIDE = {
    "en": {"架构图": "Architecture", "流程图": "Flows", "生命周期图": "Lifecycle", "安全架构": "Security", "微信支付": "WeChat Pay", "支付宝": "Alipay"},
    "ko": {"架构图": "아키텍처", "流程图": "흐름도", "生命周期图": "수명 주기", "安全架构": "보안 아키텍처", "微信支付": "WeChat Pay", "支付宝": "Alipay"},
    "ru": {"架构图": "Архитектура", "流程图": "Диаграмма потока", "生命周期图": "Жизненный цикл", "安全架构": "Безопасность", "微信支付": "WeChat Pay", "支付宝": "Alipay"},
    "de": {"架构图": "Architektur", "流程图": "Flussdiagramm", "生命周期图": "Lebenszyklus", "安全架构": "Sicherheit", "微信支付": "WeChat Pay", "支付宝": "Alipay"},
    "fr": {"架构图": "Architecture", "流程图": "Diagramme de flux", "生命周期图": "Cycle de vie", "安全架构": "Sécurité", "微信支付": "WeChat Pay", "支付宝": "Alipay"},
    "es": {"架构图": "Arquitectura", "流程图": "Diagrama de flujo", "生命周期图": "Ciclo de vida", "安全架构": "Seguridad", "微信支付": "WeChat Pay", "支付宝": "Alipay"},
    "pt": {"架构图": "Arquitetura", "流程图": "Diagrama de fluxo", "生命周期图": "Ciclo de vida", "安全架构": "Segurança", "微信支付": "WeChat Pay", "支付宝": "Alipay"},
    "hi": {"架构图": "आर्किटेक्चर", "流程图": "फ़्लोचार्ट", "生命周期图": "जीवनचक्र", "安全架构": "सुरक्षा", "微信支付": "WeChat Pay", "支付宝": "Alipay"},
    "ar": {"架构图": "البنية المعمارية", "流程图": "مخطط التدفق", "生命周期图": "دورة الحياة", "安全架构": "الأمان", "微信支付": "WeChat Pay", "支付宝": "Alipay"},
    "bn": {"架构图": "আর্কিটেকচার", "流程图": "ফ্লোচার্ট", "生命周期图": "জীবনচক্র", "安全架构": "নিরাপত্তা", "微信支付": "WeChat Pay", "支付宝": "Alipay"},
    "id": {"架构图": "Arsitektur", "流程图": "Diagram Alur", "生命周期图": "Siklus Hidup", "安全架构": "Keamanan", "微信支付": "WeChat Pay", "支付宝": "Alipay"},
    "ja": {"架构图": "アーキテクチャ", "流程图": "フローチャート", "生命周期图": "ライフサイクル", "安全架构": "セキュリティ", "微信支付": "WeChat Pay", "支付宝": "Alipay"},
}


def translate(s, table):
    return table.get(s.strip(), s)


def lang_bar(code):
    parts = []
    for c in ["zh", *[l[0] for l in LANGS]]:
        name = NATIVE[c]
        if c == code:
            target = "README.md" if c == "zh" else f"README.{c}.md"
        elif c == "zh":
            target = "README.md" if code == "zh" else "../../README.md"
        else:
            target = f"docs/i18n/README.{c}.md" if code == "zh" else f"README.{c}.md"
        parts.append(f"[{name}]({target})")
    return " | ".join(parts)


def gen_readme(code, table):
    src = (ROOT / "README.md").read_text(encoding="utf-8")
    text = BAR_RE.sub(lambda m: lang_bar(code), src)
    alt_table = {**table, **ALT_OVERRIDE.get(code, {})}
    text = ATTR_RE.sub(lambda m: f'{m.group(1)}="{translate(m.group(2), alt_table)}"', text)
    text = MD_IMG_RE.sub(
        lambda m: f"![{translate(m.group(1), alt_table)}]({m.group(2)})", text)
    # 整行翻译须在路径重写之前，否则 docs/ 被改写后字典键失配
    text = "\n".join(translate(line, table) for line in text.split("\n"))
    text = SVG_PATH_RE.sub(lambda m: f"../{m.group(1)}.{code}.svg", text)
    text = DOCS_PATH_RE.sub("../", text)
    (I18N / f"README.{code}.md").write_text(text, encoding="utf-8")


def gen_svg(name, code, table):
    src = (DOCS / f"{name}.zh.svg").read_text(encoding="utf-8")
    def repl(m):
        t = translate(m.group(2), table)
        return m.group(1) + t.replace("&", "&amp;") + m.group(3)
    text = TEXT_RE.sub(repl, src)
    (DOCS / f"{name}.{code}.svg").write_text(text, encoding="utf-8")


def main():
    sys.path.insert(0, str(SRC_DIR))
    tables = {code: importlib.import_module(f"zh_{code}").T for code, _ in LANGS}
    missing = [code for code, t in tables.items() if not t]
    if missing:
        sys.exit(f"empty translation dict: {missing}")
    I18N.mkdir(exist_ok=True)
    for code, name in LANGS:
        gen_readme(code, tables[code])
        for svg in SVGS:
            gen_svg(svg, code, tables[code])
        print(f"{name}: README.{code}.md + 5 SVGs")


if __name__ == "__main__":
    main()
