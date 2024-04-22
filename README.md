# Personal collection of Qdrant tools

- <https://qdrant.tech/>
- <https://github.com/qdrant/qdrant>

## Updating

- `nix flake update`
- `nix develop '.#dev' -c ./poetry/regen.py $PATH_TO_QDRANT_REPO`
- Update Rust toolchain version in [flake.nix](./flake.nix).
