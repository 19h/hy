#!/bin/sh
test "$1" = '-c' || exit 99
test "$2" = "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')" || exit 98
printf 'version:%s|%s|%s\n' "$PYTHONHOME" "$VIRTUAL_ENV" "$PYTHONUTF8" >> "$HY_TEST_CALLS"
printf '%s' "${HY_TEST_VERSION-3.12}"
printf 'captured version stderr' >&2
exit "${HY_TEST_STATUS:-0}"
