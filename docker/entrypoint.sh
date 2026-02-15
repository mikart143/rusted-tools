#!/bin/sh
set -e

if [ ! -S /var/run/docker.sock ]; then
  dockerd --host=unix:///var/run/docker.sock &

  i=0
  while [ $i -lt 30 ]; do
    if [ -S /var/run/docker.sock ]; then
      break
    fi
    i=$((i + 1))
    sleep 1
  done
fi

exec "$@"
