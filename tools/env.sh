# Source this before building: `. tools/env.sh`.
# Everything goes in one target folder beside the mod (never the game checkout's target/), with the
# Rust release the pinned SDK names (sdk.json `toolchain.rust`, 1.99.1) and the SDK's own
# cargo-sunrise-mod tool.
JADE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export JADE_TARGET="${JADE_TARGET:-$(dirname "$JADE_ROOT")/jade-target}"
export CARGO_TARGET_DIR="$JADE_TARGET"
export RUSTUP_HOME="$JADE_TARGET/rustup"
export RUSTUP_TOOLCHAIN=1.99.1-x86_64-unknown-linux-gnu
export PATH="$RUSTUP_HOME/toolchains/$RUSTUP_TOOLCHAIN/bin:$PATH"
export JADE_SDK="$JADE_TARGET/sdk/520fc48"
sunrise_mod() { "$JADE_SDK/tool/cargo-sunrise-mod" sunrise-mod "$@"; }
