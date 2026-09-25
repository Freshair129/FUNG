"""Executable regression fixtures for scripts/extract_knowledge.py.

Run with an ordinary Python interpreter. These tests are authored for the
single consolidated verification campaign and were intentionally not run in
this implementation packet.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "extract_knowledge.py"


def _text_pdf(text: str) -> bytes:
    escaped = text.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")
    stream = f"BT /F1 12 Tf 72 720 Td ({escaped}) Tj ET\n".encode("ascii")
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
        b"/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        b"<< /Length " + str(len(stream)).encode("ascii") + b" >>\nstream\n" + stream + b"endstream",
    ]
    output = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for index, body in enumerate(objects, 1):
        offsets.append(len(output))
        output.extend(f"{index} 0 obj\n".encode("ascii"))
        output.extend(body)
        output.extend(b"\nendobj\n")
    xref_offset = len(output)
    output.extend(f"xref\n0 {len(offsets)}\n".encode("ascii"))
    output.extend(b"0000000000 65535 f \n")
    for offset in offsets[1:]:
        output.extend(f"{offset:010d} 00000 n \n".encode("ascii"))
    output.extend(
        f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n".encode(
            "ascii"
        )
    )
    return bytes(output)


class KnowledgeExtractorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="fung-knowledge-extract-")
        self.root = Path(self.temporary.name) / "selected"
        self.root.mkdir()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def invoke(self, path: Path) -> tuple[subprocess.CompletedProcess[str], dict[str, object]]:
        result = subprocess.run(
            [sys.executable, "-I", str(SCRIPT), "--root", str(self.root), "--input", str(path)],
            capture_output=True,
            check=False,
            encoding="utf-8",
            timeout=65,
        )
        return result, json.loads(result.stdout)

    def invoke_stdin(self, data: bytes) -> tuple[subprocess.CompletedProcess[bytes], dict[str, object]]:
        result = subprocess.run(
            [
                sys.executable,
                "-I",
                str(SCRIPT),
                "--root",
                str(self.root),
                "--input-stdin",
                "--format",
                "pdf",
            ],
            input=data,
            capture_output=True,
            check=False,
            encoding=None,
            timeout=65,
        )
        return result, json.loads(result.stdout)

    def test_text_extract_preserves_exact_version_hash_and_line_locator(self) -> None:
        raw = "รายงานยอดขาย\nปี 2025\n".encode("utf-8")
        path = self.root / "sales.txt"
        path.write_bytes(raw)
        result, payload = self.invoke(path)

        self.assertEqual(result.returncode, 0)
        self.assertTrue(payload["ok"])
        document = payload["document"]
        self.assertEqual(document["contentSha256"], hashlib.sha256(raw).hexdigest())
        self.assertEqual(document["mimeType"], "text/plain")
        self.assertEqual(document["dependency"], {
            "name": "python-stdlib",
            "version": "stdlib",
            "license": "Python-PSF-2.0",
        })
        self.assertEqual(document["pages"][0]["pageIndex"], None)
        self.assertEqual(document["pages"][0]["text"], "รายงานยอดขาย\nปี 2025\n")
        self.assertEqual(len(document["parserFingerprint"]), 64)

    def test_unqualified_csv_is_explicitly_unsupported(self) -> None:
        path = self.root / "metrics.csv"
        path.write_text("metric,value\nsales,5\n", encoding="utf-8")
        result, payload = self.invoke(path)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(payload["errorCode"], "format_unsupported")

    def test_invalid_utf8_is_unavailable_not_replaced(self) -> None:
        path = self.root / "bad.txt"
        path.write_bytes(b"sales\xff")
        result, payload = self.invoke(path)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(payload["errorCode"], "text_encoding_unsupported")

    def test_selected_root_blocks_files_outside_granted_directory(self) -> None:
        outside = Path(self.temporary.name) / "private.txt"
        outside.write_text("not selected", encoding="utf-8")
        result, payload = self.invoke(outside)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(payload["errorCode"], "selected_file_outside_root")

    def test_pdf_text_has_page_locator_and_pinned_parser_fingerprint(self) -> None:
        path = self.root / "report.pdf"
        path.write_bytes(_text_pdf("Quarterly revenue 42"))
        result, payload = self.invoke(path)
        if payload.get("errorCode") == "dependency_unavailable":
            self.skipTest("pypdf is not staged in this interpreter")
        self.assertEqual(result.returncode, 0, result.stdout)
        document = payload["document"]
        self.assertEqual(document["dependency"], {
            "name": "pypdf",
            "version": "6.10.0",
            "license": "BSD-3-Clause",
        })
        self.assertIn("Quarterly revenue 42", document["pages"][0]["text"])
        self.assertEqual(document["pages"][0]["pageIndex"], 0)
        self.assertEqual(document["contentSha256"], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(len(document["parserFingerprint"]), 64)

    def test_pdf_stdin_uses_only_the_pipe_input(self) -> None:
        raw = _text_pdf("Selected input only")
        result, payload = self.invoke_stdin(raw)
        if payload.get("errorCode") == "dependency_unavailable":
            self.skipTest("pypdf is not staged in this interpreter")
        self.assertEqual(result.returncode, 0, result.stdout)
        document = payload["document"]
        self.assertEqual(document["contentSha256"], hashlib.sha256(raw).hexdigest())
        self.assertIn("Selected input only", document["pages"][0]["text"])

    def test_pdf_stdin_rejects_data_over_the_input_bound(self) -> None:
        result, payload = self.invoke_stdin(b"x" * (25 * 1024 * 1024 + 1))
        self.assertEqual(result.returncode, 2)
        self.assertEqual(payload["errorCode"], "input_size_limit")


if __name__ == "__main__":
    unittest.main()
