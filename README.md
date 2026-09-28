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
        root * /absolute/path/to/sjpoll/frontend/dist
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
