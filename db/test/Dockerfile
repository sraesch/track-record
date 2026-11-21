# PostgreSQL with pgTAP and pg_prove, used only to run the database tests.
FROM postgres:18

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        "postgresql-${PG_MAJOR}-pgtap" \
        libtap-parser-sourcehandler-pgtap-perl \
    && rm -rf /var/lib/apt/lists/*
