nixup
=====

This is a tool for [NixOS](https://nixos.org/) to display version changes between updates to packages defined in the `environment.systemPackages` variable of the system's configuration.

Usage
=====

Due to various limitations, the tool must ben ran *before* you update the system, and can then only display the version changes *after* you have updated the system (but not necessarily rebooted).

Before performing a system update, run the program with the `-p` flag. The program will collect information on the currently installed system packages and store the results in `~/.cache/nixup/saved_stores.mpack`.

After performing a system update, you can simply run the program without any arguments and it will calculate which packages have been updated based off of the results saved by the previous step.

For a complete example of the program being used in an update script, see here:
https://gitlab.com/Acizza/dotfiles/blob/desktop/updatesys.sh