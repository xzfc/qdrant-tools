fmt:
	nixfmt --width=80 flake.nix
	isort update.py
	black update.py

lint:
	deadnix flake.nix
	mypy --strict update.py
