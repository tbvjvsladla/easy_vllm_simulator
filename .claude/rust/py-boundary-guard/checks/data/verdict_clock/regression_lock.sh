#!/bin/bash
# Regression lock (h30): the shell forms that need more than one line. r2 L2 w4 (a command continued with a backslash):
date -d \
  "yesterday" +%F
x=$(date -d \
  "+90 days" +%s)
date -d
date --date
git log \
  --since=2.weeks
date -d "" +%s
date --date= +%s
# a comment that ends in a backslash does not continue \
date
# h32: openssl verify checks validity against the clock unless it is given a time
openssl verify -CAfile ca.pem cert.pem
openssl verify -attime 1735689600 -CAfile ca.pem cert.pem
openssl verify -no_check_time -CAfile ca.pem cert.pem
ok=$(openssl verify -CAfile "$ca" "$cert" 2>&1)
ssh h openssl verify -CAfile ca.pem cert.pem
openssl x509 -in cert.pem -noout -enddate
