#!/bin/sh
case "$1:$2" in
    '-c:import pip')
        echo pip-check >> "$HY_TEST_EVENTS"
        exit "${HY_TEST_PIP_STATUS:-0}"
        ;;
    '-c:'*)
        case "$2" in
            *sys.version_info*)
                echo version-check >> "$HY_TEST_EVENTS"
                printf '%s\n' '3.13'
                exit 0
                ;;
            *importlib.metadata*)
                echo script-lookup >> "$HY_TEST_EVENTS"
                printf '__hcli__:%s\n' "$HY_TEST_SCRIPT_JSON"
                exit 0
                ;;
            *"sysconfig.get_paths()['purelib']"*)
                echo purelib-check >> "$HY_TEST_EVENTS"
                printf '%s\n' "$HY_TEST_PURELIB"
                exit 0
                ;;
        esac
        ;;
    '-m:pip')
        printf 'pip|%s\n' "$*" >> "$HY_TEST_EVENTS"
        exit 0
        ;;
esac
printf 'child|%s\n' "$*" >> "$HY_TEST_EVENTS"
printf 'child output\n'
exit "${HY_TEST_CHILD_STATUS:-0}"
