#!/usr/bin/env python3
"""Bounded local text/Markdown/PDF extractor for explicitly selected files.

This process reads only the one file and the one selected root supplied by the
native caller. It has no network, macro, formula, OCR, or document discovery
code. The native launcher supplies the actual process sandbox and 60-second
hard timeout; this script adds an in-process deadline between PDF pages.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import io
import json
import sys
import time
from pathlib import Path
from typing import Any

PARSER_VERSION = "fung-knowledge-extractor/0.1.0"
PYPDF_VERSION = "6.10.0"
MAX_INPUT_BYTES = 25 * 1024 * 1024
MAX_PAGES = 500
MAX_TEXT_CHARS = 4 * 1024 * 1024
MAX_OUTPUT_BYTES = 8 * 1024 * 1024
MAX_PARSER_SECONDS = 60.0


class ExtractError(Exception):
    def __init__(self, code: str):
        super().__init__(code)
        self.code = code


def _selected_input(root_arg: str, input_arg: str) -> tuple[Path, bytes]:
    try:
        root = Path(root_arg).resolve(strict=True)
        path = Path(input_arg).resolve(strict=True)
    except OSError as exc:
        raise ExtractError("selected_file_unavailable") from exc
    if not root.is_dir():
        raise ExtractError("selected_root_unavailable")
    try:
        path.relative_to(root)
    except ValueError as exc:
        raise ExtractError("selected_file_outside_root") from exc
    if not path.is_file():
        raise ExtractError("selected_file_unavailable")
    try:
        size = path.stat().st_size
        if size <= 0 or size > MAX_INPUT_BYTES:
            raise ExtractError("input_size_limit")
        with path.open("rb") as source:
            data = source.read(MAX_INPUT_BYTES + 1)
    except OSError as exc:
        raise ExtractError("selected_file_unavailable") from exc
    if len(data) != size or len(data) > MAX_INPUT_BYTES:
        raise ExtractError("input_size_limit")
    return path, data


def _pypdf_metadata() -> tuple[Any, str, str]:
    try:
        import pypdf

        distribution = importlib.metadata.distribution("pypdf")
    except (ImportError, importlib.metadata.PackageNotFoundError) as exc:
        raise ExtractError("dependency_unavailable") from exc
    version = distribution.version
    if version != PYPDF_VERSION or getattr(pypdf, "__version__", None) != PYPDF_VERSION:
        raise ExtractError("dependency_version_mismatch")
    license_name = (
        distribution.metadata.get("License-Expression")
        or distribution.metadata.get("License")
        or ""
    )
    if license_name not in ("BSD-3-Clause", "BSD 3-Clause License"):
        raise ExtractError("dependency_license_mismatch")
    return pypdf, version, "BSD-3-Clause"


def _parser_fingerprint(script: Path, pypdf: Any, version: str) -> str:
    digest = hashlib.sha256()
    digest.update(PARSER_VERSION.encode("ascii"))
    digest.update(b"\0pypdf==")
    digest.update(version.encode("ascii"))
    for source in sorted(Path(pypdf.__file__).resolve().parent.rglob("*.py")):
        relative = source.name if source.parent == Path(pypdf.__file__).resolve().parent else source.relative_to(
            Path(pypdf.__file__).resolve().parent
        ).as_posix()
        encoded_name = relative.encode("utf-8")
        try:
            body = source.read_bytes()
        except OSError as exc:
            raise ExtractError("dependency_fingerprint_unavailable") from exc
        digest.update(len(encoded_name).to_bytes(4, "big"))
        digest.update(encoded_name)
        digest.update(len(body).to_bytes(8, "big"))
        digest.update(body)
    try:
        script_bytes = script.read_bytes()
    except OSError as exc:
        raise ExtractError("parser_fingerprint_unavailable") from exc
    digest.update(len(script_bytes).to_bytes(8, "big"))
    digest.update(script_bytes)
    return digest.hexdigest()


def _text_document(data: bytes, mime_type: str) -> list[dict[str, Any]]:
    if b"\0" in data:
        raise ExtractError("binary_text_unsupported")
    try:
        text = data.decode("utf-8", errors="strict")
    except UnicodeDecodeError as exc:
        raise ExtractError("text_encoding_unsupported") from exc
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    if len(text) > MAX_TEXT_CHARS:
        raise ExtractError("text_output_limit")
    return [{"pageIndex": None, "startChar": 0, "endChar": len(text), "text": text}]


def _pdf_document(data: bytes, pypdf: Any, started_at: float) -> tuple[list[dict[str, Any]], list[str]]:
    try:
        reader = pypdf.PdfReader(io.BytesIO(data), strict=True)
    except Exception as exc:  # pypdf errors are reduced to stable codes.
        raise ExtractError("pdf_parse_failed") from exc
    if reader.is_encrypted:
        raise ExtractError("encrypted_pdf_unsupported")
    if len(reader.pages) > MAX_PAGES:
        raise ExtractError("pdf_page_limit")

    pages: list[dict[str, Any]] = []
    warnings: list[str] = []
    total_chars = 0
    for page_index, page in enumerate(reader.pages):
        if time.monotonic() - started_at > MAX_PARSER_SECONDS:
            raise ExtractError("parser_deadline")
        try:
            text = page.extract_text() or ""
        except Exception:
            text = ""
            warnings.append("page_text_extraction_failed")
        total_chars += len(text)
        if total_chars > MAX_TEXT_CHARS:
            raise ExtractError("text_output_limit")
        pages.append(
            {"pageIndex": page_index, "startChar": 0, "endChar": len(text), "text": text}
        )
    if total_chars == 0:
        warnings.append("no_embedded_text; OCR_not_performed")
    return pages, warnings


def _extract_bytes(data: bytes, extension: str, script: Path) -> dict[str, Any]:
    started_at = time.monotonic()
    mime_types = {".txt": "text/plain", ".md": "text/markdown", ".pdf": "application/pdf"}
    mime_type = mime_types.get(extension)
    if mime_type is None:
        raise ExtractError("format_unsupported")

    warnings: list[str] = []
    if extension == ".pdf":
        if not data.startswith(b"%PDF-"):
            raise ExtractError("pdf_signature_invalid")
        pypdf, dependency_version, license_name = _pypdf_metadata()
        pages, warnings = _pdf_document(data, pypdf, started_at)
    else:
        if data.startswith(b"%PDF-"):
            raise ExtractError("mime_extension_mismatch")
        dependency_version = "stdlib"
        license_name = "Python-PSF-2.0"
        pages = _text_document(data, mime_type)
    if time.monotonic() - started_at > MAX_PARSER_SECONDS:
        raise ExtractError("parser_deadline")

    document = {
        "parserVersion": PARSER_VERSION,
        "parserFingerprint": hashlib.sha256(
            script.read_bytes() + b"\0" + dependency_version.encode("ascii")
        ).hexdigest(),
        "dependency": {
            "name": "pypdf" if extension == ".pdf" else "python-stdlib",
            "version": dependency_version,
            "license": license_name,
        },
        "contentSha256": hashlib.sha256(data).hexdigest(),
        "mimeType": mime_type,
        "sourceBytes": len(data),
        "pages": pages,
        "warnings": warnings,
    }
    # Include the pinned parser package source files in PDF provenance. The
    # top-level parser fingerprint remains stable for text/Markdown files.
    if extension == ".pdf":
        document["parserFingerprint"] = _parser_fingerprint(script, pypdf, dependency_version)
    return {"ok": True, "document": document}


def extract(root_arg: str, input_arg: str) -> dict[str, Any]:
    path, data = _selected_input(root_arg, input_arg)
    return _extract_bytes(data, path.suffix.lower(), Path(__file__))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, help=argparse.SUPPRESS)
    parser.add_argument("--input", help=argparse.SUPPRESS)
    parser.add_argument("--input-stdin", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--format", choices=("pdf",), help=argparse.SUPPRESS)
    args = parser.parse_args()
    try:
        if args.input_stdin:
            if args.format != "pdf" or args.input is not None:
                raise ExtractError("parser_invocation_invalid")
            data = sys.stdin.buffer.read(MAX_INPUT_BYTES + 1)
            if not data or len(data) > MAX_INPUT_BYTES:
                raise ExtractError("input_size_limit")
            response = _extract_bytes(data, ".pdf", Path(__file__))
        else:
            if not args.input or args.format is not None:
                raise ExtractError("parser_invocation_invalid")
            response = extract(args.root, args.input)
        output = json.dumps(response, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(output) > MAX_OUTPUT_BYTES:
            raise ExtractError("output_size_limit")
        sys.stdout.buffer.write(output)
        sys.stdout.buffer.write(b"\n")
        return 0
    except ExtractError as exc:
        response = {"ok": False, "errorCode": exc.code}
        sys.stdout.write(json.dumps(response, separators=(",", ":")) + "\n")
        return 2
    except Exception:
        # Avoid emitting paths, source text, or parser internals in diagnostics.
        sys.stdout.write('{"ok":false,"errorCode":"extractor_internal_error"}\n')
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
