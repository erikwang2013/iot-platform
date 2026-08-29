#!/usr/bin/env python3
"""Check key consistency across shared ARB locale files, optionally against HarmonyOS string.json.

Exit codes: 0 = key sets consistent; 1 = missing locale file or key mismatch.
Empty values in non-zh files are warnings only and do not fail the check.

Usage:
  python3 scripts/check_l10n.py
  python3 scripts/check_l10n.py --harmony <resources_dir>...
"""
import argparse
import json
import re
import sys
from pathlib import Path

LOCALES = ["zh", "en", "ko", "ru", "de", "fr", "es", "pt", "hi", "ar", "bn", "id", "ja"]

# HarmonyOS resource sub-dir per locale ("base" = zh)
HARMONY_DIRS = {
    "zh": "base",
    "en": "en_US",
    "ko": "ko_KR",
    "ru": "ru_RU",
    "de": "de_DE",
    "fr": "fr_FR",
    "es": "es_ES",
    "pt": "pt_BR",
    "hi": "hi_IN",
    "ar": "ar_SA",
    "bn": "bn_BD",
    "id": "id_ID",
    "ja": "ja_JP",
}

ARB_DIR = Path(__file__).resolve().parent.parent / "apps" / "shared" / "l10n"


def camel_to_snake(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def arb_keys(path: Path) -> dict:
    """Return key->value of an arb file, skipping "@" metadata keys."""
    with open(path, encoding="utf-8") as f:
        data = json.load(f)
    return {k: v for k, v in data.items() if not k.startswith("@")}


def check_arbs(arb_dir: Path) -> tuple[list[str], list[str]]:
    problems: list[str] = []
    warnings: list[str] = []
    zh_path = arb_dir / "app_zh.arb"
    if not zh_path.exists():
        return [f"missing base file: {zh_path}"], []

    zh = arb_keys(zh_path)
    empty_zh = [k for k, v in zh.items() if not str(v).strip()]
    if empty_zh:
        problems.append(f"app_zh.arb: empty values: {empty_zh}")

    # 13 locale files must exist; no extra/missing files
    seen = set()
    for p in sorted(arb_dir.glob("app_*.arb")):
        loc = p.stem.removeprefix("app_")
        seen.add(loc)
        if loc not in LOCALES:
            problems.append(f"unexpected locale file: {p.name}")
    for loc in LOCALES:
        if loc not in seen:
            problems.append(f"missing locale file: app_{loc}.arb")

    for loc in LOCALES:
        p = arb_dir / f"app_{loc}.arb"
        if not p.exists():
            continue
        keys = arb_keys(p)
        missing = sorted(set(zh) - set(keys))
        extra = sorted(set(keys) - set(zh))
        if missing or extra:
            problems.append(f"{p.name}: missing keys {missing}, extra keys {extra}")
        if loc != "zh":
            empty = [k for k in zh if not str(keys.get(k, "")).strip()]
            if empty:
                warnings.append(f"{p.name}: empty values (warning): {empty}")
    return problems, warnings


def check_harmony(harmony_dir: Path, arb_dir: Path) -> list[str]:
    zh = arb_keys(arb_dir / "app_zh.arb")
    expected = sorted(camel_to_snake(k) for k in zh)
    problems = []
    for loc in LOCALES:
        string_json = harmony_dir / HARMONY_DIRS[loc] / "element" / "string.json"
        if not string_json.exists():
            problems.append(f"{string_json.relative_to(harmony_dir)}: missing")
            continue
        with open(string_json, encoding="utf-8") as f:
            data = json.load(f)
        items = data.get("string", [])
        names = sorted(str(it["name"]) for it in items if "name" in it)
        missing = sorted(set(expected) - set(names))
        extra = sorted(set(names) - set(expected))
        if missing or extra:
            problems.append(
                f"{HARMONY_DIRS[loc]}: missing names {missing}, extra names {extra}"
            )
    return problems


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--harmony", nargs="+", metavar="DIR",
                    help="HarmonyOS resources dir(s) to validate against the arb key set")
    args = ap.parse_args()

    problems, warnings = check_arbs(ARB_DIR)
    for w in warnings:
        print(f"WARNING: {w}")
    for p in problems:
        print(f"ERROR: {p}")

    count = len(list(ARB_DIR.glob("app_*.arb")))
    status = "OK" if not problems else "MISMATCH"
    print(f"{count} arb files checked, key sets vs app_zh.arb: {status}")
    code = 1 if problems else 0

    if args.harmony:
        for d in args.harmony:
            hp = check_harmony(Path(d), ARB_DIR)
            for p in hp:
                print(f"ERROR [{d}]: {p}")
            if hp:
                code = 1
    sys.exit(code)


if __name__ == "__main__":
    main()
