"""Cloudflare Python Worker entrypoint for the Django WSGI application."""

import os

from django_cf import DjangoCF  # ty: ignore[unresolved-import]
from workers import WorkerEntrypoint  # ty: ignore[unresolved-import]

os.environ.setdefault("DJANGO_SETTINGS_MODULE", "config.settings")
os.environ["USE_CLOUDFLARE_D1"] = "true"


class Default(DjangoCF, WorkerEntrypoint):
    def get_app(self):
        from config.wsgi import application

        return application
