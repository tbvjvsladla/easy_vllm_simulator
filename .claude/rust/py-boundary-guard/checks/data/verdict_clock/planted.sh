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
a=$(date -d "+90 days" +%F)
due=$(date -d "$created + 90 days" +%s)
given=$(date -u -d "$ets" +%s)
t=$(bash -c 'date +%s')
r=$(ssh h "date -u +%s")
if (( SECONDS > 30 )); then echo late; fi
[ "$a" -ot "$b" ] && echo older
git log --since="2 weeks ago" --oneline
