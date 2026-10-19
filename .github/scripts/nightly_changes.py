"""List commits since the last published nightly, including intervening failed builds."""

import html
import json
import os
import re
import subprocess
from urllib.parse import quote


def changes(repository, commit, releases):
    previous = max(
        (
            release
            for release in releases
            if release["tag_name"].startswith("nightly-")
            and release["prerelease"]
            and not release["draft"]
            and release["published_at"]
        ),
        key=lambda release: release["published_at"],
        default=None,
    )
    base_url = f"https://github.com/{repository}"
    revision = f"refs/tags/{previous['tag_name']}..{commit}" if previous else commit
    log = subprocess.check_output(
        ["git", "-c", "log.showSignature=false", "log", "--reverse", "--format=%H%x09%s", revision, "--"],
        text=True,
        encoding="utf-8",
    )
    lines = [f"Automated nightly build from commit [{commit[:7]}]({base_url}/commit/{commit}).", ""]
    if previous:
        tag = previous["tag_name"]
        lines.extend([f"### Changes since [{tag}]({base_url}/releases/tag/{quote(tag, safe='')})", ""])
    else:
        lines.extend(["### Changes in first nightly", ""])
    for entry in log.splitlines():
        sha, _, subject = entry.partition("\t")
        title = re.sub(r"([\\`*_\[\]])", r"\\\1", html.escape(subject, quote=False))
        lines.append(f"- {title} ([{sha[:7]}]({base_url}/commit/{sha}))")
    if not log.strip():
        lines.append("No new commits since previous nightly.")
    if previous:
        lines.extend(["", f"[Full comparison]({base_url}/compare/{quote(previous['tag_name'], safe='')}...{commit})"])
    return "\n".join(lines) + "\n\n"


if __name__ == "__main__":
    repository = os.environ["GH_REPO"]
    pages = json.loads(subprocess.check_output(
        ["gh", "api", "--paginate", "--slurp", f"repos/{repository}/releases?per_page=100"],
        text=True,
        encoding="utf-8",
    ))
    print(changes(repository, os.environ["COMMIT"], [release for page in pages for release in page]), end="")
