#!/usr/bin/env python3
"""Validate FUNG's DTCG token source and build CSS and contrast artifacts."""
from __future__ import annotations

import argparse
import json
import math
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent
EXTENSION_KEY = 'com.fung.design-system'
SUPPORTED_TYPES = {'color', 'dimension', 'fontFamily', 'fontWeight', 'duration', 'cubicBezier', 'number', 'shadow'}


def fail(message: str) -> None:
    raise ValueError(message)


def validate_dimension(value: Any, path: str) -> None:
    if not isinstance(value, dict) or set(value) != {'value', 'unit'}:
        fail(f'{path}: dimension must contain value and unit.')
    if not isinstance(value['value'], (int, float)) or not math.isfinite(value['value']):
        fail(f'{path}: dimension value must be finite.')
    if value['unit'] not in {'px', 'rem'}:
        fail(f'{path}: DTCG dimension unit must be px or rem.')


def validate_color(value: Any, path: str) -> None:
    if not isinstance(value, dict) or value.get('colorSpace') != 'srgb':
        fail(f'{path}: color must use the sRGB color object format.')
    components = value.get('components')
    if not isinstance(components, list) or len(components) != 3:
        fail(f'{path}: sRGB color needs exactly three components.')
    if any(not isinstance(channel, (int, float)) or not 0 <= channel <= 1 for channel in components):
        fail(f'{path}: color components must be between 0 and 1.')
    alpha = value.get('alpha', 1)
    if not isinstance(alpha, (int, float)) or not 0 <= alpha <= 1:
        fail(f'{path}: alpha must be between 0 and 1.')
    if 'hex' in value and not re.fullmatch(r'#[0-9A-Fa-f]{6}', str(value['hex'])):
        fail(f'{path}: optional hex must be #RRGGBB.')


def validate_shadow(value: Any, path: str) -> None:
    shadows = value if isinstance(value, list) else [value]
    if not shadows:
        fail(f'{path}: shadow array cannot be empty.')
    for index, shadow in enumerate(shadows):
        item_path = f'{path}[{index}]' if isinstance(value, list) else path
        if not isinstance(shadow, dict):
            fail(f'{item_path}: shadow must be an object.')
        required = {'color', 'offsetX', 'offsetY', 'blur', 'spread'}
        if not required.issubset(shadow):
            fail(f'{item_path}: shadow is missing {sorted(required - set(shadow))}.')
        validate_color(shadow['color'], f'{item_path}.color')
        for part in ('offsetX', 'offsetY', 'blur', 'spread'):
            validate_dimension(shadow[part], f'{item_path}.{part}')
        if 'inset' in shadow and not isinstance(shadow['inset'], bool):
            fail(f'{item_path}.inset must be boolean.')


def validate_value(token_type: str, value: Any, path: str) -> None:
    if token_type == 'color':
        validate_color(value, path)
    elif token_type == 'dimension':
        validate_dimension(value, path)
    elif token_type == 'fontFamily':
        if not ((isinstance(value, str) and value.strip()) or
                (isinstance(value, list) and value and all(isinstance(item, str) and item.strip() for item in value))):
            fail(f'{path}: fontFamily must be a font name or non-empty array of names.')
    elif token_type == 'fontWeight':
        aliases = {'thin', 'hairline', 'extra-light', 'ultra-light', 'light', 'normal', 'regular', 'book',
                   'medium', 'semi-bold', 'demi-bold', 'bold', 'extra-bold', 'ultra-bold', 'black', 'heavy',
                   'extra-black', 'ultra-black'}
        if not ((isinstance(value, (int, float)) and 1 <= value <= 1000) or (isinstance(value, str) and value in aliases)):
            fail(f'{path}: fontWeight must be a number from 1 to 1000 or a standard alias.')
    elif token_type == 'duration':
        if not isinstance(value, dict) or set(value) != {'value', 'unit'} or value.get('unit') not in {'ms', 's'}:
            fail(f'{path}: duration must contain value and an ms or s unit.')
        if not isinstance(value['value'], (int, float)) or not math.isfinite(value['value']):
            fail(f'{path}: duration value must be finite.')
    elif token_type == 'cubicBezier':
        if not isinstance(value, list) or len(value) != 4 or any(not isinstance(item, (int, float)) for item in value):
            fail(f'{path}: cubicBezier must contain four numbers.')
        if not 0 <= value[0] <= 1 or not 0 <= value[2] <= 1:
            fail(f'{path}: cubicBezier x control points must be within [0, 1].')
    elif token_type == 'number':
        if not isinstance(value, (int, float)) or not math.isfinite(value):
            fail(f'{path}: number token must be finite.')
    elif token_type == 'shadow':
        validate_shadow(value, path)


def collect_tokens(node: Any, path: tuple[str, ...] = (), inherited_type: str | None = None,
                   tokens: dict[str, dict[str, Any]] | None = None) -> dict[str, dict[str, Any]]:
    if tokens is None:
        tokens = {}
    if not isinstance(node, dict):
        fail(f'{".".join(path) or "root"}: group must be an object.')
    token_type = node.get('$type', inherited_type)
    if '$value' in node:
        allowed = {'$type', '$description', '$value', '$extensions', '$deprecated'}
        unexpected = set(node) - allowed
        if unexpected:
            fail(f'{".".join(path)}: unexpected token properties {sorted(unexpected)}.')
        if not isinstance(token_type, str) or token_type not in SUPPORTED_TYPES:
            fail(f'{".".join(path)}: missing or unsupported DTCG $type {token_type!r}.')
        if not isinstance(node.get('$description'), str) or not node['$description'].strip():
            fail(f'{".".join(path)}: token must have a non-empty $description.')
        extensions = node.get('$extensions', {})
        if not isinstance(extensions, dict):
            fail(f'{".".join(path)}: $extensions must be an object.')
        validate_value(token_type, node['$value'], '.'.join(path))
        tokens['.'.join(path)] = {'path': path, 'type': token_type, 'value': node['$value'], 'extensions': extensions}
        return tokens

    allowed_group = {'$type', '$description', '$extensions', '$deprecated'}
    unexpected = {key for key in node if key.startswith('$')} - allowed_group
    if unexpected:
        fail(f'{".".join(path) or "root"}: unexpected group properties {sorted(unexpected)}.')
    for name, child in node.items():
        if name.startswith('$'):
            continue
        if not name or any(char in name for char in '{}.'):
            fail(f'{".".join(path + (name,))}: token/group name contains a DTCG-reserved character.')
        collect_tokens(child, path + (name,), token_type, tokens)
    return tokens


def luminance(hex_color: str) -> float:
    value = hex_color.lstrip('#')
    if len(value) != 6:
        fail(f'Expected an opaque #RRGGBB color for contrast, received {hex_color!r}.')
    rgb = [int(value[i:i + 2], 16) / 255.0 for i in (0, 2, 4)]
    linear = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in rgb]
    return sum(v * w for v, w in zip(linear, (0.2126, 0.7152, 0.0722)))


def ratio(foreground: str, background: str) -> float:
    a, b = sorted((luminance(foreground), luminance(background)), reverse=True)
    return (a + 0.05) / (b + 0.05)


def number_text(value: float) -> str:
    return str(int(value)) if float(value).is_integer() else f'{value:g}'


def color_css(value: dict[str, Any]) -> str:
    if value.get('alpha', 1) == 1:
        if 'hex' in value:
            return value['hex'].upper()
        return '#' + ''.join(f'{round(channel * 255):02X}' for channel in value['components'])
    channels = ', '.join(str(round(channel * 255)) for channel in value['components'])
    return f"rgba({channels}, {number_text(value['alpha'])})"


def dimension_css(value: dict[str, Any]) -> str:
    return f"{number_text(value['value'])}{value['unit']}"


def shadow_css(value: Any) -> str:
    shadows = value if isinstance(value, list) else [value]
    rendered = []
    for shadow in shadows:
        prefix = 'inset ' if shadow.get('inset', False) else ''
        rendered.append(prefix + ' '.join([
            dimension_css(shadow['offsetX']), dimension_css(shadow['offsetY']),
            dimension_css(shadow['blur']), dimension_css(shadow['spread']), color_css(shadow['color']),
        ]))
    return ', '.join(rendered)


def css_value(token: dict[str, Any]) -> str:
    value, token_type = token['value'], token['type']
    if token_type == 'color':
        return color_css(value)
    if token_type == 'dimension':
        return dimension_css(value)
    if token_type == 'fontFamily':
        families = [value] if isinstance(value, str) else value
        return ', '.join(f'"{item}"' if any(char.isspace() for char in item) else item for item in families)
    if token_type == 'fontWeight':
        return str(value)
    if token_type == 'duration':
        return dimension_css(value)
    if token_type == 'cubicBezier':
        return f"cubic-bezier({', '.join(number_text(item) for item in value)})"
    if token_type == 'number':
        extension = token['extensions'].get(EXTENSION_KEY, {})
        unit = extension.get('cssUnit', '') if isinstance(extension, dict) else ''
        return number_text(value) + unit
    if token_type == 'shadow':
        return shadow_css(value)
    fail(f"Unsupported CSS type {token_type!r}.")


def css_name(token: dict[str, Any]) -> str:
    path = token['path']
    if path[0] == 'brand':
        return 'brand-' + '-'.join(path[1:])
    if path[0] == 'shared':
        return '-'.join(path[1:])
    if path[0] == 'themes':
        return '-'.join(path[2:])
    fail(f'No CSS namespace mapping for {".".join(path)}.')


def block(selector: str, values: dict[str, str], extra: str = '', indent: str = '') -> str:
    lines = [f'{indent}{selector} {{']
    lines.extend(f'{indent}  --fung-{key}: {value};' for key, value in values.items())
    if extra:
        lines.append(f'{indent}  {extra}')
    lines.append(f'{indent}}}')
    return '\n'.join(lines)


def brand_kit_css(tokens: dict[str, dict[str, Any]]) -> str:
    brand = {key: css_value(token) for path, token in tokens.items() if token['path'][0] == 'brand' for key in [token['path'][1]]}
    shared = {token['path'][1]: css_value(token) for token in tokens.values() if token['path'][0] == 'shared' and len(token['path']) == 2}
    legacy = {
        'porcelain': brand['porcelain'], 'ink': brand['ink'], 'sage': brand['sage'], 'slate': brand['slate'],
        'metal': brand['metal'], 'clay': brand['clay'],
        'serif': shared['font-display'], 'thai': shared['font-ui'], 'sans': shared['font-latin'], 'mono': shared['font-mono'],
        'wordmark-tracking': shared['wordmark-tracking'],
    }
    lines = [
        '/* Generated from docs/FUNG_Design_System_v0.1.0/tokens/fung.tokens.json. Do not edit by hand. */',
        ':root {',
        f"  --fung-porcelain: {legacy['porcelain']};",
        f"  --fung-ink: {legacy['ink']};",
        f"  --fung-sage: {legacy['sage']};",
        f"  --fung-slate: {legacy['slate']};",
        f"  --fung-metal: {legacy['metal']};",
        f"  --fung-clay: {legacy['clay']};",
        '  --fung-mark-fill: currentColor;',
        f"  --fung-serif: {legacy['serif']};",
        f"  --fung-thai: {legacy['thai']};",
        f"  --fung-sans: {legacy['sans']};",
        f"  --fung-mono: {legacy['mono']};",
        f"  --fung-wordmark-tracking: {legacy['wordmark-tracking']};",
    ]
    for alias, source in [('1', 'space-4'), ('2', 'space-8'), ('3', 'space-12'), ('4', 'space-16'), ('6', 'space-24'), ('8', 'space-32')]:
        lines.append(f"  --fung-space-{alias}: {shared[source]};")
    for alias, source in [('sm', 'radius-sm'), ('md', 'radius-md'), ('lg', 'radius-lg'), ('pill', 'radius-pill')]:
        lines.append(f"  --fung-radius-{alias}: {shared[source]};")
    lines.append('}')
    return '\n'.join(lines) + '\n'


def build(data: dict[str, Any]) -> tuple[str, str, str, list[dict[str, Any]]]:
    tokens = collect_tokens(data)
    extension = data.get('$extensions', {}).get(EXTENSION_KEY)
    if not isinstance(extension, dict):
        fail(f'Root $extensions must contain {EXTENSION_KEY!r}.')
    for required in ('metadata', 'contrastChecks', 'breakpoints', 'board'):
        if required not in extension:
            fail(f'{EXTENSION_KEY} extension is missing {required}.')
    if not isinstance(extension['contrastChecks'], list) or not extension['contrastChecks']:
        fail('contrastChecks must be a non-empty array.')
    if set(data.get('themes', {})) != {'light', 'dark'}:
        fail('FUNG requires light and dark theme groups.')

    names = [css_name(token) for token in tokens.values()
             if token['path'][0] != 'themes' or token['path'][1] == 'light']
    duplicate = sorted({name for name in names if names.count(name) > 1})
    if duplicate:
        fail(f'CSS variable mapping collision: {duplicate}.')

    common = {css_name(token): css_value(token) for token in tokens.values() if token['path'][0] in {'brand', 'shared'}}
    theme_values = {
        theme: {css_name(token): css_value(token) for token in tokens.values() if token['path'][0] == 'themes' and token['path'][1] == theme}
        for theme in ('light', 'dark')
    }
    css = [
        f"/* FUNG Quiet Archive | DS-FUNG-001 v{extension['metadata']['version']} | BETA */",
        '/* Generated from the DTCG 2025.10 token source. Re-run build_tokens.py after editing JSON. */',
        '/* Tokens only: no font files, network imports, component styles, or native window behavior. */',
        '/* Place data-fung-root and data-theme="light|dark|system" on each surface root. */',
        block('[data-fung-root]', common),
        block('[data-fung-root]', theme_values['light'], 'color-scheme: light;'),
        '@media (prefers-color-scheme: dark) {',
        block('[data-fung-root]:not([data-theme]), [data-fung-root][data-theme="system"]', theme_values['dark'], 'color-scheme: dark;', '  '),
        '}',
        block('[data-fung-root][data-theme="light"]', theme_values['light'], 'color-scheme: light;'),
        block('[data-fung-root][data-theme="dark"]', theme_values['dark'], 'color-scheme: dark;'),
        '@media (prefers-reduced-motion: reduce) {',
        block('[data-fung-root]', {f'motion-{key}': '0ms' for key in ['press', 'hover', 'expand', 'panel', 'maximum']}, indent='  '),
        '}',
        '/* Media-query widths are literal pixels; CSS variables cannot be used in query conditions. */',
        '/* Breakpoints: ' + ' / '.join(str(v) for k, v in extension['breakpoints'].items() if k != 'existingBootstrapCoarsePointer') +
        '. Existing ' + str(extension['breakpoints'].get('existingBootstrapCoarsePointer')) + 'px bootstrap rules are not changed here. */',
    ]

    rows = []
    for theme, values in data['themes'].items():
        for check in extension['contrastChecks']:
            try:
                foreground = values[check['foreground']]['$value']
                background = values[check['background']]['$value']
            except (KeyError, TypeError):
                fail(f"Contrast pair references missing theme tokens: {theme} {check!r}.")
            if not isinstance(foreground, dict) or not isinstance(background, dict):
                fail(f"Contrast pair must reference direct color tokens: {theme} {check!r}.")
            if foreground.get('alpha', 1) != 1 or background.get('alpha', 1) != 1:
                fail(f"Contrast pair must use opaque colors: {theme} {check!r}.")
            fg, bg = color_css(foreground), color_css(background)
            measured = ratio(fg, bg)
            rows.append({'theme': theme, **check, 'fgHex': fg, 'bgHex': bg, 'ratio': round(measured, 4),
                         'pass': measured + 1e-9 >= check['minimum']})
    report = [
        '# FUNG — Solid-color contrast report', '',
        f"Version: {extension['metadata']['version']} · Status: token-level check, not a whole-application accessibility certification.", '',
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
    report += ['', f"Total: {len(rows)} pairs; {sum(row['pass'] for row in rows)} pass; {sum(not row['pass'] for row in rows)} fail.", '']
    return '\n\n'.join(css) + '\n', '\n'.join(report), brand_kit_css(tokens), rows


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='Verify generated files without modifying them.')
    args = parser.parse_args()
    try:
        data = json.loads((ROOT / 'fung.tokens.json').read_text(encoding='utf-8'))
        css, report, brand_css, rows = build(data)
        if any(not row['pass'] for row in rows):
            for row in rows:
                if not row['pass']:
                    print(f"FAIL: {row['theme']} {row['foreground']} on {row['background']} = {row['ratio']:.3f}", file=sys.stderr)
            return 1
        outputs = {
            ROOT / 'fung.tokens.css': css,
            ROOT.parent / 'qa' / 'CONTRAST_REPORT.md': report,
            ROOT.parents[1] / 'brand-kit' / 'tokens.css': brand_css,
        }
        for destination, content in outputs.items():
            if args.check:
                if not destination.exists() or destination.read_text(encoding='utf-8') != content:
                    fail(f'Generated file missing/stale: {destination.relative_to(ROOT.parents[2])}. Run build_tokens.py.')
            else:
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_text(content, encoding='utf-8')
        print(f"{'Checked' if args.check else 'Built'} FUNG DTCG tokens; {len(collect_tokens(data))} tokens and {len(rows)} solid contrast pairs passed.")
        return 0
    except (OSError, ValueError, KeyError, TypeError) as exc:
        print(f'Token build failed: {exc}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
