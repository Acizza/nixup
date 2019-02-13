nixup
=====

This is a tool for [NixOS](https://nixos.org/) to display updates to packages defined in the `environment.systemPackages` variable of the system's configuration.

Example Output
==============

```
3 system package update(s)

curl|bin: 7.63.0 -> 7.64.0
fish: 2.7.1 -> 3.0.0
^ db: 4.8.30 -> 5.3.28
^ pcre2: 10.31 -> 10.32
wine-wow|staging: 4.0-rc5 -> 4.1

2 global dependency update(s)

curl: 7.63.0 -> 7.64.0
glibc: 2.27 -> 2.28
```

Note that the `db` and `pcre2` updates only apply to the `fish` package. If all system packages used the same version of those dependencies, then they would have showed up in the global dependency section instead.

Usage
=====

There are two modes for checking for updates:

1. Before an update, by launching with no arguments
2. After an update, by saving the system package state *before* updating the system

Mode 1 is useful if you only want a quick rundown of which system packages will be updated, and do not care about which dependencies of those packages will be updated.

Please keep in mind that when using mode 1, you must run the program as root in order for it to work properly.

Mode 2 will display what dependencies of every system package have been updated. To use mode 2, first run the program with the `-s` flag *before* updating the system. This will generate a list of the current system packages and their dependencies and save it to `~/.cache/nixup/saved_stores.mpack`. After updating the system (and not necessarily rebooting), run the program with the `-f` flag and the program will load the saved package state and display the updates between it and the current package set.

For a complete example of the program being used in an update script, see here:
https://gitlab.com/Acizza/dotfiles/blob/desktop/updatesys.sh

If you'd like to use this package as part of your system overlay, you can find a Nix package for it here:
https://gitlab.com/Acizza/nixos-config/blob/desktop/overlays/pkgs/nixup.nix