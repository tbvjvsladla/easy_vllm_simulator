# shellcheck shell=bash
# golden case
PROG_KIND=policy_registry
ARGS=(verify --as-of nope -- x)
ENVS=(COLUMNS=80)
