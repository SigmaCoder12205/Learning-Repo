#!/usr/bin/env bash
# Generate local bootstrap secrets once; never print them or overwrite an existing file.
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
umask 077
if [[ ! -e .env ]]; then
    set -o noclobber
    {
        printf 'POSTGRES_PASSWORD=%s\n' "$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
        printf 'KEYCLOAK_ADMIN_PASSWORD=%s\n' "$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
    } > .env
fi
docker compose up -d --wait --wait-timeout 180
printf 'Keycloak is ready at http://127.0.0.1:8787. Bootstrap admin credentials are in auth/.env.\n'
