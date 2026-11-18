#!/usr/bin/env bash
# Builds Jade the way a player's launcher does, without a launcher install:
#   1. `cargo sunrise-mod check`, then `build`: target/sunrise-mod/jade.smod;
#   2. the package unpacked into $JADE_TARGET/mods/jade, the folder of mods the build reads;
#   3. `cargo sunrise-mod native-build` (the SDK's own tool, the launcher's code) against the
#      2d81263 SDK's runtime and api slices: a game executable with Jade linked in.
# Prints the built executable's path last. Needs the SDK slices unpacked in $JADE_SDK/pkg/{runtime,api,engine}
# (tools/fetch_sdk.sh).
set -euo pipefail
cd "$(dirname "$0")/.."
. tools/env.sh

step() { printf '\n== %s\n' "$*" >&2; }

if [ "${SKIP_CHECK:-}" != 1 ]; then
  step "cargo sunrise-mod check"
  sunrise_mod check
fi
step "cargo sunrise-mod build"
sunrise_mod build
SMOD=target/sunrise-mod/jade.smod
test -f "$SMOD"

step "stage the package"
rm -rf "$JADE_TARGET/mods"
mkdir -p "$JADE_TARGET/mods/jade"
(cd "$JADE_TARGET/mods/jade" && python3 -I -c 'import sys, zipfile; zipfile.ZipFile(sys.argv[1]).extractall(".")' "$OLDPWD/$SMOD")
cp "$SMOD" "$JADE_TARGET/jade.smod"

step "native-build"
WORK="$JADE_TARGET/native-work"
mkdir -p "$WORK" "$JADE_TARGET/cargo-home"
: > "$WORK/build.log"
PLAN=$(python3 -I - "$WORK" "$RUSTUP_HOME/toolchains/$RUSTUP_TOOLCHAIN" "$JADE_TARGET/cargo-home" "$JADE_SDK/pkg" "$JADE_TARGET/mods" <<'EOF'
import json, sys
work, toolchain, cargo_home, pkg, mods = sys.argv[1:]
print(json.dumps({
    "contract": 2,
    "work": work,
    "toolchain": {"dir": toolchain, "target": "x86_64-unknown-linux-gnu", "cargo_home": cargo_home,
                  "cargo_config": [], "c_bin": None},
    "package": {"runtime": f"{pkg}/runtime", "api": f"{pkg}/api", "engine": f"{pkg}/engine"},
    "mods": mods,
    "target": "client",
    "registry": False,
}))
EOF
)
set +e
echo "$PLAN" | "$JADE_SDK/tool/cargo-sunrise-mod" sunrise-mod native-build > "$JADE_TARGET/native-events.jsonl"
CODE=$?
set -e
tail -n 3 "$JADE_TARGET/native-events.jsonl" >&2
if [ $CODE -ne 0 ]; then
  echo "native-build failed ($CODE); log: $WORK/build.log" >&2
  tail -n 40 "$WORK/build.log" >&2
  exit $CODE
fi
python3 -I -c 'import json,sys; e=[json.loads(l) for l in open(sys.argv[1])][-1]; print(e["binary"])' "$JADE_TARGET/native-events.jsonl"
