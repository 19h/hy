#!/bin/sh
test "$1:$2:$3" = '-m:pip:download' || exit 99
for argument do printf 'arg:%s\n' "$argument" >> "$HY_TEST_EVENTS"; done
printf 'environment:%s|%s|%s|%s\n' "$PYTHONHOME" "$VIRTUAL_ENV" "$PYTHONUTF8" "$PATH" >> "$HY_TEST_EVENTS"
printf 'cwd:%s\n' "$(pwd -P)" >> "$HY_TEST_EVENTS"
if [ "${HY_TEST_READ_STDIN:-0}" = 1 ]; then
    IFS= read -r line
    printf 'stdin:%s\n' "$line" >> "$HY_TEST_EVENTS"
fi
printf '%s' "${HY_TEST_PIP_STDOUT-stdout}"
printf '%s' "${HY_TEST_PIP_STDERR-stderr}" >&2
exit "${HY_TEST_PIP_STATUS:-0}"
