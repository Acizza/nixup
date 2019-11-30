# nixup

[![dependency status](https://deps.rs/repo/github/acizza/nixup/status.svg)](https://deps.rs/repo/github/acizza/nixup)

This is a tool for [NixOS](https://nixos.org/) to display updates to system-level packages and their dependencies.

# Example Output

```
4 system package update(s)

linux: 4.20.10 -> 4.20.11
curl|bin: 7.63.0 -> 7.64.0
fish: 2.7.1 -> 3.0.0
^ db: 4.8.30 -> 5.3.28
^ pcre2: 10.31 -> 10.32
wine-wow|staging: 4.0-rc5 -> 4.2

2 global dependency update(s)

curl: 7.63.0 -> 7.64.0
glibc: 2.27 -> 2.28
```

Note that the `db` and `pcre2` updates only apply to the `fish` package. If all system packages used the same version of those dependencies, then they would have showed up in the global dependency section instead.

# Usage

Due to the nature of how NixOS handles updates, you can only see which packages were updated after you update your system, and you must run the tool before you update in order to save the current system package state.

To save the current package state, run the program with the `-s` flag. Note that you don't necessarily have to save the package state before every update; so you could, for example, run it once a week or month if you'd rather see all of the updates made over that kind of time period.

After you have updated your system, you can simply run the program without any arguments and it will display any package updates that have been made since the package state was last saved.

For a small example of the program being used in an update script, see here:
https://github.com/Acizza/dotfiles/blob/desktop/updatesys.sh

If you'd like to use this program in your system overlay, you can find a Nix package definition for it here:
https://github.com/Acizza/nixos-config/blob/desktop/overlays/pkgs/nixup.nix