fmt:
	nixfmt --width=80 flake.nix
	isort poetry/regen.py update.py
	black poetry/regen.py update.py

lint:
	deadnix flake.nix
	mypy --strict poetry/regen.py update.py
