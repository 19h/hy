#!/bin/sh
case "$1:$2" in
    '-c:import pip')
        printf 'pip-env:%s|%s|%s\n' "$PYTHONHOME" "$VIRTUAL_ENV" "$PYTHONUTF8" >> "$HY_TEST_EVENTS"
        if [ "${HY_TEST_READ_INPUT:-0}" = 1 ]; then
            IFS= read -r line
            printf 'pip-input:%s\n' "$line" >> "$HY_TEST_EVENTS"
        fi
        printf '\377\355\240\200'
        printf '\377\000' >&2
        exit "${HY_TEST_PIP_STATUS:-0}"
        ;;
    '-c:'*)
        case "$2" in
            *'sys.version_info.major'*)
                if [ "${HY_TEST_READ_INPUT:-0}" = 1 ]; then
                    IFS= read -r line
                    printf 'version-input:%s\n' "$line" >> "$HY_TEST_EVENTS"
                fi
                printf '3.12\n'
                exit 0
                ;;
            *"sysconfig.get_paths()['purelib']"*) printf '%s\n' "$HY_TEST_PURELIB"; exit 0 ;;
        esac
        ;;
    '-m:pip') printf 'pip-run\n' >> "$HY_TEST_EVENTS"; exit 0 ;;
esac
if [ "${HY_TEST_READ_INPUT:-0}" = 1 ]; then
    IFS= read -r line
    printf 'child-input:%s\n' "$line"
fi
exit 0
