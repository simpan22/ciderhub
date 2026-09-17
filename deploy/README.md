# Deploying CiderHub

Runs as a systemd service on the Hetzner box (135.181.151.120), fronted
by the existing nginx installation there, at
https://cider.simonochamanda.se.

No Docker, no build-on-server: the release binary is built locally
(Ubuntu 24.04, glibc 2.39 — matches the server exactly) and shipped
over. Askama templates are compiled into the binary; only the binary
and the `static/` directory need to be on the server. Migrations are
also embedded in the binary and run automatically against the SQLite
file on first startup.

## One-time server setup

Already done on 135.181.151.120, documented here for reproducibility
(e.g. if the server is ever rebuilt):

```bash
# Dedicated system user, no login shell
useradd --system --no-create-home --shell /usr/sbin/nologin ciderhub

# App dir (binary + static/), data dir (sqlite file), config dir
mkdir -p /opt/ciderhub /var/lib/ciderhub /etc/ciderhub
chown ciderhub:ciderhub /opt/ciderhub /var/lib/ciderhub

# Config
scp deploy/ciderhub.env root@135.181.151.120:/etc/ciderhub/ciderhub.env
ssh root@135.181.151.120 'chmod 640 /etc/ciderhub/ciderhub.env && chown root:ciderhub /etc/ciderhub/ciderhub.env'

# systemd unit
scp deploy/ciderhub.service root@135.181.151.120:/etc/systemd/system/ciderhub.service
ssh root@135.181.151.120 'systemctl daemon-reload && systemctl enable ciderhub'

# nginx site (plain HTTP at first — certbot rewrites it to add the
# HTTPS block + redirect, matching the pattern of the box's other
# sites)
scp deploy/nginx-cider.conf root@135.181.151.120:/etc/nginx/sites-available/cider.simonochamanda.se
ssh root@135.181.151.120 '
    ln -sf /etc/nginx/sites-available/cider.simonochamanda.se /etc/nginx/sites-enabled/
    nginx -t && systemctl reload nginx
'

# DNS: add an A (and AAAA, if used) record for cider.simonochamanda.se
# pointing at 135.181.151.120 before requesting a cert — Let's Encrypt
# validates via HTTP-01, which needs the domain to resolve here first.

# Cert (reuses the certbot account already registered on this box)
ssh root@135.181.151.120 'certbot --nginx -d cider.simonochamanda.se --non-interactive --agree-tos --redirect'

# Pull the certbot-modified nginx config back so the repo matches
# what's actually live
scp root@135.181.151.120:/etc/nginx/sites-available/cider.simonochamanda.se deploy/nginx-cider.conf
```

Renewal is automatic — certbot's existing `snap.certbot.renew.timer` on
the box covers every cert on it, this one included.

## Ongoing deploys

**Automatic**: `.github/workflows/deploy.yml` builds and deploys on
every push to `main` — same steps as `deploy.sh` below, just triggered
by CI instead of run by hand. See "Continuous deployment" below for
setup and, importantly, the safety tradeoff it introduces.

**Manual** (still works, e.g. for a one-off deploy without pushing to
`main`, or if CI is down):

```bash
./deploy/deploy.sh
```

Builds the release binary locally, uploads it (staged as `ciderhub.new`
then renamed into place — avoids running a half-uploaded binary if the
transfer drops) plus the `static/` directory via rsync, then restarts
the `ciderhub` service over SSH.

## Continuous deployment

`.github/workflows/deploy.yml` runs the same build-and-ship steps as
`deploy.sh`, triggered automatically on every push to `main`, using a
dedicated deploy SSH keypair (not anyone's personal key).

### sqlx offline mode

CI has no database to check queries against at compile time, so the
build runs with `SQLX_OFFLINE=true` against a cached snapshot of every
query's shape in `.sqlx/` (committed to the repo). **Whenever a query
changes or a migration is added, regenerate it and commit the
result:**

```bash
DATABASE_URL="sqlite://ciderhub.db" cargo sqlx prepare
git add .sqlx
```

If `.sqlx` goes stale, the CI build fails outright (a missing/mismatched
query in the cache is a hard compile error) — it can't silently ship
a binary checked against the wrong schema.

### One-time setup

1. Generate a dedicated keypair (don't reuse a personal key):
   ```bash
   ssh-keygen -t ed25519 -f ./ciderhub_deploy -C "github-actions-deploy@ciderhub" -N ""
   ```
2. Add `ciderhub_deploy.pub`'s contents to `/root/.ssh/authorized_keys`
   on the server.
3. Add `ciderhub_deploy`'s contents (the private key) as the
   `DEPLOY_SSH_KEY` secret on the GitHub repo (Settings → Secrets and
   variables → Actions), then delete the local key files — nothing
   else needs them once the secret is set.

### The safety tradeoff this introduces

`deploy.sh` always ran migrations automatically on startup (embedded
in the binary via `sqlx::migrate!()`) — that part isn't new. What's
new is that a schema-changing push to `main` now deploys itself
immediately, closing the manual gap between "I've written a migration"
and "I've actually run it against production" that this project's
whole verify-against-a-prod-snapshot discipline (see "Schema changes
that need `PRAGMA foreign_keys = OFF`" below, and the git history
around the incident that section documents) depends on. **Keep doing
that verification locally before pushing a migration to `main`** —
CI/CD removes the pause that made it easy to remember to, not the need
for it.

## Layout on the server

| Path                                | Purpose                                    |
|--------------------------------------|---------------------------------------------|
| `/opt/ciderhub/ciderhub`             | the binary                                  |
| `/opt/ciderhub/static/`              | CSS/JS, served via `ServeDir`               |
| `/var/lib/ciderhub/ciderhub.db`      | SQLite database (persistent, not touched by deploys) |
| `/etc/ciderhub/ciderhub.env`         | `DATABASE_URL` / `BIND_ADDR` / `APP_PASSWORD`, root:ciderhub 640 |
| `/etc/systemd/system/ciderhub.service` | service unit                              |
| `/etc/nginx/sites-available/cider.simonochamanda.se` | reverse proxy + TLS (certbot-managed) |

The app listens on `127.0.0.1:8091` — only nginx can reach it directly;
it's never exposed on a public interface.

## The site password (`APP_PASSWORD`)

The shared login password (`src/handlers/auth.rs`) is read from the
`APP_PASSWORD` environment variable — the app refuses to start without
it, no fallback default, so it's never hardcoded in source (this repo
is meant to be safe to publish). `deploy/ciderhub.env` and
`.env.example` only carry a
`changeme` placeholder; the real value exists in exactly one place,
`/etc/ciderhub/ciderhub.env` on the server, edited directly over SSH
and never committed. `deploy/deploy.sh` doesn't touch that file on
ongoing deploys — only the one-time setup above does — so changing the
password later is just:

```bash
ssh root@135.181.151.120 "\$EDITOR /etc/ciderhub/ciderhub.env && systemctl restart ciderhub"
```

## Schema changes that need `PRAGMA foreign_keys = OFF`

SQLite can't `ALTER` a `CHECK` constraint in place — the documented fix
is create-copy-drop-rename, which requires disabling `foreign_keys`
first (otherwise dropping an FK-parent table cascades and destroys
every `ON DELETE CASCADE` child row, even though the parent gets
recreated and renamed back moments later — verified this the hard way
once already, see git history around the "units"/"batch-failure"
deploy).

**`sqlx`'s SQLite migration runner always wraps a migration's SQL in a
transaction, and ignores the `-- no-transaction` marker entirely for
SQLite** (confirmed in `sqlx-sqlite-0.8.6`'s `migrate.rs`: `apply()`
unconditionally calls `self.begin()`). Since `PRAGMA foreign_keys`
can't be toggled inside an open transaction, **any migration that
needs to disable it cannot be a normal `sqlx migrate` file** — it will
silently "succeed" while cascading data loss.

For this kind of change: write a standalone script (see
`deploy/fix_events_check_constraint.py` for the template) that opens
its own connection, sets `PRAGMA foreign_keys = OFF` *before* any
`BEGIN`, does the rebuild, verifies with `PRAGMA foreign_key_check`,
and turns `foreign_keys` back on. Run it directly against the target
database file with the service stopped — never add it to
`migrations/`. Before running it anywhere real: copy the target
database, run the script against the copy, and diff row counts across
every affected table first.

## Backups

Not yet automated. The whole app's state is the one file at
`/var/lib/ciderhub/ciderhub.db` — periodically copying it off-box
(e.g. a cron job piping it through `sqlite3 .backup` to avoid copying
a file mid-write) is the natural next step before this holds data you
can't afford to lose.
