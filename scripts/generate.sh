#!/usr/bin/env bash

# Tweakable config for your engine

# Target repository (any Git-based repo link should work)
GIT_REPO_LINK="https://gitlab.com/mhouppin/stash-bot"

# Default cloning path
GIT_DEFAULT_PATH=stash_gen

# Makefile path, used for building the engine AND for verifying if the script is
# at the correct location
GIT_MAKEFILE_PATH=src

# Engine binary path to use for generating games
ENGINE_PATH=src/stash-bot

# Expected NPS of the engine
EXPECTED_NPS=2400000

# Expected base TC, in milliseconds
EXPECTED_TIME=1000

# Expected increment, in milliseconds
EXPECTED_INC=10

set -e
cd "$(dirname $0)"

if test ! -f "$GIT_MAKEFILE_PATH/Makefile"
then
    echo "Generator directory not found."
    IFS="" read -p "Desired installation path (defaults to ./$GIT_DEFAULT_PATH): " DIRPATH

    if test -z "$DIRPATH"
    then
        DIRPATH="$GIT_DEFAULT_PATH"
    fi

    echo "Cloning repository..."
    git clone -q "$GIT_REPO_LINK" "$DIRPATH"
    cp generate.sh "$DIRPATH"
    cd "$DIRPATH"
    echo
    echo "NOTE: the 'generate.sh' script has been copied to '$DIRPATH',"
    echo "so you can remove the original at the end of its execution."
    echo "The next time you want to run this script, you can run the"
    echo "following command:"
    echo
    echo "\$> bash '$DIRPATH/generate.sh'"
    echo
fi

git fetch --quiet
LOCAL="$(git rev-parse @)"
REMOTE="$(git rev-parse @{u})"

if test "_$LOCAL" != "_$REMOTE"
then
    echo "Updating compile..."
    git pull -q
    make -C "$GIT_MAKEFILE_PATH" > /dev/null
fi

if test ! -x "$ENGINE_PATH"
then
    echo "Compiling..."
    make -C "$GIT_MAKEFILE_PATH" > /dev/null
fi

test -x "$ENGINE_PATH"

if test ! -x cutechess-linux
then
    echo "Downloading cutechess-cli..."
    wget -q 'https://github.com/AndyGrant/OpenBench/raw/cf1cabfb92baae475ea8963243a2b83f948e4a7c/CoreFiles/cutechess-linux'
    chmod +x cutechess-linux
    test -x cutechess-linux
fi

if test ! -f 4moves_noob.epd
then
    echo "Downloading book..."
    wget -q 'https://github.com/AndyGrant/OpenBench/raw/b0227bc282d5f30533c30e78a40e43a18ab43f00/Books/4moves_noob.epd.zip'
    unzip -q 4moves_noob.epd.zip
    rm 4moves_noob.epd.zip
    test -f 4moves_noob.epd
fi

echo
read -p "Set the number of cores to use for generation (defaults to 1): " CORES

if test -z "$CORES"
then
    CORES=1
fi

MAX_CORES=$(nproc --all)

if test $MAX_CORES -le $CORES
then
    echo "You requested $CORES cores but your CPU only has $MAX_CORES."
    read -p "Are you sure about your choice ? (y/N) " CHOICE
    if test "_$CHOICE" != "_y"
    then
        echo "Aborting generation."
        exit 0
    fi
fi

if let "T=$CORES-1"
then
    for i in `seq 1 $T`
    do
        "./$ENGINE_PATH" bench > /dev/null &
    done
fi


NPS=$("./$ENGINE_PATH" bench | tr -c '[:alnum:][:space:]' ' ' \
    | grep -Eio '([[:digit:]]+[[:space:]]+nps)|(nps[[:space:]]+[[:digit:]]+)|(nodes second[[:space:]]+[[:digit:]]+)' \
    | grep -Eo '[[:digit:]]+' \
    | tail -n 1)

SCALED_TIME=$(( EXPECTED_NPS * EXPECTED_TIME / NPS ))
SCALED_INC=$(( EXPECTED_NPS * EXPECTED_INC / NPS ))
GAME_LENGTH=$(( SCALED_TIME * 2 + SCALED_INC * 100))
GAMES_PER_HR=$(( 3600000 / GAME_LENGTH ))

TIME=$(printf "%d.%03d" $((SCALED_TIME / 1000)) $((SCALED_TIME % 1000)))
INC=$(printf "%d.%03d" $((SCALED_INC / 1000)) $((SCALED_INC % 1000)))

echo
echo "Your machine NPS is $NPS, time control will be scaled to $TIME+${INC} sec."
echo "With your current config, you can expect to run about $GAMES_PER_HR games/hour."

read -p "Set the max number of games to run (defaults to 10000): " GAMES

if test -z "$GAMES"
then
    GAMES=10000
fi

./cutechess-linux -each proto=uci cmd="./$ENGINE_PATH" tc="$TIME+$INC" option."Move Overhead"=30 option.Hash=2 \
    -engine name=A -engine name=B -concurrency $CORES -rounds $GAMES \
    -draw movenumber=35 movecount=5 score=8 -resign movecount=5 score=400 \
    -openings file=4moves_noob.epd format=epd order=random -pgnout dataset.pgn min fi
