#!/usr/bin/env python3
"""Build scoped FUNG CSS and validate solid color-pair contrast (Python 3.9+)."""
from __future__ import annotations
import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent


def luminance(hex_color: str) -> float:
    value = hex_color.lstrip('#')
    if len(value) != 6:
        raise ValueError(f'Expected a solid #RRGGBB color, received {hex_color!r}')
    rgb = [int(value[i:i+2], 16) / 255.0 for i in (0, 2, 4)]
    linear = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in rgb]
    return sum(v * w for v, w in zip(linear, (0.2126, 0.7152, 0.0722)))


def ratio(fg: str, bg: str) -> float:
    a, b = sorted((luminance(fg), luminance(bg)), reverse=True)
    return (a + 0.05) / (b + 0.05)


def block(selector: str, values: dict[str, str], extra: str = '', indent: str = '') -> str:
    lines = [f'{indent}{selector} {{']
    lines.extend(f'{indent}  --fung-{k}: {v};' for k, v in values.items())
    if extra:
        lines.append(f'{indent}  {extra}')
    lines.append(f'{indent}}}')
    return '\n'.join(lines)


def build(data: dict[str, Any]) -> tuple[str, str, list[dict[str, Any]]]:
    prefix = {f'brand-{k}': v for k, v in data['brand'].items()}
    common = {**prefix, **data['shared']}
    css = [
        '/* FUNG Quiet Archive | DS-FUNG-001 v0.1.0 | PROPOSED */',
        '/* Generated from fung.tokens.json. Re-run build_tokens.py after editing JSON. */',
        '/* Tokens only: no fonts, network imports, component styles, or native window behavior. */',
        '/* Place data-fung-root and data-theme="light|dark|system" on each surface root. */',
        block('[data-fung-root]', common),
        block('[data-fung-root]', data['themes']['light'], 'color-scheme: light;'),
        '@media (prefers-color-scheme: dark) {',
        block('[data-fung-root]:not([data-theme]), [data-fung-root][data-theme="system"]',
              data['themes']['dark'], 'color-scheme: dark;', '  '),
        '}',
        block('[data-fung-root][data-theme="light"]', data['themes']['light'], 'color-scheme: light;'),
        block('[data-fung-root][data-theme="dark"]', data['themes']['dark'], 'color-scheme: dark;'),
        '@media (prefers-reduced-motion: reduce) {',
        block('[data-fung-root]', {f'motion-{k}':'0ms' for k in ['press','hover','expand','panel','maximum']}, indent='  '),
        '}',
        '/* Media-query widths are literal pixels; CSS variables cannot be used in query conditions. */',
        '/* Breakpoints: 640 / 960 / 1200. Existing 760px bootstrap rules are not changed here. */',
    ]
    rows=[]
    for theme, values in data['themes'].items():
        for check in data['contrastChecks']:
            fg=values[check['foreground']]; bg=values[check['background']]
            measured=ratio(fg,bg)
            rows.append({'theme':theme,**check,'fgHex':fg,'bgHex':bg,
                         'ratio':round(measured,4),'pass':measured+1e-9 >= check['minimum']})
    report=[
        '# FUNG — Solid-color contrast report', '',
        'Version: 0.1.0 · Status: token-level check, not a whole-application accessibility certification.', '',
        'Method: sRGB relative luminance; ratio = (lighter + 0.05) / (darker + 0.05).',
        'Project targets: 4.5:1 for listed text pairs; 3:1 for essential outlines/focus against adjacent base surfaces.',
        'Rounded values are displayed; unrounded values decide pass/fail.', '',
        'Not covered: transparency, blur, photography, disabled controls, decorative borders, the logo/wordmark,',
        'font rasterization, high-contrast modes, focus clipping, keyboard use, screen readers, or native overlays.',
        'Do not use brand sage/metal or text-disabled as ordinary body text without another measured pair.', '',
        '| Theme | Foreground token | Background token | Hex pair | Ratio | Target | Result |',
        '| --- | --- | --- | --- | ---: | ---: | --- |',
    ]
    for row in rows:
        report.append(f"| {row['theme']} | `{row['foreground']}` | `{row['background']}` | `{row['fgHex']}` / `{row['bgHex']}` | {row['ratio']:.2f}:1 | {row['minimum']}:1 | {'PASS' if row['pass'] else 'FAIL'} |")
    report += ['',f"Total: {len(rows)} pairs; {sum(r['pass'] for r in rows)} pass; {sum(not r['pass'] for r in rows)} fail.",'']
    return '\n\n'.join(css)+'\n','\n'.join(report),rows


def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true',help='Verify generated files without modifying them.')
    args=parser.parse_args()
    try:
        data=json.loads((ROOT/'fung.tokens.json').read_text(encoding='utf-8'))
        if set(data['themes']['light']) != set(data['themes']['dark']):
            raise ValueError('Light and dark theme keys differ.')
        css,report,rows=build(data)
        outputs={ROOT/'fung.tokens.css':css,ROOT.parent/'qa/CONTRAST_REPORT.md':report}
        if any(not r['pass'] for r in rows):
            for r in rows:
                if not r['pass']:
                    print(f"FAIL: {r['theme']} {r['foreground']} on {r['background']} = {r['ratio']:.3f}",file=sys.stderr)
            return 1
        for dest,content in outputs.items():
            if args.check:
                if not dest.exists() or dest.read_text(encoding='utf-8') != content:
                    raise ValueError(f'Generated file missing/stale: {dest.name}. Run build_tokens.py.')
            else:
                dest.parent.mkdir(parents=True,exist_ok=True)
                dest.write_text(content,encoding='utf-8')
        print(f"{'Checked' if args.check else 'Built'} FUNG tokens; {len(rows)} solid contrast pairs passed.")
        return 0
    except (OSError,ValueError,KeyError,TypeError) as exc:
        print(f'Token build failed: {exc}',file=sys.stderr)
        return 1

if __name__=='__main__':
    raise SystemExit(main())
