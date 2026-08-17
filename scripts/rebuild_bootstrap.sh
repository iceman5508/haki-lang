#!/bin/bash
# Rebuild the self-hosted Haki compiler and prove the bootstrap fixpoint.
# Usage: bash rebuild.sh [current-stage-binary]
# Produces haki_bootstrap/hakic_next (stage N+1) and, if the fixpoint holds,
# promotes it to haki_bootstrap/hakic_cur.
set -e
cd "$(dirname "$0")/../haki_bootstrap"
CUR=${1:-./hakic_cur}

echo "[1] $CUR  hakic.haki -> /tmp/bs_a.c"
"$CUR" hakic.haki --emit-c -o /tmp/bs_a.c > /dev/null
cc -std=gnu11 -O2 /tmp/bs_a.c -o ./hakic_a -lm -lpthread 2>/tmp/bs_a.cc.log
echo "    ok  $(wc -c < /tmp/bs_a.c) bytes"

echo "[2] stage A  hakic.haki -> /tmp/bs_b.c"
./hakic_a hakic.haki --emit-c -o /tmp/bs_b.c > /dev/null
cc -std=gnu11 -O2 /tmp/bs_b.c -o ./hakic_b -lm -lpthread 2>/tmp/bs_b.cc.log
echo "    ok  $(wc -c < /tmp/bs_b.c) bytes"

echo "[3] stage B  hakic.haki -> /tmp/bs_c.c"
./hakic_b hakic.haki --emit-c -o /tmp/bs_c.c > /dev/null

if cmp -s /tmp/bs_b.c /tmp/bs_c.c; then
    echo "    FIXPOINT: stage B == stage C (byte-identical)"
    cp ./hakic_b ./hakic_cur
    cp /tmp/bs_b.c  ./hakic_generated.c
    echo "    promoted -> haki_bootstrap/hakic_cur"
else
    echo "    NO FIXPOINT: bs_b.c != bs_c.c"
    cmp /tmp/bs_b.c /tmp/bs_c.c || true
    exit 1
fi
