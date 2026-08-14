---
version: "0.1.0b"
created_at: "2026-08-14T11:42:00+07:00,ATHER"
last_update: "2026-08-14T11:42:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "release-distribution"
  doc_type: "root-cause-analysis"
  scope: "anonymous Desktop installer download"
---

# RCA — Public website download returns 404 for a valid release asset

## Symptom

The production Landing Page renders the Desktop v0.1.0 CTA, but an anonymous
request to both the latest and versioned installer URLs returns HTTP 404.

## Evidence

1. The private `Freshair129/FUNG` release contains the expected asset with
   size 515,089,576 bytes and the verified SHA-256 digest.
2. Authenticated `gh release download` succeeds and matches the local hash.
3. Anonymous requests to the same private-repository asset return HTTP 404.
4. Repository metadata confirms `Freshair129/FUNG` has private visibility.

## Root Cause

GitHub Release assets inherit repository access. A release in the private
source repository is not a public binary channel, even when the release itself
is published and the website uses a stable `releases/latest/download` URL.

## Why the issue escaped detection

The initial integrity gate downloaded through an authenticated GitHub CLI
session. It proved asset equality but did not prove anonymous availability.

## Proposed prevention

- Publish Desktop binaries and hash manifests in the public binary-only
  `Freshair129/FUNG-Releases` repository.
- Keep source and history in the private `Freshair129/FUNG` repository.
- Make anonymous full-file download plus SHA-256 equality a required gate.
- Assert in source tests that the Landing URL cannot point back to the private
  source-repository release path.
- Keep Mobile out of this channel until its separate release gates pass.

## Verification

Public v0.1.0 was downloaded anonymously through the latest URL. Its size is
515,089,576 bytes and SHA-256 is
`f67a78e0b216628d19335646342eea20575d5c7b5a16cccf7daf32c6b780d414`.
The release verification workflow passed as run `31770231402`.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Initial RCA and approved public binary-repository prevention. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-08-14 | beta | Documented the private-release anonymous 404 and public binary-only correction. | pending | ATHER |
