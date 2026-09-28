# sjpoll

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
`https://YOUR_HOST/accounts/discord/login/callback/` (use `http://localhost:8000/...`
for local development). Set `DISCORD_CLIENT_ID` and `DISCORD_CLIENT_SECRET` in the
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
