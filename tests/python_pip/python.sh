#!/bin/sh
case "$1:$2" in
    '-c:import pip') exit 0 ;;
    '-c:'*) printf '%s\n' "$HY_TEST_PURELIB"; exit 0 ;;
    '-m:pip') ;;
    *) exit 99 ;;
esac

case " $* " in
    *' --dry-run '*) phase=resolution ;;
    *) phase=installation ;;
esac
printf 'phase:%s\n' "$phase" >> "$HY_TEST_EVENTS"
for argument do printf 'arg:%s\n' "$argument" >> "$HY_TEST_EVENTS"; done
printf 'environment:%s|%s|%s|%s\n' "$PYTHONHOME" "$VIRTUAL_ENV" "$PYTHONUTF8" "$PATH" >> "$HY_TEST_EVENTS"
if [ "${HY_TEST_READ_STDIN:-0}" = 1 ]; then
    IFS= read -r line
    printf 'stdin:%s\n' "$line" >> "$HY_TEST_EVENTS"
fi
printf '%s' "${HY_TEST_PIP_STDOUT-stdout}"
printf '%s' "${HY_TEST_PIP_STDERR-stderr}" >&2
if [ "${HY_TEST_FAIL_PHASE:-}" = "$phase" ]; then exit 7; fi
exit 0
