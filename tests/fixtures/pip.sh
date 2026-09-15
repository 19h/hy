#!/bin/sh
if [ "$1" = "-c" ]; then
    case "$2" in
        *'sys.version_info.major'*) printf '3.12\n'; exit 0 ;;
        *"sysconfig.get_paths()['purelib']"*)
            test -n "$HY_TEST_PURELIB" || exit 1
            printf '%s\n' "$HY_TEST_PURELIB"
            exit 0
            ;;
    esac
    printf '%s\n' '__hy__:{"executable":"/fixture/python","prefix":"/fixture","base_prefix":"/fixture","version":"3.12","pip_available":true,"externally_managed":false,"scripts":"/fixture/bin"}'
    exit 0
fi

printf '%s\n' '--invocation--' "$@" >> "$HY_TEST_PIP_ARGUMENTS"
case " $* " in
    *' --dry-run '*) phase=resolution ;;
    *) phase=installation ;;
esac
if [ -f "$HY_TEST_PIP_ARGUMENTS.fail-$phase" ]; then
    printf '%s\n' "fixture $phase failure" >&2
    exit 1
fi

destination=
find_links=
previous=
for argument in "$@"; do
    case "$previous" in
        --dest) destination=$argument ;;
        --find-links) find_links=$argument ;;
    esac
    previous=$argument
done
if [ "$3" = download ]; then
    test -n "$destination" || exit 2
    mkdir -p "$destination"
    printf '%s\n' 'fixture wheel' > "$destination/fixture-1-py3-none-any.whl"
    if [ -n "$HY_TEST_DOWNLOAD_SDIST" ]; then
        printf '%s\n' 'fixture source distribution' > "$destination/fixture-1.tar.gz"
    fi
fi
if [ -n "$HY_TEST_EXPECT_WHEEL" ] && [ "$3" = install ]; then
    test -f "$find_links/$HY_TEST_EXPECT_WHEEL" || exit 3
fi
