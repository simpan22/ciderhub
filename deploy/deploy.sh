#!/usr/bin/env bash
# Builds the release binary and ships it + static assets to the server,
# then restarts the service. Assumes one-time server setup is already
# done (see deploy/README.md) — dedicated user, directories, systemd
# unit, and nginx site all already in place.
set -euo pipefail

REMOTE_HOST="135.181.151.120"
REMOTE_USER="root"
REMOTE_APP_DIR="/opt/ciderhub"
SERVICE_NAME="ciderhub"

cd "$(dirname "$0")/.."

echo "==> Building release binary"
# sqlx::migrate!() tracks individual migration file paths but doesn't
# reliably detect *new* files added to the migrations/ directory, so a
# cached build can silently ship without a just-added migration
# embedded. Touching the file with the macro invocation forces cargo to
# re-run it (and re-scan the directory) on every deploy.
touch src/db.rs
cargo build --release

echo "==> Uploading binary"
scp target/release/ciderhub "$REMOTE_USER@$REMOTE_HOST:$REMOTE_APP_DIR/ciderhub.new"

echo "==> Uploading static assets"
rsync -az --delete static/ "$REMOTE_USER@$REMOTE_HOST:$REMOTE_APP_DIR/static/"

echo "==> Installing and restarting service"
ssh "$REMOTE_USER@$REMOTE_HOST" bash -s <<EOF
set -euo pipefail
mv "$REMOTE_APP_DIR/ciderhub.new" "$REMOTE_APP_DIR/ciderhub"
chown ciderhub:ciderhub "$REMOTE_APP_DIR/ciderhub"
chmod 755 "$REMOTE_APP_DIR/ciderhub"
chown -R ciderhub:ciderhub "$REMOTE_APP_DIR/static"
systemctl restart "$SERVICE_NAME"
sleep 1
systemctl is-active "$SERVICE_NAME"
EOF

echo "==> Deployed. https://cider.simonochamanda.se"
