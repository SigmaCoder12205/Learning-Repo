# Packaging and releases

| Path | What |
| --- | --- |
| `../Makefile` `dist` | Builds `dist/bench-VERSION-TARGET.tar.gz` and a `.sha256` file. The archive holds `bin/bench`, the `bin/cppbench` symlink, headers, guides and license, laid out like a prefix. |
| `smoke-test.sh ARCHIVE` | Extracts an archive, checks that a musl build is static, and runs real benchmarks with it |
| `aur/bench-git/` | AUR package that builds `main` from source and runs the test suite in `check()` |
| `aur/bench-bin/` | AUR package that installs the static binary from a GitHub release |
| `../../.github/workflows/bench-ci.yml` | `make verify` and a static musl build, on pushes to `main` and pull requests touching `testing/` |
| `../../.github/workflows/bench-release.yml` | On a `bench-vX.Y.Z` tag: verifies, builds the static archive, smoke-tests it, and publishes a GitHub release |

## Making a release

1. Bump `version` in `Cargo.toml`, run `cargo build` so `Cargo.lock` follows, and update `HANDOFF.md` and the README.
2. Merge to `main`, then tag the merge commit and push the tag:
   `git tag -a bench-v0.15.0 -m "bench 0.15.0" && git push origin bench-v0.15.0`
3. The release workflow refuses a tag that does not match `Cargo.toml`.
4. Update `aur/bench-bin`: set `pkgver`, reset `pkgrel=1`, then run `updpkgsums` and `makepkg --printsrcinfo > .SRCINFO`, and push both files to the AUR repository.

## Static builds locally

```bash
sudo pacman -S musl
rustup target add x86_64-unknown-linux-musl
make dist DIST_TARGET=x86_64-unknown-linux-musl
packaging/smoke-test.sh dist/bench-*-x86_64-unknown-linux-musl.tar.gz
```

On musl targets, `build.rs` links the embedded C measurement helper with `-static`, because it runs from a memfd and must not depend on a musl loader on the user's machine. The `accounts` feature is not part of release builds.

## Publishing to the AUR

Not published yet: AUR account registration was paused on 2026-11-28 (announced on aur-general and the Arch news feed). Until then, install with `makepkg -si` from `aur/bench-git`. Once the packages are published, switch the README and release notes back to `yay -S bench-bin` / `yay -S bench-git`.

AUR packages live in their own Git repositories (`ssh://aur@aur.archlinux.org/bench-git.git`). Copy `PKGBUILD` and `.SRCINFO` there, then commit and push. This needs an AUR account with your SSH public key. Test first with `makepkg -si` or a clean chroot (`extra-x86_64-build` from devtools).
