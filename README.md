# pollingjuegos

## Database

The app uses a local SQLite database, `db.sqlite3`. No database server is needed:

```sh
uv sync
uv run manage.py migrate
```

Run tests with:

```sh
uv run manage.py test
```

Each test run creates and removes its own uniquely named SQLite file in the system temporary directory; tests never use `db.sqlite3`.

## QB poll JSON API

When `DJANGO_DEBUG=true`, interactive OpenAPI docs are at `/api/docs` (schema: `/api/openapi.json`); both routes are disabled otherwise. Weeks and the QB/team catalog are maintained in Django admin; only existing weeks accept ballots. All reads are JSON:

- `GET /api/teams` — team IDs, names and abbreviations.
- `GET /api/quarterbacks?q=chiefs&team_id=16&limit=50&offset=0` — search QB names, team names or abbreviations (case-insensitive substring); filters are optional, limit is 1–100.
- `GET /api/weeks` — available seasons/weeks.
- `GET /api/weeks/{season}/{week}/rankings` — public weekly top ten, total ballots, points and vote counts. Each ballot gives 15 points to #1 through 1 point to #15. Ties sort by number of votes, then QB name and ID; ranks are numbered 1–10.
- `GET /api/weeks/{season}/{week}/ballot` — signed-in voter's ballot (empty entries if not submitted).
- `PUT /api/weeks/{season}/{week}/ballot` — create or **replace** the signed-in voter's ballot. Body: `{"quarterback_ids": [/* exactly 15 distinct IDs ordered #1 to #15 */]}`. Invalid ballots return 400, missing weeks 404. Repeating a PUT does not create another ballot.

Voting uses the existing allowlisted Discord login session, not a client-supplied voter ID. For browser writes, first call `GET /api/csrf` to receive a `csrftoken` cookie and `csrf_token` response value, then send `X-CSRFToken: <csrf_token>` and the session cookie with the PUT. A separate frontend should initially proxy `/api/` and `/accounts/` through the same origin; cross-origin cookie/CORS deployment requires explicit trusted-origin, credentials and cookie configuration. Rankings are live (not frozen when a week ends); admins control which weeks exist, and ballots can currently be replaced at any time.

## PollingJuegos frontend

The minimal Vue app in `frontend/` shows weekly rankings and lets signed-in voters
build, reorder, and save a 15-quarterback ballot. Admins create weeks in Django.

For local development, use two terminals:

```sh
DJANGO_DEBUG=true uv run manage.py runserver 127.0.0.1:8000
cd frontend && pnpm install && pnpm dev
```

Open **http://localhost:5173** (not port 8000). Vite proxies `/api/` and
`/accounts/` to Django, keeping cookies and CSRF same-origin. For local Discord
login, configure the OAuth redirect URL as
`http://localhost:5173/accounts/discord/login/callback/`.

For production, build with `cd frontend && pnpm install --frozen-lockfile && pnpm build`.
Serve `frontend/dist/` at `https://pj.ryansheppard.xyz/` and proxy `/api/`,
`/accounts/`, `/admin/`, and `/static/` to Django on the **same host**. For
example, with Caddy on the droplet (adjust the absolute build path):

```caddyfile
pj.ryansheppard.xyz {
    @django path /api/* /accounts/* /admin/* /static/*
    handle @django {
        reverse_proxy 127.0.0.1:8000
    }
    handle {
        root * /opt/pollingjuegos/frontend/dist
        try_files {path} /index.html
        file_server
    }
}
```

Bind Django only to loopback, set `DJANGO_ALLOWED_HOSTS=pj.ryansheppard.xyz`,
`DJANGO_TRUST_PROXY=true`, and a persistent `DJANGO_SECRET_KEY`. Only set
`DJANGO_TRUST_PROXY=true` behind a trusted proxy that sets `X-Forwarded-Proto`;
never expose that Django port directly to the internet. Set the Discord OAuth
redirect URL to
`https://pj.ryansheppard.xyz/accounts/discord/login/callback/`.
No cross-origin CORS or CSRF exceptions are required for this setup. Django's
`/` login placeholder is not used when the reverse proxy serves the frontend.

## Single-droplet deployment

`terraform/` provisions a NYC3 Ubuntu 24.04 droplet (`s-1vcpu-512mb-10gb`),
using the **existing DigitalOcean SSH key named `arch`**, and a firewall allowing
only TCP 80 and 443 from IPv4 and IPv6. Public SSH (22) and Gunicorn (8000)
are not exposed. Port 80 is needed for Caddy's HTTP-to-HTTPS redirect and ACME
HTTP challenges. For an existing droplet, SSH access must already work over
Tailscale before applying this firewall change. For a *new* droplet, bootstrap
Tailscale via the DO console or temporarily allow SSH from **your own IP**;
remove that temporary rule after verifying a new Tailscale SSH session. The droplet
and firewall are named `pollingjuegos`. Terraform does not manage DNS, secrets,
or application releases.

```sh
cd terraform
export DIGITALOCEAN_TOKEN=...  # token with permission to manage droplets/firewalls and read SSH keys
terraform init
terraform plan
terraform apply
terraform output ipv4_address
```

Set a Cloudflare **DNS-only** A record for `pj.ryansheppard.xyz` pointing at
that IP before starting Caddy; do not add an AAAA record unless you have an
IPv6 address configured. If enabling Cloudflare's proxy later, review TLS mode
(Full strict) and the ACME/certificate setup. Register the Discord callback
`https://pj.ryansheppard.xyz/accounts/discord/login/callback/`.
Terraform state contains infrastructure details: keep it private (and use a
secure remote backend if collaborating). Optional paid DigitalOcean droplet
backups can be enabled with `-var='enable_droplet_backups=true'`.

Build the Vue app **locally** (not on the 512 MB droplet):

```sh
pnpm -C frontend install --frozen-lockfile
pnpm -C frontend build
```

If mise is installed at `/root/.local/bin/mise`, run this **in a root Bash
shell** (`sudo -i` first if needed) to activate mise for future interactive
sessions. Copy the binary to a location accessible to the app user, because
`/root` is not accessible to `pollingjuegos`:

```sh
echo "eval \"\$(/root/.local/bin/mise activate bash)\"" >> ~/.bashrc
source ~/.bashrc
install -m 0755 /root/.local/bin/mise /usr/local/bin/mise
```

The `.bashrc` activation is for your interactive shell only; systemd does not
read it. If your mise binary lives elsewhere, adjust the paths above. On the
droplet, install the base packages and create an unprivileged app user:

```sh
sudo apt update && sudo apt install -y caddy sqlite3 ca-certificates
sudo adduser --system --group --home /var/lib/pollingjuegos pollingjuegos
sudo install -d -o pollingjuegos -g pollingjuegos -m 0755 /opt/pollingjuegos
sudo install -d -o pollingjuegos -g pollingjuegos -m 0700 /var/backups/pollingjuegos
```

The existing mise installation supplies uv; do not run the separate Astral uv
installer. The `pollingjuegos` service account needs its **own** mise-managed
uv installation (your login user's mise data directory may be inaccessible).
From a local checkout, build the frontend as above and upload the source and
`frontend/dist/` over Tailscale (replace `TAILSCALE_HOST` with its Tailscale IP
or MagicDNS name). **Run this from the repository root**, not `frontend/`:

```sh
rsync -a --exclude='.git/' --exclude='.venv/' --exclude='node_modules/' \
  --exclude='db.sqlite3*' --exclude='.env' --exclude='.env.*' \
  --exclude='staticfiles/' --exclude='terraform/' --exclude='mise.local.toml' \
  ./ root@TAILSCALE_HOST:/opt/pollingjuegos/
ssh root@TAILSCALE_HOST 'chown -R pollingjuegos:pollingjuegos /opt/pollingjuegos && chmod 755 /opt/pollingjuegos'
```

Do **not** add `--delete`: the server's database, virtualenv, and collected
static files live under `/opt/pollingjuegos` and must survive each upload.
Use a reviewed local checkout; rsync also sends uncommitted changes. No
repository token or deploy key is needed on the droplet.

On the droplet, install uv through mise as `pollingjuegos` and sync
Python/dependencies (`.python-version` pins Python, which uv downloads for that
user). Only install uv, not the pnpm/terraform tools from `mise.toml`:

```sh
sudo -u pollingjuegos -H sh -c 'cd /opt/pollingjuegos && /usr/local/bin/mise install uv@0.12.19 && /usr/local/bin/mise exec -- uv sync --locked --no-dev'
```

The systemd service runs `/opt/pollingjuegos/.venv/bin/gunicorn` directly, so
it does not need mise or a login shell at runtime. Install the deployment files
from the uploaded checkout:

```sh
sudo install -o root -g pollingjuegos -m 0640 /opt/pollingjuegos/deploy/pollingjuegos.env.example /etc/pollingjuegos.env
sudoedit /etc/pollingjuegos.env  # replace ALL placeholders; keep the secret key stable
sudo install -m 0644 /opt/pollingjuegos/deploy/Caddyfile /etc/caddy/Caddyfile
sudo install -m 0644 /opt/pollingjuegos/deploy/pollingjuegos.service /etc/systemd/system/pollingjuegos.service
sudo install -m 0644 /opt/pollingjuegos/deploy/pollingjuegos-backup.service /etc/systemd/system/pollingjuegos-backup.service
sudo install -m 0644 /opt/pollingjuegos/deploy/pollingjuegos-backup.timer /etc/systemd/system/pollingjuegos-backup.timer
sudo install -m 0755 /opt/pollingjuegos/deploy/backup-sqlite.sh /usr/local/bin/pollingjuegos-backup
sudo install -d /etc/systemd/journald.conf.d
sudo install -m 0644 /opt/pollingjuegos/deploy/journald.conf /etc/systemd/journald.conf.d/pollingjuegos.conf
sudo systemctl restart systemd-journald
sudo systemctl daemon-reload
sudo caddy validate --config /etc/caddy/Caddyfile
sudo systemctl enable --now caddy
```

Run database migration and static collection with the production environment
via transient systemd units (no credentials on the command line):

```sh
sudo systemd-run --wait --collect -p User=pollingjuegos -p WorkingDirectory=/opt/pollingjuegos -p EnvironmentFile=/etc/pollingjuegos.env /opt/pollingjuegos/.venv/bin/python manage.py migrate --noinput
sudo systemd-run --wait --collect -p User=pollingjuegos -p WorkingDirectory=/opt/pollingjuegos -p EnvironmentFile=/etc/pollingjuegos.env /opt/pollingjuegos/.venv/bin/python manage.py collectstatic --noinput
sudo systemctl enable --now pollingjuegos.service pollingjuegos-backup.timer
sudo systemctl start pollingjuegos-backup.service  # verify the first backup
```

For updates, stop the service briefly, rebuild the frontend locally and
repeat the rsync and ownership commands above. Then run
`sudo -u pollingjuegos -H sh -c 'cd /opt/pollingjuegos && /usr/local/bin/mise exec -- uv sync --locked --no-dev'`,
`migrate`, and `collectstatic` again before starting the service. Reinstall
any changed systemd/Caddy/journald files as needed (and reload the relevant
service). If you previously configured Git access on the droplet, revoke the
repo token or deploy key at GitHub and remove its stored credential/private key
from the droplet. For the token workflow, remove
`/var/lib/pollingjuegos/.git-credentials` and unset that user's Git credential
helper. Leave the droplet's **SSH login key** (`arch`) alone.
`journalctl -u pollingjuegos -u caddy` shows service errors. The journald drop-in
caps persistent journal use at 100 MB (50 MB for runtime logs) and reserves
free space; this limits **journal logs only**, not the database, backups, or
other files. The backup timer makes daily consistent SQLite snapshots, keeping
roughly two weeks **on the same disk**: copy them offsite with an independently
configured backup job and test restores. Neither local copies nor optional DO
backups alone protect against all droplet/account failures. Set disk-space and
backup-failure alerts; 10 GB and 512 MB leave little headroom. The app's
console email backend is development-only: configure a production email
backend before relying on email delivery or a clean `check --deploy`.

Before `terraform apply` removes public port 22, confirm a **new** SSH
session over Tailscale works (not just an existing connection). Inspect the
plan to ensure it updates only the firewall; do not remove the SSH key from the
droplet. DigitalOcean's firewall sees Tailscale's underlying network traffic,
not its inner SSH port, so no DO inbound SSH rule is required for relayed
Tailscale connections. Direct Tailscale connections may require an additional
inbound UDP rule; without one, Tailscale can use its relay network. Keep an
out-of-band recovery path (DO console), and use Tailscale's access controls to
restrict SSH. Avoid compiling the frontend on this tiny server.

## Static files and deployment

For local development, enable Django's debug mode explicitly to have `runserver`
serve static files from the installed apps:

```sh
DJANGO_DEBUG=true uv run manage.py runserver
```

`DJANGO_DEBUG` defaults to false; values other than `true` (case-insensitive) do
not enable debug mode. For deployment, set `DJANGO_ALLOWED_HOSTS` to a comma-separated
list of your hostnames, set a persistent `DJANGO_SECRET_KEY`, and collect static
files during the build/deploy:

```sh
uv run manage.py collectstatic --noinput
```

WhiteNoise serves the collected admin/allauth static assets from `staticfiles/`
through the Django app with compressed, hashed filenames. Keep that directory
available to the running app; it is generated and Git-ignored. Static files are
not user uploads. Other production security/email settings still need configuring
before deploying (`uv run manage.py check --deploy`).

## Discord sign-in

Create a Discord OAuth2 application and set its redirect URL to
`https://YOUR_HOST/accounts/discord/login/callback/` (use `http://localhost:5173/...`
for local frontend development). Set `DISCORD_CLIENT_ID` and `DISCORD_CLIENT_SECRET` in the
environment before starting Django. The app requests only the `identify` scope.
Use a stable `DJANGO_SECRET_KEY` for persistent sessions.

After migrating, approve Discord **user IDs** (not usernames) before anyone signs in:

```sh
uv run manage.py shell -c 'from polls.models import ApprovedDiscordUser; ApprovedDiscordUser.objects.create(discord_id="YOUR_DISCORD_USER_ID")'
```

Users visit `/accounts/discord/login/` to sign in. Unapproved IDs receive a 403
before an account or session is created. Password login is disabled, including
for Django admin; `/admin/login/` offers a Discord sign-in link. To administer
the allowlist, mark an approved Discord user's linked Django account as
staff/superuser using
the Django shell after their first sign-in; they can then visit `/admin/`:

```sh
uv run manage.py shell -c 'from allauth.socialaccount.models import SocialAccount; u = SocialAccount.objects.get(provider="discord", uid="YOUR_DISCORD_USER_ID").user; u.is_staff = u.is_superuser = True; u.save()'
```
Disabling an allowlist entry invalidates that user's existing session on the
next request. This does not terminate an already-running request.
