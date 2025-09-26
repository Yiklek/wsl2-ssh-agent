#!/usr/bin/env bash

if [ -z "$WSLENV" ]; then
    echo "This script is intended to be sourced in WSL."
    return 1
fi

if ! command -v socat &> /dev/null; then
    echo "socat could not be found, please install it."
    return 1
fi

export SSH_AUTH_SOCK=$HOME/.ssh/ssh-agent.sock
rm -f $SSH_AUTH_SOCK

bridge_path="wsl2-ssh-agent.exe"
if [ -n "$1" ]; then
    bridge_path="$1"
fi

if [ -n "$2" ]; then
    full_command="$bridge_path $2"
else
    full_command="$bridge_path"
fi

socat UNIX-LISTEN:$SSH_AUTH_SOCK,fork EXEC:"$full_command",nofork &
