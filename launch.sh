#!/usr/bin/env bash
# Launch Azeroth Overseer for a live presentation: builds a release binary
# if needed, starts the server, waits for it to come up, then opens a
# kiosk-mode browser window pointed at it. Ctrl+C stops both.
#
# If something is already listening on the configured port, this assumes
# the server is already running (e.g. started separately during setup) and
# just opens the browser, without touching that process.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

if ! command -v cargo >/dev/null 2>&1 && [[ -f "$HOME/.cargo/env" ]]; then
	source "$HOME/.cargo/env"
fi

CONFIG_FILE="config.toml"
PORT=7777
if [[ -f "$CONFIG_FILE" ]]; then
	extracted=$(grep -E '^\s*http_port\s*=' "$CONFIG_FILE" | head -1 | sed -E 's/.*=\s*([0-9]+).*/\1/')
	[[ -n "$extracted" ]] && PORT="$extracted"
fi
URL="http://localhost:${PORT}"

port_is_open() {
	timeout 1 bash -c "echo > /dev/tcp/127.0.0.1/${PORT}" 2>/dev/null
}

SERVER_PID=""
cleanup() {
	if [[ -n "$SERVER_PID" ]]; then
		echo "Stopping server (pid ${SERVER_PID})..."
		kill "$SERVER_PID" 2>/dev/null || true
		wait "$SERVER_PID" 2>/dev/null || true
	fi
}
trap cleanup EXIT INT TERM

if port_is_open; then
	echo "Something is already listening on port ${PORT} -- assuming the server is already running."
else
	if [[ ! -f "$CONFIG_FILE" ]]; then
		echo "config.toml not found. Copy config.toml.example to config.toml and fill in your database credentials first." >&2
		exit 1
	fi

	echo "Building release binary (this can take a minute the first time)..."
	cargo build --release --bin overseer

	echo "Starting server..."
	./target/release/overseer &
	SERVER_PID=$!

	echo "Waiting for it to come up on port ${PORT}..."
	up=0
	for _ in $(seq 1 30); do
		if port_is_open; then
			up=1
			break
		fi
		sleep 0.5
	done
	if [[ "$up" -ne 1 ]]; then
		echo "Server did not come up on port ${PORT} within 15s -- check the output above." >&2
		exit 1
	fi
fi

open_kiosk() {
	for browser in google-chrome google-chrome-stable chromium chromium-browser; do
		if command -v "$browser" >/dev/null 2>&1; then
			"$browser" --kiosk --new-window "$URL" >/dev/null 2>&1 &
			disown
			return 0
		fi
	done
	if command -v firefox >/dev/null 2>&1; then
		firefox --kiosk "$URL" >/dev/null 2>&1 &
		disown
		return 0
	fi
	return 1
}

echo "Opening ${URL} in kiosk mode..."
if ! open_kiosk; then
	echo "No supported browser (Chrome/Chromium/Firefox) found on PATH." >&2
	echo "Open ${URL} manually and go fullscreen (F11)." >&2
fi

if [[ -n "$SERVER_PID" ]]; then
	echo "Server running (pid ${SERVER_PID}). Press Ctrl+C here to stop it."
	wait "$SERVER_PID"
else
	echo "Using the already-running server -- this script won't stop it. Press Ctrl+C to exit."
	while true; do sleep 3600; done
fi
