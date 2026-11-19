#!/bin/bash
# date -u +%s in a comment is no read
NOW=$(date -u +%FT%TZ)
START=$EPOCHSECONDS
find "$DIR" -mmin -10 -print
[ "$a" -nt "$b" ] && echo newer
TTL_DAYS=7
when=$(date -u -d "@$START" +%F)   # a given time: no read
git log --since=1.week --oneline
stat -c %Y "$f"
printf '%(%F)T\n' -1
echo "$NOW $when"
