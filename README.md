# Manta Cargo Workspace
---

## Motivation

This repository contains the [cargo workspace](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html) for the following projects:

+ [ochami-rs](https://github.com/OpenCHAMI/ochami-rs#)
+ [csm-rs](https://github.com/eth-cscs/csm-rs)
+ [manta-backend-dispatcher](https://github.com/eth-cscs/manta-backend-dispatcher)
+ [manta-ws](https://github.com/eth-cscs/manta-ws)
+ [manta](https://github.com/eth-cscs/manta)

Since feature implementation require modifications on one or more of these repositories, it is easier to manage shared dependencies in one central location. It makes the release and the dependency upgrade processes much simpler.
Using git submodules allows us to easily share the current state of the project to make contributing easier.

## How to use cargo workspaces

This repository contains a Cargo.toml with a "workspace.dependencies" section that can be inherited by each of the packages in the workspace
If several packages in the workspace share an external dependency called "external_", then it is worth adding it to the workspace Cargo.toml and make the following changes to the Cargo.toml of the
packages:
```toml
# Change from this:
external_package = { version = "0.12.13" }
# to this:
external_package = { workspace = true }
```
Please note that you don't have to share depdencies between projects in a workspace. If one of the projects in the workspace requires a specific version of one dependency, you can always hard code that version in its Cargo.toml.

Regarding dependencies between workspace packages, the line "workspace=true" will cause the packages to use the LOCAL version of the packages. We use git submodules to enforce specific versions of each repositories that should be checked out .

You can run cargo operations in the workspace
```sh
# run tests in all the workspace packages
cargo test
# run tests in a single pacakge
cargo test --package workspace_package_1

# builds all the workspace packages in a top-level target/ directory
cargo build
# build single package in a top-level target/ directory
cargo build --pacakge workspace_package_1

# run cargo release in ever directory
cargo release patch
# run cargo release in a single workspace package
cargo release patch --package workspace_package_1
```

## How to use git submodules 

```sh
# Add a new git submodule

# 1) run the follwing commands
git submodule add <repo url> <path>
git add . && git commit -m 'chore: add new submodule <repo>' && git push
# 2) update the README.md, add the new project

# Clone with all submodules
git clone --recurse-submodules https://github.com/eth-cscs/manta-cargo-workspace

# Fetch latest commit for all submodules
git submodule update --remote

# Manually update checked out branch to latest commit
cd <submodule path>
git pull
cd ..
git add <submodule path>
git commit -m "Update <submodule> to latest in branch <branch>" && git push

# Manually update to specific ref
cd <submodule path>
git checkout <git tag>
cd ..
git add <submodule path>
git commit -m "Update <submodule> to <git tag>" && git push

# Push changes in a submodule
git push origin HEAD:<branch name>

# Fetch latest commit for all submodules
git submodule update --remote

# check the current ref of each subdmodule
git submodule status
```

## Pipelines / Github Actions

Work in progress
