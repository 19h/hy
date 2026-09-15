#!/bin/sh
case "$1:$2" in
    '-c:import pip')
        printf 'pip-check\n' >> "$HY_TEST_EVENTS"
        exit "${HY_TEST_PIP_STATUS:-0}"
        ;;
    '-c:'*)
        case "$2" in
            *'sys.version_info.major'*)
                printf 'version:%s|%s|%s\n' "$PYTHONHOME" "$VIRTUAL_ENV" "$PYTHONUTF8" >> "$HY_TEST_EVENTS"
                count=0
                if [ -f "$HY_TEST_VERSION_COUNT" ]; then IFS= read -r count < "$HY_TEST_VERSION_COUNT"; fi
                count=$((count + 1))
                printf '%s\n' "$count" > "$HY_TEST_VERSION_COUNT"
                if [ "${HY_TEST_READ_INPUT:-0}" = 1 ]; then
                    IFS= read -r line
                    printf 'version-input:%s\n' "$line" >> "$HY_TEST_EVENTS"
                fi
                if [ "$count" -le "${HY_TEST_GOOD_CALLS:-0}" ]; then
                    printf '3.12\n'
                else
                    /bin/cat "$HY_TEST_VERSION_STDOUT"
                    /bin/cat "$HY_TEST_VERSION_STDERR" >&2
                fi
                exit "${HY_TEST_VERSION_STATUS:-0}"
                ;;
            *"sysconfig.get_paths()['purelib']"*) printf '%s\n' "$HY_TEST_PURELIB"; exit 0 ;;
        esac
        ;;
    '-m:pip') printf 'pip-run\n' >> "$HY_TEST_EVENTS"; exit 0 ;;
esac
printf 'child-run\n' >> "$HY_TEST_EVENTS"
if [ "${HY_TEST_READ_INPUT:-0}" = 1 ]; then
    IFS= read -r line
    printf 'child-input:%s\n' "$line"
fi
exit 0
