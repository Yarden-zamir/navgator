#!/bin/sh
# Build this checkout and serve it on 127.0.0.1:4747 for the T3 preview panel.
# The "Web preview" action in t3.json runs this script.
#
# A running `navgator web` from any checkout holds port 4747. Thus stop it
# after the build, or the preview shows the old build.
set -eu
cd "$(dirname "$0")/.."
port=4747
[ -d web/ui/node_modules ] || npm --prefix web/ui ci
cargo build --release

# Stop only the process that listens on the port, and only if it answers as
# navgator. Another program on the port makes `navgator web` fail below.
if curl -fsS -m 2 "http://127.0.0.1:$port/api/health" 2>/dev/null | grep -q '^navgator web'; then
  lsof -nP -t -iTCP:"$port" -sTCP:LISTEN | xargs kill
  while lsof -nP -t -iTCP:"$port" -sTCP:LISTEN >/dev/null; do sleep 0.2; done
fi
exec target/release/navgator web --no-open --port "$port"
