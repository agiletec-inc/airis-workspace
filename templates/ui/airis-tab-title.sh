#!/bin/sh
# airis-tab-title.sh — Claude Code terminal tab-title hook.
#
# Generated and kept current by `airis ui install`. Intentionally
# self-contained: it does NOT call the `airis` binary, so a stale, rebuilt,
# or missing `airis` can never break the hook (the version-skew bug that the
# old `airis claude tab-title` wiring suffered from).
#
# Usage: airis-tab-title.sh <state> <running-emoji> <waiting-emoji> <idle-emoji>
#   state: idle | running | waiting | stop
# The Claude Code hook JSON payload arrives on stdin.
#
# Claude Code hooks have no controlling TTY, so the script cannot write an OSC
# escape sequence directly. It prints {"terminalSequence": "<OSC>"} and Claude
# Code emits the (allowlisted) OSC 0 sequence on the hook's behalf.

state="$1"
running_emoji="$2"
waiting_emoji="$3"
idle_emoji="$4"

payload=$(cat)

# Repo name from the hook payload's cwd (fallback: $PWD).
cwd=$(printf '%s' "$payload" | jq -r '.cwd // empty' 2>/dev/null)
[ -z "$cwd" ] && cwd="$PWD"
repo=$(basename "$cwd" 2>/dev/null)
[ -z "$repo" ] && repo="claude"

emoji=""
case "$state" in
  running) emoji="$running_emoji" ;;
  waiting) emoji="$waiting_emoji" ;;
  idle)    emoji="$idle_emoji" ;;
  stop)
    # Stop fires on every turn end — including when Claude ends its turn by
    # asking via AskUserQuestion / ExitPlanMode. The payload carries no flag
    # to tell "done" from "asking", so resolve it from the transcript: the
    # last assistant entry decides.
    emoji="$idle_emoji"
    tp=$(printf '%s' "$payload" | jq -r '.transcript_path // empty' 2>/dev/null)
    if [ -n "$tp" ] && [ -f "$tp" ]; then
      last=$(tail -n 200 "$tp" | grep '"type":"assistant"' | tail -n 1)
      if [ -n "$last" ]; then
        names=$(printf '%s' "$last" | jq -r \
          '[.message.content[]? | select(.type=="tool_use") | .name] | join(" ")' \
          2>/dev/null)
        case " $names " in
          *" AskUserQuestion "*|*" ExitPlanMode "*) emoji="$waiting_emoji" ;;
        esac
      fi
    fi
    ;;
  *) exit 0 ;;
esac

if [ -n "$emoji" ]; then
  title="$emoji $repo"
else
  title="$repo"
fi

# OSC 0 = set icon name + window/tab title (BEL-terminated).
osc=$(printf '\033]0;%s\007' "$title")
jq -nc --arg s "$osc" '{terminalSequence: $s}'
exit 0
