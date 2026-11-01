"""
WSGI config for pollingjuegos project.

It exposes the WSGI callable as a module-level variable named ``application``.

For more information on this file, see
https://docs.djangoproject.com/en/6.1/howto/deployment/wsgi/
"""

import os

from django.core.wsgi import get_wsgi_application

os.environ.setdefault("DJANGO_SETTINGS_MODULE", "config.settings")

if os.environ.get("OTEL_CONFIG_FILE") and os.environ.get("HONEYCOMB_API_KEY"):
    import logging

    from opentelemetry.instrumentation.auto_instrumentation import initialize

    logging.basicConfig()
    initialize()

application = get_wsgi_application()
