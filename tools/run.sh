#!/usr/bin/env bash
# Starts the game built by tools/build.sh offscreen, in fresh scratch folders, with Jade's package
# in the game folder's sunrise-mods/, and a control socket for sunrise-ctl. Prints the scratch
# folder; the socket is $S/ctl.sock, the game's PID is in $S/game.pid. Stop it with:
# sunrise-ctl --socket $S/ctl.sock quit. On this machine heavy jobs go through crew:
#   CREW_AGENT=website JADE_WAIT=1 crew run --kind harness -- tools/run.sh
# (JADE_WAIT=1 keeps the script alive until the game exits, so crew sees the whole run).
#
#   RESOURCES  prepared 26.3 resources (read only); default: $JADE_TARGET/resources/26.3, a copy of the
#              installed 2d81263 Nightly's (the 520fc48 game reads them as they are)
#   GAME       the built executable; default: the last native-build's
set -euo pipefail
cd "$(dirname "$0")/.."
. tools/env.sh
GAME="${GAME:-$JADE_TARGET/install/sunrise-client}"
RESOURCES="${RESOURCES:-$JADE_TARGET/resources/26.3}"
test -x "$GAME" || { echo "no game at $GAME: run tools/build.sh" >&2; exit 1; }
test -d "$RESOURCES" || { echo "no prepared resources at $RESOURCES" >&2; exit 1; }
# Not $TMPDIR: a job runner may give each job its own and delete it when the job ends.
S=$(mktemp -d /tmp/jade-run.XXXXXX)
mkdir -p "$S/home" "$S/launcher" "$S/data" "$S/config" "$S/game/sunrise-mods"
cp "$JADE_TARGET/jade.smod" "$S/game/sunrise-mods/"
export SUNRISE_HOME="$S/home" SUNRISE_LAUNCHER_HOME="$S/launcher" XDG_DATA_HOME="$S/data" XDG_CONFIG_HOME="$S/config"
export SUNRISE_LAUNCHER_NO_SELF_UPDATE=1 SUNRISE_NO_AUDIO=1
cd "$S/game"
setsid "$GAME" --offscreen --control --control-socket "$S/ctl.sock" --resources "$RESOURCES" --game-dir "$S/game" "$@" \
  > "$S/game.out" 2>&1 < /dev/null &
GAME_PID=$!
echo "$GAME_PID" > "$S/game.pid"
echo "$S"
# JADE_WAIT=1: stay until the game exits, so a job runner (crew) accounts for the whole run.
if [ "${JADE_WAIT:-}" = 1 ]; then
  while kill -0 "$GAME_PID" 2>/dev/null; do sleep 2; done
fi
