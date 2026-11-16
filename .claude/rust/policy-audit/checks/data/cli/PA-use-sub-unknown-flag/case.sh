# shellcheck shell=bash
# golden case
PROG_KIND=policy_registry
ARGS=(verify --bogus 1 --as-of nope)
ENVS=(COLUMNS=80)
