# niri-composer

> **WIP** currently in very early development, not usable yet.

A composer daemon for niri wayland compositor.

The projects idea is to fill in the gaps of the default niri config, which doesnt have the ability to do things like:

- Conditional keybindings
- Conditional action triggers

In order to do things like this you need to interface with niri-ipc, probably with bash and jq, which is not ideal in terms of speed and usability.

niri-composer is a daemon that runs alongside niri, connects to its ipc, and lets you create multibranch keybindings, in kdl, with all actions from niri config available to use.
