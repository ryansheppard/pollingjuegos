"""Run Gunicorn with the production Dockerfile's server arguments."""

import sys

from gunicorn.app.wsgiapp import run


def main() -> None:
    sys.argv = [
        "gunicorn",
        "--workers",
        "1",
        "--threads",
        "2",
        "--bind",
        "0.0.0.0:8000",
        "--access-logfile",
        "-",
        "--error-logfile",
        "-",
        "config.wsgi:application",
    ]
    run()


if __name__ == "__main__":
    main()
