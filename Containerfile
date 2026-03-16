FROM docker.io/nixos/nix:latest

# Enable flakes, disable sandbox (avoids needing --privileged for Nix builds)
RUN echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf \
 && echo "sandbox = false" >> /etc/nix/nix.conf \
 && echo "max-jobs = auto" >> /etc/nix/nix.conf

# Install tooling needed before Nix builds start.
# git-minimal is already present in the base image — do not add nixpkgs.git.
RUN nix-env -iA \
      nixpkgs.nodejs_22 \
      nixpkgs.dbus \
      nixpkgs.pkg-config \
      nixpkgs.cacert \
 && nix-env --delete-generations old \
 && nix-collect-garbage -d

# Set npm global prefix to /usr/local so the bin lands in a directory
# already on PATH in every subsequent RUN step and at runtime.
ENV NPM_CONFIG_PREFIX=/usr/local
ENV PATH="/usr/local/bin:${PATH}"

# Install Claude Code globally and verify the binary is reachable.
RUN npm install -g @anthropic-ai/claude-code \
 && claude --version

# Sanity-check git (provided by the base image as git-minimal).
RUN git --version

WORKDIR /work

# Default: run the prompt non-interactively in YOLO mode.
# Override CMD for an interactive shell: podman run -it ... bash
CMD ["claude", "--dangerously-skip-permissions", \
     "-p", "/work/claude-code-build-prompt-v4.md"]
