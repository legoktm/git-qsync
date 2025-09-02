# git-qsync

git-qsync enables easy sharing of Git repositories, branches and commits across multiple [Qubes OS](https://www.qubes-os.org/) VMs (Git Qubes Sync).

## How it works

1. Run `git qsync export`, which creates a [Git bundle](https://git-scm.com/docs/git-bundle) of the current branch's delta from the main branch. (If you export the main branch, it'll include the entire branch's history.)
2. The export command will use `qvm-move` to move that bundle to another VM, opening a dom0 prompt asking you which VM to send the data to. Notably, data won't cross the VM boundary without your explicit approval.
3. In the recipient VM, in a Git checkout of the same repository, run `git qsync import` to import the latest corresponding bundle from `~/QubesIncoming` and check out that branch. If it would overwrite an existing branch, it'll prompt for confirmation.
4. That's it!

## Installation

```
cargo install --locked --git https://github.com/legoktm/git-qsync
```

If you run `git qsync init`, it'll set up global aliases so `git qe` → `git qsync export`, and `git qi` → `git qsync import`.

## Security limitations

git-qsync does no verification of received Git bundles before it imports them aside from running `git bundle verify`,
so there's no protection against a malicious VM exporting a bundle that exploits a vulnerability against Git.
There's also no verification of the bundle contents itself, in case it exports a commit with malicious code.

## Goals

* Easily share Git branches and commits across Qubes VMs with minimal hassle
* Don't require any set up in dom0, just use existing cross-VM tooling (`qvm-move`)

## License

GPL-3.0-or-later, though most of the code is AI-authored and ineligible for copyright protection.
