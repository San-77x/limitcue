# Packaging

Files for distributing LimitCue outside the release tarball.

## AUR — `limitcue-bin`

[`aur/PKGBUILD`](aur/PKGBUILD) installs the prebuilt release tarball, the
desktop entry, and the KWin integration.

To submit or update it, from a checkout of the AUR repository:

```sh
git clone ssh://aur@aur.archlinux.org/limitcue-bin.git
cp packaging/aur/PKGBUILD limitcue-bin/
cd limitcue-bin
makepkg --printsrcinfo > .SRCINFO   # regenerate after any PKGBUILD edit
makepkg -si                         # test the build locally first
git add PKGBUILD .SRCINFO && git commit -m "Update to 0.1.0" && git push
```

Bump `pkgver` and run `updpkgsums` (from `pacman-contrib`) for a new release.
The checksum must match the `.sha256` asset attached to the GitHub release.

## cargo-binstall

`Cargo.toml` carries `[package.metadata.binstall]` pointing at the same release
tarball, so once the crate is published to crates.io:

```sh
cargo binstall limitcue
```

installs a prebuilt binary without compiling. The asset is Linux x86_64 only,
which is the only target currently shipped.
