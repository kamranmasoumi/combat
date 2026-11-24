#!/usr/bin/env python3
"""
اسکریپت ساخت مانیفست تفاضلی (SHA-256) و انتشار پچ کلاینت روی سرور Django.

نمونه اجرا:
  1) قرار دادن Game Coordinator در حالت Updating:
     python3 publish_patch.py --coordinator-only updating --api-url https://your-domain.com --secret YOUR_SECRET

  2) ساخت مانیفست از پوشه release (شامل moba_client.exe و پوشه assets/) و انتشار پچ:
     python3 publish_patch.py \
       --release-dir ./dist_release \
       --version 0.2.0 \
       --build-number 2 \
       --min-supported-build 2 \
       --title "Patch 0.2.0 - Aegis Mirror & Demo Overhaul" \
       --notes "Passive+Active Aegis Mirror, Dota 2 Demo Hero Panel, Auto-Updater" \
       --base-download-url /media/patches/0.2.0/ \
       --api-url https://your-domain.com \
       --secret YOUR_SECRET
"""

import argparse
import hashlib
import json
import os
import shutil
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import List, Dict


SKIP_NAMES = {
    ".DS_Store",
    "Thumbs.db",
    "manifest.json",
    "client_build.json",
    "moba_client.exe.old",
}
SKIP_DIRS = {".update_staging", ".git", "__pycache__"}


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while True:
            chunk = f.read(1024 * 1024)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest()


def scan_release_dir(release_dir: Path) -> List[Dict]:
    if not release_dir.is_dir():
        raise FileNotFoundError(f"Release directory not found: {release_dir}")

    entries: List[Dict] = []
    for root, dirs, files in os.walk(release_dir):
        dirs[:] = [d for d in sorted(dirs) if d not in SKIP_DIRS]
        for fname in sorted(files):
            if fname in SKIP_NAMES or fname.endswith(".pdb") or fname.endswith(".old"):
                continue
            full = Path(root) / fname
            rel = full.relative_to(release_dir).as_posix()
            # فقط moba_client.exe در ریشه و فایل‌های داخل assets/
            if rel != "moba_client.exe" and not rel.startswith("assets/"):
                continue
            entries.append(
                {
                    "path": rel,
                    "sha256": sha256_file(full),
                    "size": full.stat().st_size,
                }
            )
    return entries


def post_json(url: str, payload: dict, secret: str) -> dict:
    body = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=body,
        headers={
            "Content-Type": "application/json",
            "X-Internal-Secret": secret,
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        err_text = e.read().decode("utf-8", errors="replace")
        print(f"[HTTP {e.code}] {err_text}", file=sys.stderr)
        sys.exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(description="Build & publish MOBA client patch manifest")
    parser.add_argument("--release-dir", type=str, help="Folder containing moba_client.exe and assets/")
    parser.add_argument("--version", type=str, help="Semantic version, e.g. 0.2.0")
    parser.add_argument("--build-number", type=int, help="Monotonically increasing integer build number")
    parser.add_argument("--min-supported-build", type=int, default=None, help="Minimum allowed build for matchmaking")
    parser.add_argument("--title", type=str, default="Client Update", help="Patch title")
    parser.add_argument("--notes", type=str, default="", help="Release notes")
    parser.add_argument("--base-download-url", type=str, help="Base URL where patch files are hosted")
    parser.add_argument("--optional", action="store_true", help="Mark patch as optional instead of mandatory")
    parser.add_argument("--api-url", type=str, default="", help="Django backend base URL, e.g. http://127.0.0.1:8000")
    parser.add_argument("--secret", type=str, default="", help="INTERNAL_WEBHOOK_SECRET header value")
    parser.add_argument(
        "--coordinator-only",
        choices=["online", "updating", "maintenance"],
        default=None,
        help="Only switch Game Coordinator mode without publishing a patch",
    )
    parser.add_argument("--banner", type=str, default="", help="Optional custom Coordinator banner message")
    parser.add_argument("--manifest-out", type=str, default="manifest.json", help="Local manifest output path")
    parser.add_argument(
        "--media-root",
        type=str,
        default="",
        help="Django MEDIA_ROOT path; if set, auto-creates <media_root>/patches/<version>/ and copies all patch files there",
    )
    args = parser.parse_args()

    api_base = args.api_url.rstrip("/")

    if args.coordinator_only:
        if not api_base:
            print("Error: --api-url is required with --coordinator-only", file=sys.stderr)
            sys.exit(1)
        res = post_json(
            f"{api_base}/api/v1/client/coordinator/set/",
            {
                "mode": args.coordinator_only,
                "banner_message": args.banner,
            },
            args.secret,
        )
        print(f"[COORDINATOR] Updated -> {json.dumps(res, ensure_ascii=False)}")
        return

    if not args.release_dir or not args.version or not args.build_number or not args.base_download_url:
        print("Error: --release-dir, --version, --build-number, and --base-download-url are required.", file=sys.stderr)
        sys.exit(1)

    release_path = Path(args.release_dir)
    files = scan_release_dir(release_path)
    total_size = sum(f["size"] for f in files)
    print(f"[SCAN] Found {len(files)} files ({total_size / (1024 * 1024):.2f} MB)")

    payload = {
        "version": args.version,
        "build_number": args.build_number,
        "min_supported_build": args.min_supported_build or args.build_number,
        "title": args.title,
        "release_notes": args.notes,
        "base_download_url": args.base_download_url,
        "is_mandatory": not args.optional,
        "activate": True,
        "set_coordinator_online": True,
        "files": files,
    }

    out_path = Path(args.manifest_out)
    out_path.write_text(json.dumps(payload, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"[MANIFEST] Written to {out_path}")

    if args.media_root:
        dest_dir = Path(args.media_root) / "patches" / args.version
        dest_dir.mkdir(parents=True, exist_ok=True)
        for item in files:
            src_file = release_path / item["path"]
            dst_file = dest_dir / item["path"]
            dst_file.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src_file, dst_file)
        print(f"[MEDIA] Copied {len(files)} files into {dest_dir}")
    else:
        print(f"[NOTE] --media-root not set; make sure files are uploaded to <MEDIA_ROOT>/patches/{args.version}/")

    if api_base:
        res = post_json(f"{api_base}/api/v1/client/patch/publish/", payload, args.secret)
        print(f"[PUBLISHED] Live broadcast sent -> {json.dumps(res, ensure_ascii=False)}")
    else:
        print("[INFO] --api-url not provided; manifest saved locally only.")


if __name__ == "__main__":
    main()