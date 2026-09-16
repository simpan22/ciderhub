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

```bash
./deploy/deploy.sh
```

Builds the release binary locally, uploads it (staged as `ciderhub.new`
then renamed into place — avoids running a half-uploaded binary if the
transfer drops) plus the `static/` directory via rsync, then restarts
the `ciderhub` service over SSH.

## Layout on the server

| Path                                | Purpose                                    |
|--------------------------------------|---------------------------------------------|
| `/opt/ciderhub/ciderhub`             | the binary                                  |
| `/opt/ciderhub/static/`              | CSS/JS, served via `ServeDir`               |
| `/var/lib/ciderhub/ciderhub.db`      | SQLite database (persistent, not touched by deploys) |
| `/etc/ciderhub/ciderhub.env`         | `DATABASE_URL` / `BIND_ADDR`, root:ciderhub 640 |
| `/etc/systemd/system/ciderhub.service` | service unit                              |
| `/etc/nginx/sites-available/cider.simonochamanda.se` | reverse proxy + TLS (certbot-managed) |

The app listens on `127.0.0.1:8091` — only nginx can reach it directly;
it's never exposed on a public interface.

## Backups

Not yet automated. The whole app's state is the one file at
`/var/lib/ciderhub/ciderhub.db` — periodically copying it off-box
(e.g. a cron job piping it through `sqlite3 .backup` to avoid copying
a file mid-write) is the natural next step before this holds data you
can't afford to lose.
