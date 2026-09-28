"""Export a fresh, migrated SQLite database for initial D1 provisioning.

Run from the repository root: uv run scripts/export_initial_d1.py
This is for a NEW empty D1 database, not for updating an existing one.
"""

import os
import re
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "migrations" / "0001_initial.sql"

with tempfile.TemporaryDirectory() as tmp:
    db_path = Path(tmp) / "initial.sqlite3"
    env = os.environ.copy()
    env.pop("USE_CLOUDFLARE_D1", None)
    env["DJANGO_SQLITE_PATH"] = str(db_path)
    subprocess.run(
        [sys.executable, str(ROOT / "manage.py"), "migrate", "--noinput"],
        cwd=ROOT,
        env=env,
        check=True,
    )
    with sqlite3.connect(db_path) as db:
        tables = []
        inserts = {}
        indexes = []
        for line in db.iterdump():
            if line in ("BEGIN TRANSACTION;", "COMMIT;") or "sqlite_sequence" in line:
                continue
            if line.startswith("CREATE TABLE "):
                tables.append(line)
            elif match := re.match(r'^INSERT INTO "([^"]+)" ', line):
                inserts.setdefault(match.group(1), []).append(line)
            elif line.startswith(("CREATE INDEX ", "CREATE UNIQUE INDEX ")):
                indexes.append(line)
            else:
                raise ValueError(f"Unexpected SQLite dump statement: {line}")

        # SQLite's dump interleaves rows with CREATE TABLE. D1 enforces foreign
        # keys, so create all tables before inserting referenced rows.
        pending = set(inserts)
        ordered_inserts = []
        while pending:
            ready = sorted(
                table
                for table in pending
                if not {
                    row[0]
                    for row in db.execute(
                        'SELECT "table" FROM pragma_foreign_key_list(?)', (table,)
                    )
                }
                & pending
            )
            if not ready:
                raise ValueError(
                    f"Cyclic foreign keys among populated tables: {pending}"
                )
            for table in ready:
                ordered_inserts.extend(inserts[table])
                pending.remove(table)

        statements = tables + ordered_inserts + indexes

OUTPUT.parent.mkdir(exist_ok=True)
OUTPUT.write_text(
    "-- Initial Django schema and seed data; apply only to an empty D1.\n"
    + "\n".join(statements)
    + "\n"
)
print(f"Wrote {OUTPUT}")
