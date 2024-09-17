#!/usr/bin/env bash

docker build -t voskbuilder .
id=$(docker create voskbuilder)
docker cp $id:/opt/vosk-api/src/copydir/libvosk.so ./lib/vosk
docker cp $id:/opt/vosk-api/src/copydir/vosk_api.h ./lib/vosk
docker rm -v $id
