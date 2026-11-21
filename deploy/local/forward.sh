#!/usr/bin/env bash
# Forwards the local stack to localhost until Ctrl-C:
#
#   http://localhost:8088          web UI (UI API under /ui/api, no tokens)
#   http://localhost:8089          machine port: /api and /mcp (API tokens)
#
# Reconnects by itself when up.sh replaces the pods.
# TR_LOCAL_UI_PORT and TR_LOCAL_API_PORT change the local ports.
# KUBE_CONTEXT overrides the context (default docker-desktop).
set -euo pipefail

context=${KUBE_CONTEXT:-docker-desktop}
ui_port=${TR_LOCAL_UI_PORT:-8088}
api_port=${TR_LOCAL_API_PORT:-8089}

# kubectl port-forward ends when its pod goes away; start it again.
forward() {
    while true; do
        kubectl --context "$context" --namespace track-record port-forward "$1" "$2" >/dev/null 2>&1 || true
        sleep 1
    done
}

forward svc/frontend "$ui_port:80" &
ui=$!
forward svc/server "$api_port:8080" &
api=$!
trap 'kill "$ui" "$api" 2>/dev/null || true; pkill -P "$ui" 2>/dev/null; pkill -P "$api" 2>/dev/null; exit 0' EXIT INT TERM

echo "Web UI:       http://localhost:$ui_port"
echo "Machine API:  http://localhost:$api_port/api  (MCP: /mcp)"
echo "Press Ctrl-C to stop."
wait
