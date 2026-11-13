# gh-roam

[![Build](https://github.com/arriba-tools/gh-roam/actions/workflows/build.yml/badge.svg)](https://github.com/arriba-tools/gh-roam/actions/workflows/build.yml)
[![Lint](https://github.com/arriba-tools/gh-roam/actions/workflows/lint.yml/badge.svg)](https://github.com/arriba-tools/gh-roam/actions/workflows/lint.yml)
[![codecov](https://codecov.io/gh/arriba-tools/gh-roam/graph/badge.svg)](https://codecov.io/gh/arriba-tools/gh-roam)

A GitHub CLI extension that roams config files between machines through a gist, such as
`.vscode/settings.json`, `.claude/CLAUDE.md` or a dotfile in your home folder. Each project gets
one gist that holds the files plus a manifest listing them.

## Contents

- [Install](#install)
- [Usage](#usage)
- [First machine](#first-machine)
- [Every other machine](#every-other-machine)
- [Which side wins](#which-side-wins)
- [Platform-specific files](#platform-specific-files)
- [Options](#options)
- [Safety](#safety)
- [Gist layout](#gist-layout)
- [Authentication](#authentication)
- [Development](#development)
- [License](#license)

## Install

```
gh extension install arriba-tools/gh-roam
```

Upgrade with `gh extension upgrade roam`.

## Usage

| Command | Description |
|---------|-------------|
| `roam [sync] [files]` | Push or pull each tracked file, whichever side is newer (default) |
| `roam pull [files]` | Pull each tracked file the gist has newer, without pushing anything |
| `roam status [files]` | Show what sync would push or pull, without changing anything |
| `roam add [--platform] <files>` | Track files and push them, creating the gist on first use |
| `roam remove [--platform] <files>` | Stop tracking files and delete them from the gist |
| `roam restore [files]` | Overwrite local files with the gist versions |
| `roam cleanup` | Delete gist files the manifest no longer lists |
| `roam open` | Open the project's gist in a web browser |
| `roam delete` | Delete the project's gist, after confirming |

## First machine

Track the files from anywhere inside the project:

```
gh roam add --private .vscode/settings.json .claude/CLAUDE.md
```

This creates the gist and pushes the files. Gists are public unless you pass `--private`, and the
flag only matters here, when the gist is created.

## Every other machine

Run `roam` with no arguments inside the same project:

```
gh roam
```

It reads the manifest from the gist, pulls every tracked file, and pushes any you changed locally.
Run it again whenever you edit a tracked file. To track another file later, run `roam add` again
from any machine.

A machine that should only receive changes runs `gh roam pull` instead. It never writes to the
gist, and its own newer files stay as they are.

## Which side wins

Sync works per file and never merges. It compares each file's local modification time with the time
the manifest recorded for it.

| Local file | Sync | Pull |
|------------|------|------|
| Newer than the manifest | Pushes it to the gist | Keeps it, unpushed |
| Older than the manifest | Pulls the gist version | Pulls the gist version |
| Missing | Pulls the gist version | Pulls the gist version |
| Within a second of the manifest | Leaves it alone | Leaves it alone |

Edit a file on one machine at a time, since the newer copy overwrites the other. Run `roam status`
first to see what would happen, or `roam restore` to take the gist version regardless.

## Platform-specific files

Add a file with `--platform` when it only applies to the current OS, such as a toolchain path:

```
gh roam add --platform .vscode/cmake-variants.yaml
```

Run the same command on each OS and each keeps its own copy of that path in the gist. Sync only
touches the copy for the OS it runs on, `macos`, `linux`, or `windows`.

## Options

| Option | Description |
|--------|-------------|
| `--name <NAME>` | Gist name prefix (default: basename of the project root) |
| `--root <DIR>` | Directory that file paths are relative to (default: git root, or the current directory) |
| `--private` | Create the gist as secret instead of public |
| `-y`, `--yes` | Skip confirmation prompts |
| `-h`, `--help` | Print help for any command |
| `-V`, `--version` | Print version information |

## Safety

- Before pushing, `roam` scans files for potential secrets such as API keys, tokens, and passwords,
  and aborts if it finds any.
- `roam delete` asks for confirmation, or takes `--yes`.

## Gist layout

The gist's description is `"<project> config-sync"`, which is how `roam` finds it again. Each file
is stored under a flat name with underscores for path separators, as in
`zlib-ng_.vscode_settings.json`, and platform-specific files tag the OS before the extension, as in
`zlib-ng_.vscode_settings[macos].json`. The manifest is `.<project>-config-sync.yaml`.

The layout is the one `gh arriba config` used, so its gists work with `roam` unchanged.

## Authentication

Uses the token from `gh auth token`, falling back to `GH_TOKEN` or `GITHUB_TOKEN`. Set `GH_HOST` to
target GitHub Enterprise Server.

## Development

```
git clone https://github.com/arriba-tools/gh-roam.git
cd gh-roam
cargo build --release
cp target/release/gh-roam .
gh extension install .
```

The copied binary is ignored by git. Rebuild and copy it again to pick up changes.

## License

MIT
