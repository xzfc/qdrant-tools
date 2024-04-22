fmt:
	nixfmt --width=80 flake.nix
	isort poetry/regen.py
	black poetry/regen.py

lint:
	deadnix flake.nix
	mypy --strict poetry/regen.py
