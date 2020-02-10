with import <nixpkgs> {};

pkgs.mkShell {
    buildInputs = let
        sqlite = pkgs.sqlite.overrideAttrs (oldAttrs: rec {
            NIX_CFLAGS_COMPILE = oldAttrs.NIX_CFLAGS_COMPILE or "" + " -DSQLITE_USE_URI=1";
        });
    in [ stdenv.cc sqlite.dev ];
}
